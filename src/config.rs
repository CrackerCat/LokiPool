use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub proxy: ProxyConfig,
    pub log: LogConfig,
    pub fofa: FofaConfig,
    pub quake: QuakeConfig,
    pub hunter: HunterConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub max_connections: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    pub proxy_file: String,
    pub test_timeout: u64,
    pub health_check_switch: bool,
    pub health_check_interval: u64,
    pub retry_times: u32,
    pub auto_switch: bool,
    pub switch_interval: u64,
    pub max_concurrency: usize,
    pub use_auth: bool,          // 是否使用代理认证
    pub username: String,        // 代理认证用户名
    pub password: String,        // 代理认证密码
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogConfig {
    pub show_connection_log: bool,
    pub show_error_log: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FofaConfig {
    pub switch: bool,
    pub api_url: String,
    pub fofa_key: String,
    pub query_str: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuakeConfig {
    pub switch: bool,
    pub api_url: String,
    pub quake_key: String,
    pub query_str: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HunterConfig {
    pub switch: bool,
    pub api_url: String,
    pub hunter_key: String,
    pub query_str: String,
    pub size: u64,
}

// 硬编码的默认配置字符串
const DEFAULT_CONFIG: &str = r#"[server]
bind_host = "127.0.0.1"
bind_port = 1080
max_connections = 100

[proxy]
proxy_file = "proxies.txt"
test_timeout = 5
health_check_switch = true  # 是否启用健康检查
health_check_interval = 300 # 健康检测间隔(秒)
retry_times = 3
auto_switch = false        # 是否开启自动切换代理
switch_interval = 300      # 自动切换间隔(秒)
max_concurrency = 100     # 最大并发测试数
use_auth = false          # 是否使用代理认证
username = ""             # 代理认证用户名
password = ""             # 代理认证密码

[log]
show_connection_log = false  # 设置为 false 可以关闭连接日志
show_error_log = false      # 设置为 false 可以关闭错误日志

[fofa]
switch = false
api_url = 'https://fofa.info/api/v1/search/all'
fofa_key = '186******f8a******6a92******4abf1c' # 替换成自己的key
query_str = '(protocol=="socks5" && country="CN" && banner="Method:No Authentication") && after="2025-02-25"' # 这里可以用after添加时间限制，过滤不可用的代理
size = 10000 # 这里是获取的条数

[quake]
switch = false
api_url = 'https://quake.360.net/api/v3/search/quake_service'
quake_key = '0e****-3***-4***-a***-5a21********' # 替换成自己的key
query_str = 'service:socks5 AND country: "CN" AND response:"No authentication"'
size = 500 # 这里是获取的条数

[hunter]
switch = false
api_url = 'https://hunter.qianxin.com/openApi/search'
hunter_key = '365*******9ab9*******b0f0*******d1cd0d3399' # 替换成自己的key
query_str = 'protocol=="socks5"&&protocol.banner="No authentication"&&ip.country="CN"'
size = 4 # 这里是指页数，一页100条
"#;

impl Default for Config {
    fn default() -> Self {
        // 从硬编码字符串解析默认配置
        // 注意：toml库能够处理带有注释的配置文件
        match toml::from_str(DEFAULT_CONFIG) {
            Ok(config) => config,
            Err(e) => {
                // 如果解析失败，打印错误并使用硬编码的备选配置
                eprintln!("错误：无法解析默认配置字符串: {}", e);
                Config {
                    server: ServerConfig {
                        bind_host: "127.0.0.1".to_string(),
                        bind_port: 1080,
                        max_connections: 100,
                    },
                    proxy: ProxyConfig {
                        proxy_file: "proxies.txt".to_string(),
                        test_timeout: 5,
                        health_check_switch: true,
                        health_check_interval: 300,
                        retry_times: 3,
                        auto_switch: false,
                        switch_interval: 300,
                        max_concurrency: 100,
                        use_auth: false,
                        username: String::new(),
                        password: String::new(),
                    },
                    log: LogConfig {
                        show_connection_log: false,
                        show_error_log: false,
                    },
                    fofa: FofaConfig {
                        switch: false,
                        api_url: "https://fofa.info/api/v1/search/all".to_string(),
                        fofa_key: "186******f8a******6a92******4abf1c".to_string(),
                        query_str: "(protocol==\"socks5\" && country=\"CN\" && banner=\"Method:No Authentication\") && after=\"2025-02-25\"".to_string(),
                        size: 10000,
                    },
                    quake: QuakeConfig {
                        switch: false,
                        api_url: "https://quake.360.net/api/v3/search/quake_service".to_string(),
                        quake_key: "0e****-3***-4***-a***-5a21********".to_string(),
                        query_str: "service:socks5 AND country: \"CN\" AND response:\"No authentication\"".to_string(),
                        size: 500,
                    },
                    hunter: HunterConfig {
                        switch: false,
                        api_url: "https://hunter.qianxin.com/openApi/search".to_string(),
                        hunter_key: "365*******9ab9*******b0f0*******d1cd0d3399".to_string(),
                        query_str: "protocol==\"socks5\"&&protocol.banner=\"No authentication\"&&ip.country=\"CN\"".to_string(),
                        size: 4,
                    },
                }
            }
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Path::new("config.toml");
        
        if !config_path.exists() {
            let config = Config::default();
            fs::write(config_path, DEFAULT_CONFIG)?;
            return Ok(config);
        }
        
        let content = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
} 