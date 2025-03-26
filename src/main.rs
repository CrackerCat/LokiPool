use tokio::io::{self, AsyncBufReadExt, BufReader};
use std::io::Write;
use anyhow::Result;
use lokipool::{Config, SocksServer};
use tokio::signal;
use colored::*;
use std::path::Path;
use std::fs;
use std::fs::File;

const LOGO: &str = r#"
██╗      ██████╗ ██╗  ██╗██╗██████╗  ██████╗  ██████╗ ██╗     
██║     ██╔═══██╗██║ ██╔╝██║██╔══██╗██╔═══██╗██╔═══██╗██║     
██║     ██║   ██║█████╔╝ ██║██████╔╝██║   ██║██║   ██║██║     
██║     ██║   ██║██╔═██╗ ██║██╔═══╝ ██║   ██║██║   ██║██║     
███████╗╚██████╔╝██║  ██╗██║██║     ╚██████╔╝╚██████╔╝███████╗
╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═╝╚═╝      ╚═════╝  ╚═════╝ ╚══════╝
"#;

const VERSION: &str = "v0.1.3";

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 显示Logo和版本信息
    println!("{}", LOGO.bright_cyan());
    println!("{}", "A Fast and Reliable SOCKS5 Proxy Pool".bright_black());
    println!("{} {}", "Version:".bright_black(), VERSION.bright_black());
    println!("{} {}", "Author:".bright_black(), "Le1a".bright_black());
    println!("{} {}\n", "GitHub:".bright_black(), "https://github.com/Le1a/LokiPool".bright_blue().underline());

    // 加载配置
    let config = match Config::load() {
        Ok(cfg) => {
            println!("{}", "成功加载配置文件".green().bold());
            cfg
        }
        Err(e) => {
            eprintln!("{} {}", "加载配置文件失败:".red().bold(), e);
            return Ok(());
        }
    };

    // 创建SOCKS5服务器
    let server = SocksServer::new(config.clone());
    println!("\n{}", "创建SOCKS5服务器...".cyan().bold());
    
    // 显示自动切换配置
    if config.proxy.auto_switch {
        println!("{} {} {}", 
            "自动切换已开启,间隔:".green().bold(),
            config.proxy.switch_interval.to_string().yellow().bold(),
            "秒".green().bold()
        );
    }
    
    // 检查代理文件是否存在
    let proxy_file = config.proxy.proxy_file.clone();
    if !Path::new(&proxy_file).exists() {
        println!("{} {}", "代理文件不存在，正在创建:".yellow().bold(), &proxy_file);
        match File::create(&proxy_file) {
            Ok(_) => println!("{}", "创建代理文件成功".green().bold()),
            Err(e) => {
                eprintln!("{} {}", "创建代理文件失败:".red().bold(), e);
                return Ok(());
            }
        }
    }
    
    // 检查代理文件是否为空
    let is_empty = match fs::metadata(&proxy_file) {
        Ok(metadata) => metadata.len() == 0,
        Err(e) => {
            eprintln!("{} {}", "读取代理文件失败:".red().bold(), e);
            return Ok(());
        }
    };
    
    // 如果文件为空，从配置的源获取代理
    if is_empty {
        println!("{}", "代理文件为空".yellow().bold());
        
        // 检查是否有任何代理源开启
        if config.fofa.switch || config.quake.switch || config.hunter.switch {
            println!("{}", "尝试从配置的API获取代理...".cyan().bold());
            match lokipool::crawler::fetch_proxies(&config).await {
                Ok(_) => println!("{}", "从API获取代理成功".green().bold()),
                Err(e) => {
                    eprintln!("{} {}", "从API获取代理失败:".red().bold(), e);
                    return Ok(());
                }
            }
        } else {
            eprintln!("{}", "代理文件内容为空且自动爬取功能未配置".red().bold());
            return Ok(());
        }
    }
    
    // 加载代理列表
    if let Err(e) = server.get_proxy_pool().load_from_file(&proxy_file).await {
        eprintln!("{} {}", "加载代理列表失败:".red().bold(), e);
        return Ok(());
    }
    
    // 创建用户输入处理任务
    let server_clone = server.clone();
    let input_handle = tokio::spawn(async move {
        let (host, port) = server_clone.get_bind_info();
        println!("\n{} {}:{}", 
            "代理服务器已启动在".green().bold(),
            host,
            port
        );
        println!("\n可用命令:");
        println!("  list  - 显示所有代理");
        println!("  next  - 切换到下一个代理");
        println!("  show  - 显示当前代理");
        println!("  quit  - 退出程序\n");
        
        print!("> ");
        let _ = std::io::stdout().flush();
        
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            match line.trim() {
                "list" => {
                    println!("\n当前代理列表:");
                    for (i, proxy) in server_clone.get_proxy_pool().list_proxies().await.iter().enumerate() {
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
                }
                "next" => {
                    if let Some(proxy) = server_clone.get_proxy_pool().next_proxy().await {
                        println!("{} {} ({}: {}ms)", 
                            "切换到代理:".green().bold(),
                            proxy.address.cyan(),
                            "延迟".yellow(),
                            proxy.latency.as_millis().to_string().yellow()
                        );
                    } else {
                        println!("{}", "没有可用的代理".red().bold());
                    }
                }
                "show" => {
                    if let Some(proxy) = server_clone.get_proxy_pool().get_current_proxy().await {
                        println!("{} {} ({}: {}ms)", 
                            "当前代理:".green().bold(),
                            proxy.address.cyan(),
                            "延迟".yellow(),
                            proxy.latency.as_millis().to_string().yellow()
                        );
                    } else {
                        println!("{}", "没有可用的代理".red().bold());
                    }
                }
                "quit" => break,
                "" => {}, // 忽略空行
                _ => println!("{}", "未知命令".red()),
            }
            print!("> ");
            let _ = std::io::stdout().flush();
        }
    });

    // 启动服务器
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("{} {}", "服务器错误:".red().bold(), e);
        }
    });

    // 等待Ctrl+C信号或用户输入quit
    tokio::select! {
        _ = signal::ctrl_c() => {
            println!("\n{}", "接收到终止信号，正在关闭服务器...".yellow().bold());
        }
        _ = input_handle => {
            println!("{}", "用户请求退出，正在关闭服务器...".yellow().bold());
        }
    }

    // 中止服务器任务
    server_handle.abort();
    println!("{}", "服务器已关闭".green().bold());

    Ok(())
}
