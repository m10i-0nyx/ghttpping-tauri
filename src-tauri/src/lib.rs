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
    pub status_code: Option<u16>,
    pub response_time_ms: Option<u64>,
    pub tls_certificate_expiry: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
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
                status_code: None,
                response_time_ms: None,
                tls_certificate_expiry: None,
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
                status_code: None,
                response_time_ms: None,
                tls_certificate_expiry: None,
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
                status_code: None,
                response_time_ms: Some(elapsed),
                tls_certificate_expiry: None,
                success: false,
                error_message: Some(format!("接続エラー: {}", e)),
            });
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    let success = response.status().is_success();

    // TLS証明書の有効期限取得（HTTPSの場合）
    let tls_expiry = if parsed_url.scheme() == "https" {
        get_tls_certificate_expiry(&url, ignore_tls_errors).await
    } else {
        None
    };

    Ok(HttpPingResult {
        url: url.clone(),
        status_code: Some(status_code),
        response_time_ms: Some(elapsed),
        tls_certificate_expiry: tls_expiry,
        success,
        error_message: if success {
            None
        } else {
            Some(format!("HTTPステータス: {}", status_code))
        },
    })
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

// TLS証明書の有効期限を取得
async fn get_tls_certificate_expiry(url: &str, ignore_errors: bool) -> Option<String> {
    // 簡易実装: 実際の証明書取得は複雑なため、接続確認のみ
    // 実用的には native-tls や rustls の証明書情報取得機能を使用
    let _ = (url, ignore_errors);
    Some("証明書情報取得は未実装".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![environment_check, ping_http])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
