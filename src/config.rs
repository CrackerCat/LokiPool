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
    pub health_check_interval: u64,
    pub retry_times: u32,
    pub auto_switch: bool,
    pub switch_interval: u64,
    pub max_concurrency: usize,
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

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                bind_host: "127.0.0.1".to_string(),
                bind_port: 1080,
                max_connections: 100,
            },
            proxy: ProxyConfig {
                proxy_file: "proxies.txt".to_string(),
                test_timeout: 5,
                health_check_interval: 300,
                retry_times: 3,
                auto_switch: false,
                switch_interval: 300,
                max_concurrency: 50,
            },
            log: LogConfig {
                show_connection_log: true,
                show_error_log: false,
            },
            fofa: FofaConfig {
                switch: false,
                api_url: "".to_string(),
                fofa_key: "".to_string(),
                query_str: "".to_string(),
                size: 100,
            },
            quake: QuakeConfig {
                switch: false,
                api_url: "https://quake.360.net/api/v3/search/quake_service".to_string(),
                quake_key: "".to_string(),
                query_str: "service:socks5 AND country: \"CN\" AND response:\"No authentication\"".to_string(),
                size: 500,
            },
            hunter: HunterConfig {
                switch: false,
                api_url: "".to_string(),
                hunter_key: "".to_string(),
                query_str: "".to_string(),
                size: 100,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Path::new("config.toml");
        
        if !config_path.exists() {
            let config = Config::default();
            let toml = toml::to_string_pretty(&config)?;
            fs::write(config_path, toml)?;
            return Ok(config);
        }
        
        let content = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
} 