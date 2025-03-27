use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use std::sync::Arc;
use crate::proxy_pool::ProxyPool;
use tracing::{info, error, warn};
use crate::config::Config;
use colored::*;

#[derive(Clone)]
pub struct SocksServer {
    proxy_pool: Arc<ProxyPool>,
    config: Arc<Config>,
}

impl SocksServer {
    pub fn new(config: Config) -> Self {
        let proxy_pool = ProxyPool::new(config.clone());
        let server = SocksServer {
            proxy_pool: Arc::new(proxy_pool),
            config: Arc::new(config),
        };
        
        // 如果开启了自动切换，启动自动切换任务
        if server.config.proxy.auto_switch {
            let proxy_pool = Arc::clone(&server.proxy_pool);
            let switch_interval = server.config.proxy.switch_interval;
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(switch_interval)).await;
                    if let Some(proxy) = proxy_pool.next_proxy().await {
                        // 总是显示自动切换的日志，不受show_connection_log控制
                        println!("{} {} {} {} {}", 
                            "[自动切换]".blue().bold(),
                            "切换到新代理:".green().bold(),
                            proxy.address.cyan().bold(),
                            "(延迟:".yellow(),
                            format!("{}ms)", proxy.latency.as_millis()).yellow()
                        );
                    } else {
                        println!("{} {}", 
                            "[自动切换]".blue().bold(),
                            "没有可用的代理".red().bold()
                        );
                    }
                }
            });
        }
        
        server
    }

    pub fn get_proxy_pool(&self) -> &Arc<ProxyPool> {
        &self.proxy_pool
    }

    pub fn get_config(&self) -> &Arc<Config> {
        &self.config
    }

    pub fn get_bind_info(&self) -> (String, u16) {
        (
            self.config.server.bind_host.clone(),
            self.config.server.bind_port
        )
    }

    pub async fn run(&self) -> Result<()> {
        let addr = format!("{}:{}", 
            self.config.server.bind_host,
            self.config.server.bind_port
        );
        
        let listener = TcpListener::bind(&addr).await?;
        info!("SOCKS5服务器启动在: {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    if self.config.log.show_connection_log {
                        info!("新的连接来自: {}", addr);
                    }
                    let proxy_pool = Arc::clone(&self.proxy_pool);
                    let config = Arc::clone(&self.config);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, proxy_pool, Arc::clone(&config)).await {
                            if config.log.show_error_log {
                                error!("处理连接错误: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    if self.config.log.show_error_log {
                        warn!("接受连接失败: {}", e);
                    }
                }
            }
        }
    }

    async fn handle_connection(client: TcpStream, proxy_pool: Arc<ProxyPool>, config: Arc<Config>) -> Result<()> {
        let (mut inbound_reader, mut inbound_writer) = client.into_split();

        // 处理SOCKS5握手
        handle_handshake(&mut inbound_reader, &mut inbound_writer, &config).await?;

        // 读取SOCKS5请求
        let mut buf = [0u8; 4];
        inbound_reader.read_exact(&mut buf).await?;

        if buf[0] != 0x05 || buf[1] != 0x01 {
            return Err(anyhow::anyhow!("不支持的SOCKS5命令"));
        }

        // 读取目标地址
        let atyp = buf[3];
        let target_addr = match atyp {
            0x01 => { // IPv4
                let mut addr = [0u8; 4];
                inbound_reader.read_exact(&mut addr).await?;
                format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3])
            },
            0x03 => { // 域名
                let len = inbound_reader.read_u8().await? as usize;
                let mut domain = vec![0u8; len];
                inbound_reader.read_exact(&mut domain).await?;
                String::from_utf8(domain)?
            },
            0x04 => { // IPv6
                let mut addr = [0u8; 16];
                inbound_reader.read_exact(&mut addr).await?;
                return Err(anyhow::anyhow!("暂不支持IPv6"));
            },
            _ => return Err(anyhow::anyhow!("不支持的地址类型")),
        };

        // 读取端口
        let port = inbound_reader.read_u16().await?;
        let _target = format!("{}:{}", target_addr, port);

        // 获取代理
        if let Some(proxy) = proxy_pool.get_current_proxy().await {
            let proxy_addr: SocketAddr = proxy.address.parse()?;
            let mut upstream = match TcpStream::connect(proxy_addr).await {
                Ok(stream) => stream,
                Err(e) => {
                    if config.log.show_error_log {
                        eprintln!("代理连接失败: {} - {}", proxy.address, e);
                    }
                    // 发送失败响应
                    let response = [
                        0x05, 0x04, 0x00, 0x01,
                        0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00,
                    ];
                    inbound_writer.write_all(&response).await?;
                    return Ok(());
                }
            };

            // 与上游SOCKS5服务器进行握手
            upstream.write_all(&[0x05, 0x01, 0x00]).await?;
            let mut response = [0u8; 2];
            upstream.read_exact(&mut response).await?;
            
            if response[0] != 0x05 || response[1] != 0x00 {
                eprintln!("上游代理握手失败");
                return Ok(());
            }

            // 发送连接请求到上游代理
            let mut request = Vec::new();
            request.extend_from_slice(&[0x05, 0x01, 0x00]); // VER, CMD, RSV
            
            match atyp {
                0x01 => { // IPv4
                    request.push(0x01);
                    for octet in target_addr.split('.') {
                        request.push(octet.parse::<u8>()?);
                    }
                },
                0x03 => { // Domain
                    request.push(0x03);
                    request.push(target_addr.len() as u8);
                    request.extend_from_slice(target_addr.as_bytes());
                },
                _ => unreachable!(),
            }
            
            // 添加端口
            request.extend_from_slice(&port.to_be_bytes());
            
            // 发送请求到上游代理
            upstream.write_all(&request).await?;
            
            // 读取上游代理响应
            let mut response = [0u8; 4];
            upstream.read_exact(&mut response).await?;
            
            if response[1] != 0x00 {
                if config.log.show_error_log {
                    eprintln!("上游代理连接目标失败");
                }
                let response = [
                    0x05, 0x04, 0x00, 0x01,
                    0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00,
                ];
                inbound_writer.write_all(&response).await?;
                return Ok(());
            }
            
            // 跳过绑定地址和端口
            match response[3] {
                0x01 => { // IPv4
                    let mut addr = [0u8; 4];
                    upstream.read_exact(&mut addr).await?;
                },
                0x03 => { // Domain
                    let len = upstream.read_u8().await?;
                    let mut domain = vec![0u8; len as usize];
                    upstream.read_exact(&mut domain).await?;
                },
                0x04 => { // IPv6
                    let mut addr = [0u8; 16];
                    upstream.read_exact(&mut addr).await?;
                },
                _ => return Err(anyhow::anyhow!("上游代理返回了不支持的地址类型")),
            }
            let mut port = [0u8; 2];
            upstream.read_exact(&mut port).await?;

            // 发送成功响应给客户端
            let response = [
                0x05, 0x00, 0x00, 0x01,
                0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ];
            inbound_writer.write_all(&response).await?;

            // 双向转发数据
            let (mut upstream_reader, mut upstream_writer) = upstream.into_split();
            let client_to_proxy = tokio::io::copy(&mut inbound_reader, &mut upstream_writer);
            let proxy_to_client = tokio::io::copy(&mut upstream_reader, &mut inbound_writer);
            
            tokio::select! {
                res = client_to_proxy => {
                    if let Err(e) = res {
                        if config.log.show_error_log {
                            eprintln!("客户端到代理传输错误: {}", e);
                        }
                    }
                },
                res = proxy_to_client => {
                    if let Err(e) = res {
                        if config.log.show_error_log {
                            eprintln!("代理到客户端传输错误: {}", e);
                        }
                    }
                }
            }
        } else {
            // 发送失败响应
            let response = [
                0x05, 0x01, 0x00, 0x01,
                0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ];
            inbound_writer.write_all(&response).await?;
            if config.log.show_error_log {
                eprintln!("没有可用的代理");
            }
        }

        Ok(())
    }
}

async fn handle_handshake<R, W>(reader: &mut R, writer: &mut W, config: &Arc<Config>) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    // 读取客户端支持的认证方法
    let mut method_selection = [0u8; 2];
    reader.read_exact(&mut method_selection).await?;
    
    if method_selection[0] != 0x05 {
        return Err(anyhow::anyhow!("不支持的SOCKS版本"));
    }
    
    let nmethods = method_selection[1] as usize;
    let mut methods = vec![0u8; nmethods];
    reader.read_exact(&mut methods).await?;

    // 检查是否需要认证
    if config.proxy.use_auth {
        // 查找客户端是否支持用户名/密码认证 (0x02)
        if methods.contains(&0x02) {
            // 回复使用用户名/密码认证方法
            writer.write_all(&[0x05, 0x02]).await?;
            writer.flush().await?;
            
            // 处理用户名/密码认证
            let mut auth_version = [0u8; 1];
            reader.read_exact(&mut auth_version).await?;
            
            if auth_version[0] != 0x01 {
                return Err(anyhow::anyhow!("不支持的认证版本"));
            }
            
            // 读取用户名
            let ulen = reader.read_u8().await? as usize;
            let mut username = vec![0u8; ulen];
            reader.read_exact(&mut username).await?;
            let username = String::from_utf8(username)?;
            
            // 读取密码
            let plen = reader.read_u8().await? as usize;
            let mut password = vec![0u8; plen];
            reader.read_exact(&mut password).await?;
            let password = String::from_utf8(password)?;
            
            // 验证用户名和密码
            if username == config.proxy.username && password == config.proxy.password {
                // 认证成功
                writer.write_all(&[0x01, 0x00]).await?;
                writer.flush().await?;
            } else {
                // 认证失败
                writer.write_all(&[0x01, 0x01]).await?;
                writer.flush().await?;
                return Err(anyhow::anyhow!("认证失败"));
            }
        } else {
            // 客户端不支持我们需要的认证方法
            writer.write_all(&[0x05, 0xFF]).await?;
            writer.flush().await?;
            return Err(anyhow::anyhow!("客户端不支持所需的认证方法"));
        }
    } else {
        // 不需要认证，回复使用无认证方法
        writer.write_all(&[0x05, 0x00]).await?;
        writer.flush().await?;
    }

    Ok(())
} 