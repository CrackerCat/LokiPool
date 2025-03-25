use crate::config::Config;
use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use serde::Deserialize;
use serde_json;
use std::fs::OpenOptions;
use std::io::Write;
use colored::*;

// FOFA API响应结构
#[derive(Debug, Deserialize)]
pub struct FofaResponse {
    pub error: bool,
    pub results: Vec<Vec<String>>,
}

// Quake API响应结构
#[derive(Debug, Deserialize)]
pub struct QuakeResponse {
    pub code: i32,
    pub message: String,
    pub data: Vec<QuakeItem>,
}

#[derive(Debug, Deserialize)]
pub struct QuakeItem {
    pub ip: String,
    pub port: u32,
}

// Hunter API响应结构
#[derive(Debug, Deserialize)]
pub struct HunterResponse {
    pub code: u32,
    pub message: String,
    pub data: HunterData,
}

#[derive(Debug, Deserialize)]
pub struct HunterData {
    pub total: u64,
    pub arr: Vec<HunterItem>,
}

#[derive(Debug, Deserialize)]
pub struct HunterItem {
    pub ip: String,
    pub port: u32,
}

pub async fn fetch_proxies(config: &Config) -> Result<()> {
    let mut proxies = Vec::new();
    let mut fetch_success = false;
    
    // 从FOFA获取代理
    if config.fofa.switch {
        match fetch_from_fofa(config).await {
            Ok(fofa_proxies) => {
                println!("{} {}", "从FOFA获取代理成功:".green().bold(), fofa_proxies.len().to_string().yellow().bold());
                proxies.extend(fofa_proxies);
                fetch_success = true;
            },
            Err(e) => {
                eprintln!("{} {}", "从FOFA获取代理失败:".red().bold(), e);
            }
        }
    }

    // 从Quake获取代理
    if config.quake.switch {
        match fetch_from_quake(config).await {
            Ok(quake_proxies) => {
                println!("{} {}", "从Quake获取代理成功:".green().bold(), quake_proxies.len().to_string().yellow().bold());
                proxies.extend(quake_proxies);
                fetch_success = true;
            },
            Err(e) => {
                eprintln!("{} {}", "从Quake获取代理失败:".red().bold(), e);
            }
        }
    }
    
    // 从Hunter获取代理
    if config.hunter.switch {
        match fetch_from_hunter(config).await {
            Ok(hunter_proxies) => {
                println!("{} {}", "从Hunter获取代理成功:".green().bold(), hunter_proxies.len().to_string().yellow().bold());
                proxies.extend(hunter_proxies);
                fetch_success = true;
            },
            Err(e) => {
                eprintln!("{} {}", "从Hunter获取代理失败:".red().bold(), e);
            }
        }
    }
    
    if !fetch_success {
        return Err(anyhow::anyhow!("所有代理源获取失败"));
    }
    
    // 去重
    proxies.sort();
    proxies.dedup();
    
    // 写入文件
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(&config.proxy.proxy_file)
        .map_err(|e| anyhow::anyhow!("打开代理文件失败: {}", e))?;
    
    for proxy in &proxies {
        writeln!(file, "{}", proxy)
            .map_err(|e| anyhow::anyhow!("写入代理文件失败: {}", e))?;
    }
    
    println!("{} {}", "共获取并保存代理:".green().bold(), proxies.len().to_string().yellow().bold());
    Ok(())
}

async fn fetch_from_fofa(config: &Config) -> Result<Vec<String>> {
    println!("{}", "从FOFA API获取代理列表...".cyan().bold());

    let query_base64 = general_purpose::STANDARD.encode(&config.fofa.query_str);
    
    let url = format!(
        "{}?key={}&qbase64={}&size={}",
        config.fofa.api_url,
        config.fofa.fofa_key,
        query_base64,
        config.fofa.size
    );

    let client = reqwest::Client::new();
    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("发送FOFA API请求失败: {}", e))?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("FOFA API请求失败: HTTP状态码 {}", response.status()));
    }
    
    let fofa_data: FofaResponse = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("解析FOFA API响应失败: {}", e))?;
    
    if fofa_data.error {
        return Err(anyhow::anyhow!("FOFA API返回错误"));
    }

    let mut proxies = Vec::new();
    for proxy in fofa_data.results {
        if proxy.len() >= 1 {
            proxies.push(proxy[0].clone());
        }
    }

    Ok(proxies)
}

async fn fetch_from_quake(config: &Config) -> Result<Vec<String>> {
    println!("{}", "从Quake API获取代理列表...".cyan().bold());

    let url = &config.quake.api_url;
    let client = reqwest::Client::new();
    
    // 准备请求体
    let request_body = serde_json::json!({
        "query": config.quake.query_str,
        "latest": "True",
        "start": 0,
        "size": config.quake.size,
        "include": ["ip", "port"]
    });
    
    let response = client.post(url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/83.0.4103.116 Safari/537.36")
        .header("X-QuakeToken", &config.quake.quake_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("发送Quake API请求失败: {}", e))?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Quake API请求失败: HTTP状态码 {}", response.status()));
    }
    
    let quake_data: QuakeResponse = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("解析Quake API响应失败: {}", e))?;
    
    if quake_data.code != 0 {
        return Err(anyhow::anyhow!("Quake API返回错误: {}", quake_data.message));
    }

    let mut proxies = Vec::new();
    for item in quake_data.data {
        let proxy = format!("{}:{}", item.ip, item.port);
        proxies.push(proxy);
    }
    
    println!("{} {}", 
        "从Quake获取代理数量:".green().bold(),
        proxies.len().to_string().yellow().bold()
    );

    Ok(proxies)
}

async fn fetch_from_hunter(config: &Config) -> Result<Vec<String>> {
    println!("{}", "从Hunter API获取代理列表...".cyan().bold());

    let query_base64 = general_purpose::STANDARD.encode(&config.hunter.query_str);
    let mut all_proxies = Vec::new();
    
    // 遍历所有页
    for page in 1..=config.hunter.size {
        let url = format!(
            "{}?api-key={}&search={}&page={}&page_size=100",
            config.hunter.api_url,
            config.hunter.hunter_key,
            query_base64,
            page
        );
        
        let client = reqwest::Client::new();
        let response = client.get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("发送Hunter API请求失败 (第{}页): {}", page, e))?;
        
        if !response.status().is_success() {
            eprintln!("{} {}", format!("Hunter API请求第{}页失败: HTTP状态码", page).red().bold(), response.status());
            continue; // 继续下一页而不是完全中止
        }
        
        let hunter_data: HunterResponse = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                eprintln!("{} {}", format!("解析Hunter API响应失败 (第{}页):", page).red().bold(), e);
                continue; // 继续下一页
            }
        };
        
        if hunter_data.code != 200 {
            eprintln!("{} {}", format!("Hunter API返回错误 (第{}页):", page).red().bold(), hunter_data.message);
            continue; // 继续下一页
        }
        
        // 提取代理
        for item in hunter_data.data.arr {
            let proxy = format!("{}:{}", item.ip, item.port);
            all_proxies.push(proxy);
        }
        
        // 添加延迟，防止API限流
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
    
    Ok(all_proxies)
} 