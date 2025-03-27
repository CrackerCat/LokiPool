# LokiPool

<div align="center">

<div align="center">
  <img src="logo.png" alt="Logo" width="150" height="150">
</div>

</br>

![Version](https://img.shields.io/badge/版本-0.1.4-blue)
![Language](https://img.shields.io/badge/语言-Rust-orange)
![License](https://img.shields.io/badge/许可证-GPL-green)

**高性能SOCKS5代理池管理器，支持智能测速与自动切换**

</div>

## 📖 项目概述

LokiPool是一个使用Rust编写的高性能SOCKS5代理池管理工具，能够自动测速、管理多个代理服务器，提供高效稳定的匿名代理服务。通过本地转发，让您使用最佳的代理服务，同时保持稳定的连接质量。

## ✨ 主要功能

- **🚀 本地SOCKS5服务** - 在本地开放自定义端口，提供稳定的SOCKS5代理服务
- **⚡ 智能代理选择** - 基于延迟自动选择最快的代理服务器
- **🔍 健康监测** - 定期测试代理列表的连通性和速度，移除不可用代理
- **⏱️ 延迟排序** - 根据对百度的访问延迟，对代理进行智能排序
- **💻 交互式管理** - 支持通过命令行实时查看和管理代理状态
- **🔄 自动切换** - 可配置自动定时切换代理，增强匿名性
- **🔎 自动获取代理** - 支持从FOFA、Hunter和Quake自动获取最新代理
- **⚙️ 并发控制** - 智能控制代理测试的并发数，提高效率

## 🚀 安装方法

### 预编译二进制文件

从[Releases页面](https://github.com/Le1a/LokiPool/releases)下载适合您系统的预编译二进制文件：

- `lokipool-linux-x86_64` - Linux (64位)
- `lokipool-windows-x86_64.exe` - Windows (64位)
- `lokipool-macos-x86_64` - macOS (Intel)
- `lokipool-macos-arm64` - macOS (Apple Silicon)

### 从源码编译

```bash
# 克隆仓库
git clone https://github.com/Le1a/LokiPool.git
cd LokiPool

# 编译
cargo build --release

# 运行
./target/release/lokipool
```

## 📝 使用方法

1. 在`proxies.txt`文件中添加SOCKS5代理服务器地址（每行一个，格式：`IP:端口`）
2. 或启用自动代理获取功能，从FOFA/Hunter/Quake获取最新代理
3. 运行程序，将自动测试代理速度并启动本地代理服务
4. 配置您的应用程序使用本地SOCKS5代理（默认`127.0.0.1:1080`）

### 交互命令

| 命令 | 描述 |
|------|------|
| `help` | 显示帮助信息 |
| `show` | 显示当前使用的代理及其延迟 |
| `next` | 手动切换到下一个代理 |
| `goto <序号>` |  切换到对应代理节点 |
| `list` | 显示所有可用代理及其延迟排序 |
| `ping` | 延迟检测 |
| `quit` | 退出程序 |

## ⚙️ 配置说明

在`config.toml`文件中可以自定义以下配置：

### 服务器配置

```toml
[server]
bind_host = "127.0.0.1"  # 本地绑定地址
bind_port = 1080         # 本地绑定端口
max_connections = 100    # 最大连接数
```

### 代理配置

```toml
[proxy]
proxy_file = "proxies.txt"       # 代理列表文件
test_timeout = 5                 # 代理测试超时时间(秒)
health_check_interval = 300      # 健康检测间隔(秒)
retry_times = 3                  # 失败重试次数
auto_switch = false              # 是否自动切换代理
switch_interval = 5              # 自动切换间隔(秒)
max_concurrency = 100            # 代理测试最大并发数
```

### 日志配置

```toml
[log]
show_connection_log = false      # 是否显示连接日志
show_error_log = false           # 是否显示错误日志
```

### 代理获取配置
**如果没有Quake积分可通过填写邀请码获取：
个人中心输入邀请码“PSZZ90”你我均可获得5,000长效积分哦，地址 quake.360.net**

```toml
# FOFA API配置
[fofa]
switch = false                   # 是否启用FOFA获取代理
api_url = 'https://fofa.info/api/v1/search/all'
fofa_key = '你的FOFA_KEY'
query_str = '(protocol=="socks5" && country="CN" && banner="Method:No Authentication") && after="2025-02-25"' # 时间可以自定义
size = 10000                     # 获取代理数量

# Quake API配置
[quake]
switch = false                   # 是否启用Quake获取代理
api_url = 'https://quake.360.net/api/v3/search/quake_service'
quake_key = '你的QUAKE_KEY'
query_str = 'service:socks5 AND country: "CN" AND response:"No authentication"'
size = 500                       # 获取代理数量

# Hunter API配置
[hunter]
switch = false                   # 是否启用Hunter获取代理
api_url = 'https://hunter.qianxin.com/openApi/search'
hunter_key = '你的HUNTER_KEY'
query_str = 'protocol=="socks5"&&protocol.banner="No authentication"&&ip.country="CN"'
size = 4                         # 获取页数，每页100条
```

## 🔧 高级用法

### 代理服务集成

LokiPool可以轻松与各种应用程序集成：

- **浏览器**: 在网络设置中配置SOCKS5代理
- **命令行工具**: 设置`ALL_PROXY`环境变量
- **开发环境**: 配置包管理器和开发工具使用代理

### 性能优化

- 增加`max_connections`值以支持更多并发连接
- 调整`health_check_interval`减少服务器负担
- 根据网络环境调整`test_timeout`获得更准确的延迟测试
- 调整`max_concurrency`优化代理测试性能和资源占用

### 自动获取代理

1. 配置至少一个API源（FOFA/Quake/Hunter）
2. 设置相应的API Key和查询语句
3. 将对应的`switch`选项设为`true`
4. 程序将在代理文件为空时自动获取新代理

## 📜 许可证

GPL License

## 🤝 贡献

欢迎提交PR和Issue，一起改进LokiPool！

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Le1a/LokiPool&type=Date)](https://star-history.com/#Le1a/LokiPool&Date)

---

<div align="center">
<i>让您的网络连接更快、更安全、更匿名</i>
</div>
