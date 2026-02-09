use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkAdapter {
    pub name: String,
    pub ip_addresses: Vec<String>,
    pub has_ipv4: bool,
    pub has_ipv6: bool,
    pub has_ipv4_global: bool,
    pub has_ipv6_global: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvironmentCheckResult {
    pub adapters: Vec<NetworkAdapter>,
    pub ipv4_connectivity: bool,
    pub ipv6_connectivity: bool,
    pub dns_resolution: bool,
    pub internet_available: bool,
    pub error_messages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpPingResult {
    pub url: String,
    pub ip_address: Option<String>,
    pub status_code: Option<u16>,
    pub response_time_ms: Option<u64>,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DnsResolution {
    pub ipv4_addresses: Vec<String>,
    pub ipv6_addresses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpPingDualResult {
    pub url: String,
    pub dns_resolution: DnsResolution,
    pub ipv4: HttpPingResult,
    pub ipv6: HttpPingResult,
}

#[tauri::command]
async fn environment_check() -> Result<EnvironmentCheckResult, String> {
    let mut result = EnvironmentCheckResult {
        adapters: vec![],
        ipv4_connectivity: false,
        ipv6_connectivity: false,
        dns_resolution: false,
        internet_available: false,
        error_messages: vec![],
    };

    // ネットワークアダプタの取得
    match get_network_interfaces() {
        Ok(adapters) => {
            result.adapters = adapters;
        }
        Err(e) => {
            result
                .error_messages
                .push(format!("ネットワークアダプタの取得に失敗: {}", e));
        }
    }

    // IPv4接続確認
    match check_ipv4_connectivity().await {
        Ok(connected) => {
            result.ipv4_connectivity = connected;
        }
        Err(e) => {
            result
                .error_messages
                .push(format!("IPv4接続確認に失敗: {}", e));
        }
    }

    // IPv6接続確認
    match check_ipv6_connectivity().await {
        Ok(connected) => {
            result.ipv6_connectivity = connected;
        }
        Err(e) => {
            result
                .error_messages
                .push(format!("IPv6接続確認に失敗: {}", e));
        }
    }

    // DNS解決確認
    match check_dns_resolution().await {
        Ok(resolved) => {
            result.dns_resolution = resolved;
        }
        Err(e) => {
            result
                .error_messages
                .push(format!("DNS解決確認に失敗: {}", e));
        }
    }

    // インターネット接続判定
    result.internet_available = (result.ipv4_connectivity || result.ipv6_connectivity)
        && result.dns_resolution;

    Ok(result)
}

#[tauri::command]
async fn ping_http(
    url: String,
    ignore_tls_errors: bool,
) -> Result<HttpPingResult, String> {
    let start = Instant::now();

    // URLの検証
    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            return Ok(HttpPingResult {
                url: url.clone(),
                ip_address: None,
                status_code: None,
                response_time_ms: None,
                success: false,
                error_message: Some(format!("無効なURL: {}", e)),
            });
        }
    };

    // HTTPクライアントの構築
    let client = if ignore_tls_errors {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(30))
            .build()
    } else {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
    };

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return Ok(HttpPingResult {
                url: url.clone(),
                ip_address: None,
                status_code: None,
                response_time_ms: None,
                success: false,
                error_message: Some(format!("HTTPクライアント作成失敗: {}", e)),
            });
        }
    };

    // HTTPリクエスト
    let response = match client.get(parsed_url.as_str()).send().await {
        Ok(resp) => resp,
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            return Ok(HttpPingResult {
                url: url.clone(),
                ip_address: None,
                status_code: None,
                response_time_ms: Some(elapsed),
                success: false,
                error_message: Some(format!("接続エラー: {}", e)),
            });
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    let success = response.status().is_success();

    Ok(HttpPingResult {
        url: url.clone(),
        ip_address: None,
        status_code: Some(status_code),
        response_time_ms: Some(elapsed),
        success,
        error_message: if success {
            None
        } else {
            Some(format!("HTTPステータス: {}", status_code))
        },
    })
}

#[tauri::command]
async fn ping_http_dual(
    url: String,
    ignore_tls_errors: bool,
) -> Result<HttpPingDualResult, String> {
    // URLの検証
    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            return Err(format!("無効なURL: {}", e));
        }
    };

    let host = match parsed_url.host_str() {
        Some(h) => h,
        None => return Err("URLからホスト名を抽出できません".to_string()),
    };

    // ステップ1: DNS名前解決 (IPv4とIPv6)
    let dns_result = resolve_dns(host).await;
    let ipv4_addresses = dns_result.ipv4_addresses.clone();
    let ipv6_addresses = dns_result.ipv6_addresses.clone();

    // ステップ2: IPv4アドレスへのHTTP接続
    let ipv4_result = if !ipv4_addresses.is_empty() {
        connect_to_ip_with_host(
            url.clone(),
            &ipv4_addresses[0],
            host,
            ignore_tls_errors,
            parsed_url.port(),
        )
        .await
    } else {
        HttpPingResult {
            url: url.clone(),
            ip_address: None,
            status_code: None,
            response_time_ms: None,
            success: false,
            error_message: Some("IPv4アドレスが見つかりません".to_string()),
        }
    };

    // ステップ3: IPv6アドレスへのHTTP接続
    let ipv6_result = if !ipv6_addresses.is_empty() {
        connect_to_ip_with_host(
            url.clone(),
            &ipv6_addresses[0],
            host,
            ignore_tls_errors,
            parsed_url.port(),
        )
        .await
    } else {
        HttpPingResult {
            url: url.clone(),
            ip_address: None,
            status_code: None,
            response_time_ms: None,
            success: false,
            error_message: Some("IPv6アドレスが見つかりません".to_string()),
        }
    };

    Ok(HttpPingDualResult {
        url,
        dns_resolution: dns_result,
        ipv4: ipv4_result,
        ipv6: ipv6_result,
    })
}

// DNS名前解決を実行
async fn resolve_dns(host: &str) -> DnsResolution {
    use trust_dns_resolver::TokioAsyncResolver;

    let mut ipv4_addresses = Vec::new();
    let mut ipv6_addresses = Vec::new();

    // TokioAsyncResolverを作成（システム設定から）
    match TokioAsyncResolver::tokio_from_system_conf() {
        Ok(resolver) => {
            // IP解決を実行
            if let Ok(lookup) = resolver.lookup_ip(host).await {
                for ip_addr in lookup.iter() {
                    match ip_addr {
                        std::net::IpAddr::V4(v4) => {
                            ipv4_addresses.push(v4.to_string());
                        }
                        std::net::IpAddr::V6(v6) => {
                            ipv6_addresses.push(v6.to_string());
                        }
                    }
                }
            }
        }
        Err(_) => {
            // システム設定から作成できない場合は DefaultResolverConfig を使用
            use trust_dns_resolver::config::*;

            let resolver = TokioAsyncResolver::tokio(
                ResolverConfig::new(),
                ResolverOpts::default(),
            );

            if let Ok(lookup) = resolver.lookup_ip(host).await {
                for ip_addr in lookup.iter() {
                    match ip_addr {
                        std::net::IpAddr::V4(v4) => {
                            ipv4_addresses.push(v4.to_string());
                        }
                        std::net::IpAddr::V6(v6) => {
                            ipv6_addresses.push(v6.to_string());
                        }
                    }
                }
            }
        }
    }

    DnsResolution {
        ipv4_addresses,
        ipv6_addresses,
    }
}

// 指定されたIPアドレスにHTTP接続（Hostsヘッダー付き）
async fn connect_to_ip_with_host(
    original_url: String,
    ip_address: &str,
    host: &str,
    ignore_tls_errors: bool,
    _port: Option<u16>,
) -> HttpPingResult {
    let start = Instant::now();

    // URLをIPアドレスで置き換え（IPv6の場合は[]で囲む）
    let ip_for_url = if ip_address.contains(':') {
        format!("[{}]", ip_address)
    } else {
        ip_address.to_string()
    };

    let request_url = format!("{}://{}",
        if original_url.starts_with("https") { "https" } else { "http" },
        ip_for_url
    );

    // HTTPクライアントの構築
    let client_result = if ignore_tls_errors {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(30))
            .build()
    } else {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
    };

    let client = match client_result {
        Ok(c) => c,
        Err(e) => {
            return HttpPingResult {
                url: original_url,
                ip_address: Some(ip_address.to_string()),
                status_code: None,
                response_time_ms: None,
                success: false,
                error_message: Some(format!("HTTPクライアント作成失敗: {}", e)),
            };
        }
    };

    // HTTPリクエスト（Hostヘッダー付き）
    let response = match client
        .get(&request_url)
        .header("Host", host)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            return HttpPingResult {
                url: original_url,
                ip_address: Some(ip_address.to_string()),
                status_code: None,
                response_time_ms: Some(elapsed),
                success: false,
                error_message: Some(format!("接続エラー: {}", e)),
            };
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    let success = response.status().is_success();

    HttpPingResult {
        url: original_url,
        ip_address: Some(ip_address.to_string()),
        status_code: Some(status_code),
        response_time_ms: Some(elapsed),
        success,
        error_message: if success {
            None
        } else {
            Some(format!("HTTPステータス: {}", status_code))
        },
    }
}

// ネットワークインターフェース情報を取得
fn get_network_interfaces() -> Result<Vec<NetworkAdapter>, String> {
    use std::process::Command;

    let output = Command::new("powershell")
        .args(&[
            "-Command",
            "Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | Select-Object -ExpandProperty Name",
        ])
        .output()
        .map_err(|e| format!("PowerShellコマンド実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("ネットワークアダプタの取得に失敗しました".to_string());
    }

    let adapter_names = String::from_utf8_lossy(&output.stdout);
    let mut adapters = Vec::new();

    for name in adapter_names.lines() {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }

        // 各アダプタのIPアドレスを取得
        let ip_output = Command::new("powershell")
            .args(&[
                "-Command",
                &format!(
                    "Get-NetIPAddress -InterfaceAlias '{}' | Select-Object -ExpandProperty IPAddress",
                    name
                ),
            ])
            .output();

        if let Ok(ip_out) = ip_output {
            let ip_addresses: Vec<String> = String::from_utf8_lossy(&ip_out.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let mut has_ipv4 = false;
            let mut has_ipv6 = false;
            let mut has_ipv4_global = false;
            let mut has_ipv6_global = false;

            for ip_str in &ip_addresses {
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    match ip {
                        IpAddr::V4(v4) => {
                            has_ipv4 = true;
                            if is_global_ipv4(&v4) {
                                has_ipv4_global = true;
                            }
                        }
                        IpAddr::V6(v6) => {
                            has_ipv6 = true;
                            if is_global_ipv6(&v6) {
                                has_ipv6_global = true;
                            }
                        }
                    }
                }
            }

            adapters.push(NetworkAdapter {
                name: name.to_string(),
                ip_addresses,
                has_ipv4,
                has_ipv6,
                has_ipv4_global,
                has_ipv6_global,
            });
        }
    }

    Ok(adapters)
}

// IPv4がグローバルアドレスかどうかを判定
fn is_global_ipv4(ip: &Ipv4Addr) -> bool {
    !ip.is_private()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_broadcast()
        && !ip.is_multicast()
        && !ip.is_unspecified()
}

// IPv6がグローバルアドレスかどうかを判定
fn is_global_ipv6(ip: &Ipv6Addr) -> bool {
    !ip.is_loopback() && !ip.is_multicast() && !ip.is_unspecified()
}

// IPv4接続確認
async fn check_ipv4_connectivity() -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("クライアント作成失敗: {}", e))?;

    match client.get("https://getipv4.0nyx.net/").send().await {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false),
    }
}

// IPv6接続確認
async fn check_ipv6_connectivity() -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("クライアント作成失敗: {}", e))?;

    match client.get("https://getipv6.0nyx.net/").send().await {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false),
    }
}

// DNS解決確認
async fn check_dns_resolution() -> Result<bool, String> {
    use tokio::net::lookup_host;

    match lookup_host("example.com:80").await {
        Ok(mut addrs) => Ok(addrs.next().is_some()),
        Err(_) => Ok(false),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![environment_check, ping_http, ping_http_dual])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
