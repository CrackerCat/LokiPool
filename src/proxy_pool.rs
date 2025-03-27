use std::fs::{self, File};
use std::io::{self, BufRead};
use std::path::Path;
use tokio::sync::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use reqwest::Proxy;
use tokio::time::timeout;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::net::TcpStream;
use std::net::SocketAddr;
use crate::config::Config;
use std::error::Error as StdError;
use std::collections::HashSet;
use anyhow;
use std::fmt::Debug;

#[derive(Clone, Debug)]
pub struct ProxyEntry {
    pub address: String,
    pub latency: Duration,
    pub last_check: Instant,
    pub fail_count: u32,
}

pub struct ProxyPool {
    proxies: Arc<RwLock<Vec<ProxyEntry>>>,
    current_index: Arc<RwLock<usize>>,
    config: Arc<Config>,
    proxy_file: Arc<String>,
}

impl ProxyPool {
    pub fn new(config: Config) -> Self {
        ProxyPool {
            proxies: Arc::new(RwLock::new(Vec::new())),
            current_index: Arc::new(RwLock::new(0)),
            config: Arc::new(config.clone()),
            proxy_file: Arc::new(config.proxy.proxy_file),
        }
    }

    pub fn get_config(&self) -> &Arc<Config> {
        &self.config
    }

    // 通用的代理测试函数
    async fn test_proxy(proxy_addr: &str, timeout_secs: u64, fast_check: bool) -> anyhow::Result<Duration> {
        let client = reqwest::Client::builder()
            .proxy(Proxy::all(format!("socks5://{}", proxy_addr))?)
            .build()?;

        let start = Instant::now();
        
        if fast_check {
            // 健康检查只发送HEAD请求
            let resp = timeout(Duration::from_secs(timeout_secs), client.head("http://www.baidu.com").send()).await??;
            if !resp.status().is_success() {
                return Err(anyhow::anyhow!("HTTP状态码错误: {}", resp.status()));
            }
        } else {
            // 完整测试发送HEAD和GET请求
            timeout(Duration::from_secs(timeout_secs), async {
                // 先发送HEAD请求检查连接性
                let resp = client.head("http://www.baidu.com")
                    .send()
                    .await?;
                
                if !resp.status().is_success() {
                    return Err(anyhow::anyhow!("HTTP状态码错误: {}", resp.status()));
                }
                
                // 如果HEAD请求成功，再发送GET请求测试实际访问
                let resp = client.get("http://www.baidu.com")
                    .send()
                    .await?;
                
                if !resp.status().is_success() {
                    return Err(anyhow::anyhow!("HTTP状态码错误: {}", resp.status()));
                }
                
                // 确保能读取响应内容
                let _body = resp.bytes().await?;
                Ok::<_, anyhow::Error>(())
            }).await??;
        }
        
        Ok(start.elapsed())
    }

    // 测试代理有效性（初始加载和健康检查共用）
    pub async fn test_proxies<I, F, T>(&self, 
        proxies: I, 
        test_name: &str, 
        timeout: u64,
        fast_check: bool,
        show_progress: bool,
        each_item: F
    ) -> Vec<ProxyEntry> 
    where 
        I: IntoIterator<Item = T>,
        F: Fn(T) -> (String, Option<ProxyEntry>) + Send + Sync + 'static,
        T: Send + Sync + 'static
    {
        let proxies: Vec<T> = proxies.into_iter().collect();
        let total = proxies.len();
        
        if total == 0 {
            return Vec::new();
        }
        
        let max_concurrency = self.config.proxy.max_concurrency;
        
        println!("{} {} {}", 
            format!("开始{}...", test_name).cyan().bold(),
            format!("共{}个代理", total).yellow().bold(),
            format!("并发数: {}", max_concurrency).green().bold()
        );
        
        // 创建进度条
        let pb = if show_progress {
            let pb = ProgressBar::new(total as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("#>-"));
            Some(Arc::new(pb))
        } else {
            None
        };
        
        // 创建信号量控制并发数
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
        let valid_proxies = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let mut handles = Vec::with_capacity(total);
        
        for proxy in proxies {
            let semaphore = semaphore.clone();
            let pb = pb.clone();
            let valid_proxies = valid_proxies.clone();
            let (addr, entry) = each_item(proxy);
            
            let handle = tokio::spawn(async move {
                // 获取信号量许可
                let _permit = semaphore.acquire().await.unwrap();
                
                // 测试代理
                let result = Self::test_proxy(&addr, timeout, fast_check).await;
                
                // 更新进度条
                if let Some(pb) = &pb {
                    pb.inc(1);
                }
                
                // 如果测试成功，添加到有效代理列表
                if let Ok(latency) = result {
                    let mut proxies = valid_proxies.lock().await;
                    if let Some(mut old_entry) = entry {
                        // 更新现有条目
                        old_entry.latency = latency;
                        old_entry.last_check = Instant::now();
                        old_entry.fail_count = 0;
                        proxies.push(old_entry);
                    } else {
                        // 创建新条目
                        proxies.push(ProxyEntry {
                            address: addr,
                            latency,
                            last_check: Instant::now(),
                            fail_count: 0,
                        });
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // 等待所有测试完成
        for handle in handles {
            let _ = handle.await;
        }
        
        // 结束进度条
        if let Some(pb) = pb {
            pb.finish_with_message(format!("{}完成", test_name));
        }
        
        // 获取有效代理并排序
        let mut proxies = Arc::try_unwrap(valid_proxies)
            .expect("获取有效代理失败")
            .into_inner();
            
        // 按延迟排序
        proxies.sort_by(|a, b| a.latency.cmp(&b.latency));
        
        proxies
    }
    
    pub async fn load_from_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        // 读取代理文件
        let file = File::open(&path)?;
        let reader = io::BufReader::new(file);
        let mut proxies = HashSet::new();

        // 读取并去重代理地址
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                proxies.insert(line.trim().to_string());
            }
        }
        
        if proxies.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "代理文件为空"));
        }
        
        let total = proxies.len();
        
        // 测试代理
        let valid_proxies = self.test_proxies(
            proxies, 
            "代理测试", 
            self.config.proxy.test_timeout, 
            false, 
            true,
            |addr| (addr, None)
        ).await;
        
        // 更新代理列表
        let mut pool = self.proxies.write().await;
        *pool = valid_proxies.clone();

        // 更新文件中的代理列表（只保留有效代理）
        let valid_proxies_str: Vec<String> = valid_proxies.iter()
            .map(|p| p.address.clone())
            .collect();
        fs::write(&path, valid_proxies_str.join("\n"))?;

        println!("\n{} {} {}", 
            "测试完成，可用代理:".green().bold(), 
            valid_proxies.len().to_string().yellow().bold(),
            "个".green().bold()
        );
        
        let invalid_count = total - valid_proxies.len();
        if invalid_count > 0 {
            println!("{} {} {}", 
                "已删除无效代理:".yellow().bold(),
                invalid_count.to_string().red().bold(),
                "个".yellow().bold()
            );
        }
        
        // 显示延迟信息
        for (i, proxy) in valid_proxies.iter().enumerate() {
            let latency = proxy.latency.as_millis();
            let latency_str = match latency {
                0..=100 => latency.to_string().green(),
                101..=300 => latency.to_string().yellow(),
                _ => latency.to_string().red(),
            };
            println!("{:3}. {} - {}ms", 
                (i + 1).to_string().blue().bold(),
                proxy.address.cyan(),
                latency_str
            );
        }
        println!();

        // 启动健康检查任务
        self.start_health_check();

        Ok(())
    }

    // 启动健康检查
    fn start_health_check(&self) {
        let pool = Arc::clone(&self.proxies);
        let config = Arc::clone(&self.config);
        let proxy_file = Arc::clone(&self.proxy_file);
        let self_clone = Arc::new(self.clone());
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(config.proxy.health_check_interval)).await;
                
                let proxies = pool.read().await;
                if proxies.is_empty() {
                    continue;
                }
                
                // 复制代理列表用于检查
                let proxies_to_check: Vec<ProxyEntry> = proxies.clone();
                let total = proxies_to_check.len();
                drop(proxies); // 释放读锁
                
                // 健康检查
                let valid_proxies = self_clone.test_proxies(
                    proxies_to_check,
                    "健康检查",
                    3, // 健康检查超时时间
                    true, // 快速检查
                    false, // 不显示进度条
                    |entry| (entry.address.clone(), Some(entry))
                ).await;
                
                // 更新代理池
                let mut pool_write = pool.write().await;
                *pool_write = valid_proxies.clone();
                
                // 更新文件中的代理列表
                if !valid_proxies.is_empty() {
                    let valid_proxies_str: Vec<String> = valid_proxies.iter()
                        .map(|p| p.address.clone())
                        .collect();
                    if let Err(e) = fs::write(&*proxy_file, valid_proxies_str.join("\n")) {
                        eprintln!("{} {}", "更新代理文件失败:".red().bold(), e);
                    }
                }
                
                let removed_count = total - valid_proxies.len();
                if removed_count > 0 {
                    println!("{} {}", "已移除失效代理:".yellow().bold(), removed_count.to_string().red().bold());
                }
                
                println!("{} {}", "健康检查完成，当前可用代理:".green().bold(), valid_proxies.len().to_string().yellow().bold());
            }
        });
    }

    pub async fn get_current_proxy(&self) -> Option<ProxyEntry> {
        let proxies = self.proxies.read().await;
        let index = *self.current_index.read().await;
        proxies.get(index).cloned()
    }

    pub async fn next_proxy(&self) -> Option<ProxyEntry> {
        let mut index = self.current_index.write().await;
        let proxies = self.proxies.read().await;
        
        if proxies.is_empty() {
            return None;
        }

        *index = (*index + 1) % proxies.len();
        proxies.get(*index).cloned()
    }

    pub async fn choose_proxy(&self, index : usize) -> Option<ProxyEntry> {
        let proxies = self.proxies.read().await;
        let mut current_index = self.current_index.write().await;
        
        if proxies.is_empty() {
            return None;
        }

        *current_index = (index - 1) % proxies.len();
        proxies.get(*current_index).cloned()
    }

    pub async fn list_proxies(&self) -> Vec<ProxyEntry> {
        self.proxies.read().await.clone()
    }
}

// 添加Clone实现，用于健康检查
impl Clone for ProxyPool {
    fn clone(&self) -> Self {
        ProxyPool {
            proxies: Arc::new(RwLock::new(Vec::new())),
            current_index: Arc::new(RwLock::new(0)),
            config: self.config.clone(),
            proxy_file: self.proxy_file.clone(),
        }
    }
} 