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
pub struct GlobalIPInfo {
    pub client_host: String,
    pub datetime_jst: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DnsServerInfo {
    pub interface_alias: String,
    pub ipv4_dns_servers: Vec<String>,
    pub ipv6_dns_servers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvironmentCheckResult {
    pub adapters: Vec<NetworkAdapter>,
    pub ipv4_connectivity: bool,
    pub ipv6_connectivity: bool,
    pub dns_resolution: bool,
    pub internet_available: bool,
    pub ipv4_global_ip: Option<GlobalIPInfo>,
    pub ipv6_global_ip: Option<GlobalIPInfo>,
    pub dns_servers: Vec<DnsServerInfo>,
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
        ipv4_global_ip: None,
        ipv6_global_ip: None,
        dns_servers: vec![],
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

    // DNSサーバ情報の取得
    match get_dns_servers() {
        Ok(dns_info) => {
            result.dns_servers = dns_info;
        }
        Err(e) => {
            result
                .error_messages
                .push(format!("DNSサーバ情報取得に失敗: {}", e));
        }
    }

    // インターネット接続判定
    result.internet_available = (result.ipv4_connectivity || result.ipv6_connectivity)
        && result.dns_resolution;

    // グローバルIPアドレス取得
    if result.internet_available {
        match fetch_global_ipv4_info().await {
            Ok(info) => {
                result.ipv4_global_ip = Some(info);
            }
            Err(e) => {
                result.error_messages.push(format!("IPv4グローバルIP取得に失敗: {}", e));
            }
        }

        match fetch_global_ipv6_info().await {
            Ok(info) => {
                result.ipv6_global_ip = Some(info);
            }
            Err(e) => {
                result.error_messages.push(format!("IPv6グローバルIP取得に失敗: {}", e));
            }
        }
    }

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

// DNS名前解決を実行（tokio を使用・非ブロッキング）
async fn resolve_dns(host: &str) -> DnsResolution {
    use tokio::net::lookup_host;
    use std::net::IpAddr;

    let mut ipv4_addresses = Vec::new();
    let mut ipv6_addresses = Vec::new();

    // ホスト名をIPアドレスに解決
    let socket_addr = format!("{}:80", host);

    match lookup_host(&socket_addr).await {
        Ok(addrs) => {
            for addr in addrs {
                match addr.ip() {
                    IpAddr::V4(ipv4) => {
                        ipv4_addresses.push(ipv4.to_string());
                    }
                    IpAddr::V6(ipv6) => {
                        ipv6_addresses.push(ipv6.to_string());
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("DNS resolution failed for {}: {:?}", host, e);
        }
    }

    DnsResolution {
        ipv4_addresses,
        ipv6_addresses,
    }
}

// 指定されたIPアドレスにHTTP接続（curl コマンドを使用・SNI対応）
async fn connect_to_ip_with_host(
    original_url: String,
    ip_address: &str,
    host: &str,
    ignore_tls_errors: bool,
    port: Option<u16>,
) -> HttpPingResult {
    use std::process::Command;

    let start = Instant::now();

    let is_https = original_url.starts_with("https");
    let _scheme = if is_https { "https" } else { "http" };
    let default_port = if is_https { 443 } else { 80 };
    let port_num = port.unwrap_or(default_port);

    // --resolve オプションにはIPv6は角括弧で囲む
    let resolve_arg = if ip_address.contains(':') {
        format!("{}:{}:[{}]", host, port_num, ip_address)
    } else {
        format!("{}:{}:{}", host, port_num, ip_address)
    };

    // 元のURLでのリクエスト（SNI用）
    let request_url = original_url.clone();

    let mut cmd_args = vec![
        "--resolve".to_string(),
        resolve_arg,
        "-s".to_string(),
        "-o".to_string(),
        "nul".to_string(),
        "-w".to_string(),
        "%{http_code}".to_string(),
        "-m".to_string(),
        "30".to_string(),
    ];

    if ignore_tls_errors {
        cmd_args.push("-k".to_string());
    }

    cmd_args.push(request_url.clone());

    let output = Command::new("curl.exe")
        .args(&cmd_args)
        .output();

    let elapsed = start.elapsed().as_millis() as u64;

    match output {
        Ok(output) => {
            let status_code_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();

            if output.status.success() && !status_code_str.is_empty() {
                if let Ok(status_code) = status_code_str.parse::<u16>() {
                    let success = status_code >= 200 && status_code < 300;
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
                } else {
                    HttpPingResult {
                        url: original_url,
                        ip_address: Some(ip_address.to_string()),
                        status_code: None,
                        response_time_ms: Some(elapsed),
                        success: false,
                        error_message: Some(format!("ステータスコード解析失敗: {}", status_code_str)),
                    }
                }
            } else {
                // エラーメッセージをstderr と status から取得
                let error_msg = if !stderr_str.is_empty() {
                    stderr_str
                } else {
                    format!("curl 終了コード: {}", output.status.code().unwrap_or(-1))
                };

                HttpPingResult {
                    url: original_url,
                    ip_address: Some(ip_address.to_string()),
                    status_code: None,
                    response_time_ms: Some(elapsed),
                    success: false,
                    error_message: Some(format!("接続エラー: {}", error_msg)),
                }
            }
        }
        Err(e) => {
            HttpPingResult {
                url: original_url,
                ip_address: Some(ip_address.to_string()),
                status_code: None,
                response_time_ms: Some(elapsed),
                success: false,
                error_message: Some(format!("curl 実行失敗: {}", e)),
            }
        }
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

// DNSサーバ情報を取得
fn get_dns_servers() -> Result<Vec<DnsServerInfo>, String> {
    // 最初に ipconfig /all を試す（最も確実）
    match parse_dns_from_ipconfig() {
        Ok(result) if !result.is_empty() => {
            return Ok(result);
        }
        _ => {
            // ipconfig で失敗した場合、PowerShell を試す
            return get_dns_servers_from_powershell();
        }
    }
}

// PowerShell を使用して DNS サーバ情報を取得
fn get_dns_servers_from_powershell() -> Result<Vec<DnsServerInfo>, String> {
    use std::process::Command;

    // Get-NetAdapter で取得してから、各アダプタの DNS を取得
    let ps_command = r#"Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | ForEach-Object {
        $iface = $_.Name
        Get-DnsClientServerAddress -InterfaceAlias $iface -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ServerAddresses | ForEach-Object { "$iface : $_" }
    }"#;

    let output = Command::new("powershell")
        .args(&["-Command", ps_command])
        .output()
        .map_err(|e| format!("PowerShellコマンド実行失敗: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("DNS サーバ取得 PowerShell エラー: {}", stderr);
        return Err("DNSサーバ情報の取得に失敗しました".to_string());
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut result = Vec::new();
    let mut current_adapter_map: std::collections::HashMap<String, (Vec<String>, Vec<String>)> =
        std::collections::HashMap::new();

    // 出力をパース: "InterfaceName : IP_Address" 形式
    for line in output_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(sep_pos) = line.find(" : ") {
            let adapter_name = line[..sep_pos].trim().to_string();
            let ip_addr = line[sep_pos + 3..].trim().to_string();

            if is_ip_address_like(&ip_addr) {
                let entry = current_adapter_map
                    .entry(adapter_name)
                    .or_insert_with(|| (Vec::new(), Vec::new()));

                let colon_count = ip_addr.matches(':').count();
                if colon_count > 1 {
                    entry.1.push(ip_addr);
                } else if ip_addr.contains('.') {
                    entry.0.push(ip_addr);
                }
            }
        }
    }

    // HashMap を DnsServerInfo に変換
    for (adapter_name, (ipv4_addrs, ipv6_addrs)) in current_adapter_map {
        if !ipv4_addrs.is_empty() || !ipv6_addrs.is_empty() {
            result.push(DnsServerInfo {
                interface_alias: adapter_name,
                ipv4_dns_servers: ipv4_addrs,
                ipv6_dns_servers: ipv6_addrs,
            });
        }
    }

    if result.is_empty() {
        return Err("PowerShell から DNS 情報を取得できませんでした".to_string());
    }

    Ok(result)
}

// フォールバック: ipconfig /all から DNS サーバ情報を取得
fn parse_dns_from_ipconfig() -> Result<Vec<DnsServerInfo>, String> {
    use std::process::Command;

    let output = Command::new("ipconfig")
        .args(&["/all"])
        .output()
        .map_err(|e| format!("ipconfig コマンド実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("DNS サーバ情報の取得に失敗しました".to_string());
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut result = Vec::new();
    let mut current_adapter: Option<String> = None;
    let mut current_ipv4_dns: Vec<String> = Vec::new();
    let mut current_ipv6_dns: Vec<String> = Vec::new();

    for line in output_str.lines() {
        let line_lower = line.to_lowercase();
        let trimmed = line.trim();

        // アダプタ行の検出
        // 行頭にスペースがなく、「アダプター」または「adapter」を含み、「:」を含む行
        if !line.starts_with(' ')
            && !line.is_empty()
            && (line_lower.contains("アダプター") || line_lower.contains("adapter"))
            && line.contains(':')
        {
            // 前のアダプタ情報を保存
            if let Some(adapter_name) = current_adapter.take() {
                if !current_ipv4_dns.is_empty() || !current_ipv6_dns.is_empty() {
                    result.push(DnsServerInfo {
                        interface_alias: adapter_name,
                        ipv4_dns_servers: current_ipv4_dns.clone(),
                        ipv6_dns_servers: current_ipv6_dns.clone(),
                    });
                }
            }

            // 新しいアダプタ情報を抽出
            if let Some(pos) = line.find(':') {
                let adapter_name = line[..pos].trim().to_string();
                // "イーサネット アダプター xxx:" 形式から "xxx" を抽出
                // または "Ethernet adapter xxx:" 形式から "xxx" を抽出
                let extracted_name = if let Some(name_start) = adapter_name.to_lowercase().find("アダプター ") {
                    adapter_name[name_start + 5..].to_string() // "アダプター " は5文字
                } else if let Some(name_start) = adapter_name.to_lowercase().find("adapter ") {
                    adapter_name[name_start + 8..].to_string()
                } else {
                    adapter_name
                };

                current_adapter = Some(extracted_name);
                current_ipv4_dns.clear();
                current_ipv6_dns.clear();
            }
        } else if current_adapter.is_some() && (line_lower.contains("dns サーバー") || line_lower.contains("dns servers")) && line.contains(':') {
            // DNS サーバー行
            if let Some(pos) = line.find(':') {
                let dns_part = line[pos + 1..].trim();
                if !dns_part.is_empty() && is_ip_address_like(dns_part) {
                    let colon_count = dns_part.matches(':').count();
                    if colon_count > 1 {
                        // IPv6
                        current_ipv6_dns.push(dns_part.to_string());
                    } else if dns_part.contains('.') {
                        // IPv4
                        current_ipv4_dns.push(dns_part.to_string());
                    }
                }
            }
        } else if current_adapter.is_some() && line.starts_with(' ') && !trimmed.is_empty() {
            // DNS サーバーの継続行（インデント付き）
            // ただし「. . . .」が含まれていない行のみ（属性行ではない）
            if !line.contains(" . ") && is_ip_address_like(trimmed) {
                let colon_count = trimmed.matches(':').count();
                if colon_count > 1 {
                    // IPv6
                    if !current_ipv6_dns.contains(&trimmed.to_string()) {
                        current_ipv6_dns.push(trimmed.to_string());
                    }
                } else if trimmed.contains('.') {
                    // IPv4
                    if !current_ipv4_dns.contains(&trimmed.to_string()) {
                        current_ipv4_dns.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    // 最後のアダプタの情報を保存
    if let Some(adapter_name) = current_adapter {
        if !current_ipv4_dns.is_empty() || !current_ipv6_dns.is_empty() {
            result.push(DnsServerInfo {
                interface_alias: adapter_name,
                ipv4_dns_servers: current_ipv4_dns,
                ipv6_dns_servers: current_ipv6_dns,
            });
        }
    }

    Ok(result)
}

// IP アドレスのようなパターンかどうかを判定
fn is_ip_address_like(s: &str) -> bool {
    // IPv4: 3つ以上のドット + 数字
    // IPv6: 2つ以上のコロン + 16進数
    let dot_count = s.matches('.').count();
    let colon_count = s.matches(':').count();
    let has_hex = s.chars().any(|c| c.is_ascii_hexdigit());
    let has_digit = s.chars().any(|c| c.is_ascii_digit());

    (dot_count >= 3 && has_digit) || (colon_count >= 2 && has_hex)
}

// グローバルIPv4情報の取得
async fn fetch_global_ipv4_info() -> Result<GlobalIPInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("クライアント作成失敗: {}", e))?;

    #[derive(Deserialize)]
    struct IpResponse {
        client_host: String,
        datetime_jst: String,
    }

    let response = client
        .get("https://getipv4.0nyx.net/json")
        .send()
        .await
        .map_err(|e| format!("IPv4リクエスト失敗: {}", e))?;

    let body: IpResponse = response
        .json()
        .await
        .map_err(|e| format!("JSON解析失敗: {}", e))?;

    Ok(GlobalIPInfo {
        client_host: body.client_host,
        datetime_jst: body.datetime_jst,
    })
}

// グローバルIPv6情報の取得
async fn fetch_global_ipv6_info() -> Result<GlobalIPInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("クライアント作成失敗: {}", e))?;

    #[derive(Deserialize)]
    struct IpResponse {
        client_host: String,
        datetime_jst: String,
    }

    let response = client
        .get("https://getipv6.0nyx.net/json")
        .send()
        .await
        .map_err(|e| format!("IPv6リクエスト失敗: {}", e))?;

    let body: IpResponse = response
        .json()
        .await
        .map_err(|e| format!("JSON解析失敗: {}", e))?;

    Ok(GlobalIPInfo {
        client_host: body.client_host,
        datetime_jst: body.datetime_jst,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![environment_check, ping_http, ping_http_dual])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
