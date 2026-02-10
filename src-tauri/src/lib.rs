use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Instant;
use std::process::{Command, Stdio};
use std::collections::HashMap;
use url::Url;
use encoding_rs::SHIFT_JIS;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

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
pub struct DnsResolution {
    pub ipv4_addresses: Vec<String>,
    pub ipv6_addresses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpPingResult {
    pub url: String,
    pub ip_address: Option<String>,
    pub status_code: Option<u16>,
    pub response_time_ms: Option<u64>,
    pub success: bool,
    pub error_message: Option<String>,
    pub verbose_log: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpPingDualResult {
    pub url: String,
    pub dns_resolution: DnsResolution,
    pub ipv4: HttpPingResult,
    pub ipv6: HttpPingResult,
}

// IP取得用の内部構造体
#[derive(Deserialize)]
struct IpResponse {
    client_host: String,
    datetime_jst: String,
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

    // IPv4接続確認（グローバルIP取得で兼ねる）
    match fetch_global_ip_info("https://getipv4.0nyx.net/json", 2).await {
        Ok(info) => {
            result.ipv4_connectivity = true;
            result.ipv4_global_ip = Some(info);
        }
        Err(e) => {
            result.ipv4_connectivity = false;
            result.error_messages.push(format!("IPv4グローバルIP取得に失敗: {}", e));
        }
    }

    // IPv6接続確認（グローバルIP取得で兼ねる）
    match fetch_global_ip_info("https://getipv6.0nyx.net/json", 2).await {
        Ok(info) => {
            result.ipv6_connectivity = true;
            result.ipv6_global_ip = Some(info);
        }
        Err(e) => {
            result.ipv6_connectivity = false;
            // IPv4が成功している場合は、IPv6エラーを表示しない
            if !result.ipv4_connectivity {
                result.error_messages.push(format!("IPv6グローバルIP取得に失敗: {}", e));
            }
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

    // DNSサーバ情報の取得（タイムアウト付き）
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        get_dns_servers_async(),
    )
    .await
    {
        Ok(Ok(dns_info)) => {
            result.dns_servers = dns_info;
        }
        Ok(Err(e)) => {
            result
                .error_messages
                .push(format!("DNSサーバ情報取得に失敗: {}", e));
        }
        Err(_) => {
            result
                .error_messages
                .push("DNSサーバ情報取得がタイムアウトしました".to_string());
        }
    }

    // インターネット接続判定
    result.internet_available = (result.ipv4_connectivity || result.ipv6_connectivity)
        && result.dns_resolution;

    Ok(result)
}

#[tauri::command]
async fn ping_http_dual(
    url: String,
    ignore_tls_errors: bool,
    save_verbose_log: bool,
) -> Result<HttpPingDualResult, String> {
    if ignore_tls_errors {
        log_security_warning("TLS証明書検証が無効化されています");
    }

    validate_url(&url)?;

    let parsed_url = match Url::parse(&url) {
        Ok(u) => u,
        Err(e) => return Err(format!("無効なURL: {}", e)),
    };

    let host = match parsed_url.host_str() {
        Some(h) => h,
        None => return Err("URLからホスト名を抽出できません".to_string()),
    };

    // ホスト名の検証（セキュリティ）
    validate_hostname(host)?;

    // DNS名前解決
    let dns_result = resolve_dns(host).await;
    let ipv4_addresses = dns_result.ipv4_addresses.clone();
    let ipv6_addresses = dns_result.ipv6_addresses.clone();

    // IPv4/IPv6への並列接続試行
    let (ipv4_result, ipv6_result) = tokio::join!(
        connect_to_ip_with_host(
            url.clone(),
            &ipv4_addresses,
            host,
            ignore_tls_errors,
            parsed_url.port(),
            save_verbose_log,
        ),
        connect_to_ip_with_host(
            url.clone(),
            &ipv6_addresses,
            host,
            ignore_tls_errors,
            parsed_url.port(),
            save_verbose_log,
        ),
    );

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

    let socket_addr = format!("{}:80", host);

    match lookup_host(&socket_addr).await {
        Ok(addrs) => {
            for addr in addrs {
                match addr.ip() {
                    IpAddr::V4(ipv4) => {
                        let ip_str = ipv4.to_string();
                        if !ipv4_addresses.contains(&ip_str) {
                            ipv4_addresses.push(ip_str);
                        }
                    }
                    IpAddr::V6(ipv6) => {
                        let ip_str = ipv6.to_string();
                        if !ipv6_addresses.contains(&ip_str) {
                            ipv6_addresses.push(ip_str);
                        }
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
    ip_addresses: &[String],
    host: &str,
    ignore_tls_errors: bool,
    port: Option<u16>,
    save_verbose_log: bool,
) -> HttpPingResult {
    // IPアドレスが存在しない場合
    if ip_addresses.is_empty() {
        let is_https = original_url.starts_with("https");
        return HttpPingResult {
            url: original_url,
            ip_address: None,
            status_code: None,
            response_time_ms: None,
            success: false,
            error_message: Some(
                if is_https {
                    "IPv6アドレスが見つかりません".to_string()
                } else {
                    "IPv4アドレスが見つかりません".to_string()
                }
            ),
            verbose_log: None,
        };
    }

    // 最初のIPアドレスを使用して接続を試行
    let ip_address = &ip_addresses[0];
    perform_curl_request(&original_url, ip_address, host, ignore_tls_errors, port, save_verbose_log).await
}

// curlを使用したHTTPリクエスト実行
async fn perform_curl_request(
    original_url: &str,
    ip_address: &str,
    host: &str,
    ignore_tls_errors: bool,
    port: Option<u16>,
    save_verbose_log: bool,
) -> HttpPingResult {
    let start = Instant::now();

    let is_https = original_url.starts_with("https");
    let default_port = if is_https { 443 } else { 80 };
    let port_num = port.unwrap_or(default_port);

    // --resolveオプションの構築（IPv6は角括弧で囲む）
    let resolve_arg = if ip_address.contains(':') {
        format!("{}:{}:[{}]", host, port_num, ip_address)
    } else {
        format!("{}:{}:{}", host, port_num, ip_address)
    };

    let mut cmd_args = vec![
        "--resolve".to_string(),
        resolve_arg,
    ];

    // verbose ログを保存する場合は -v オプションを追加、否則 -s オプションを追加
    if save_verbose_log {
        cmd_args.push("-v".to_string());
    } else {
        cmd_args.push("-s".to_string());
    }

    cmd_args.extend(vec![
        "-o".to_string(),
        "nul".to_string(),
        "-w".to_string(),
        "%{http_code}".to_string(),
        "-m".to_string(),
        "10".to_string(),
    ]);

    if ignore_tls_errors {
        cmd_args.push("-k".to_string());
    }

    cmd_args.push(original_url.to_string());

    let output = Command::new("curl.exe")
        .args(&cmd_args)
        .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output();

    let elapsed = start.elapsed().as_millis() as u64;

    match output {
        Ok(output) => {
            let status_code_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let verbose_log_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let verbose_log = if !verbose_log_str.is_empty() {
                Some(verbose_log_str.clone())
            } else {
                None
            };

            if output.status.success() && !status_code_str.is_empty() {
                if let Ok(status_code) = status_code_str.parse::<u16>() {
                    let success = status_code >= 200 && status_code < 300;
                    HttpPingResult {
                        url: original_url.to_string(),
                        ip_address: Some(ip_address.to_string()),
                        status_code: Some(status_code),
                        response_time_ms: Some(elapsed),
                        success,
                        error_message: if success {
                            None
                        } else {
                            Some(format!("HTTPステータス: {}", status_code))
                        },
                        verbose_log,
                    }
                } else {
                    HttpPingResult {
                        url: original_url.to_string(),
                        ip_address: Some(ip_address.to_string()),
                        status_code: None,
                        response_time_ms: Some(elapsed),
                        success: false,
                        error_message: Some(format!("ステータスコード解析失敗: {}", status_code_str)),
                        verbose_log,
                    }
                }
            } else {
                let error_msg = if !verbose_log_str.is_empty() {
                    verbose_log_str.clone()
                } else {
                    format!("curl 終了コード: {}", output.status.code().unwrap_or(-1))
                };

                HttpPingResult {
                    url: original_url.to_string(),
                    ip_address: Some(ip_address.to_string()),
                    status_code: None,
                    response_time_ms: Some(elapsed),
                    success: false,
                    error_message: Some(format!("接続エラー: {}", error_msg)),
                    verbose_log,
                }
            }
        }
        Err(e) => HttpPingResult {
            url: original_url.to_string(),
            ip_address: Some(ip_address.to_string()),
            status_code: None,
            response_time_ms: Some(elapsed),
            success: false,
            error_message: Some(format!("curl 実行失敗: {}", e)),
            verbose_log: None,
        },
    }
}

// ネットワークインターフェース情報を取得（セキュリティ強化版）
fn get_network_interfaces() -> Result<Vec<NetworkAdapter>, String> {
    let output = Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            "Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | Select-Object -ExpandProperty Name",
        ])
        .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("PowerShellコマンド実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("ネットワークアダプタの取得に失敗しました".to_string());
    }

    let adapter_names = decode_command_output(&output.stdout);
    let mut adapters = Vec::new();

    for name in adapter_names.lines() {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }

        // アダプタ名のサニタイズ（基本的なチェック）
        if !is_valid_adapter_name(name) {
            eprintln!("Invalid adapter name: {}", name);
            continue;
        }

        // 各アダプタのIPアドレスを取得
        let get_ip_cmd = format!(
            "Get-NetIPAddress -InterfaceAlias '{}' | Where-Object {{$_.PrefixOrigin -ne 'WellKnown'}} | Select-Object -ExpandProperty IPAddress",
            name
        );

        let ip_output = Command::new("powershell")
            .args(&["-NoProfile", "-WindowStyle", "Hidden", "-Command", &get_ip_cmd])
            .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output();

        if let Ok(ip_out) = ip_output {
            let ip_addresses: Vec<String> = decode_command_output(&ip_out.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && is_valid_ip_address(s))
                .collect();

            let (has_ipv4, has_ipv6, has_ipv4_global, has_ipv6_global) =
                analyze_ip_addresses(&ip_addresses);

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

// IPv4/IPv6接続確認（汎用関数）
#[allow(dead_code)]
async fn check_connectivity(url: &str, timeout_secs: u64) -> Result<bool, String> {
    let output = Command::new("curl.exe")
        .args(&[
            "-s",
            "-o",
            "nul",
            "-w",
            "%{http_code}",
            "-m",
            &timeout_secs.to_string(),
            url,
        ])
        .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("curl実行失敗: {}", e))?;

    if !output.status.success() {
        return Ok(false);
    }

    let status_code_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match status_code_str.parse::<u16>() {
        Ok(status_code) => Ok(status_code >= 200 && status_code < 300),
        Err(_) => Ok(false),
    }
}

// グローバルIP情報取得（汎用関数）
async fn fetch_global_ip_info(url: &str, timeout_secs: u64) -> Result<GlobalIPInfo, String> {
    // 1回目: 通常のTLS検証で接続を試みる
    let output = Command::new("curl.exe")
        .args(&["-s", "-m", &timeout_secs.to_string(), url])
        .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("curl実行失敗: {}", e))?;

    // 失敗時はTLS証明書検証を無視してフォールバック
    let json_str = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        // 2回目: TLS証明書検証を無視して接続を試みる
        let fallback_output = Command::new("curl.exe")
            .args(&["-s", "-k", "-m", &timeout_secs.to_string(), url])
            .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .map_err(|e| format!("curl実行失敗(フォールバック): {}", e))?;

        if !fallback_output.status.success() {
            return Err("グローバルIP取得失敗（TLS検証有無両方失敗）".to_string());
        }

        String::from_utf8_lossy(&fallback_output.stdout).to_string()
    };

    let body: IpResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("JSON解析失敗: {}", e))?;

    Ok(GlobalIPInfo {
        client_host: body.client_host,
        datetime_jst: body.datetime_jst,
    })
}

// DNS解決確認
async fn check_dns_resolution() -> Result<bool, String> {
    use tokio::net::lookup_host;

    match lookup_host("example.com:80").await {
        Ok(mut addrs) => Ok(addrs.next().is_some()),
        Err(_) => Ok(false),
    }
}

// DNS サーバ情報の取得（非同期版）
async fn get_dns_servers_async() -> Result<Vec<DnsServerInfo>, String> {
    // ipconfig /all を優先的に使用（最も確実）
    match tokio::task::spawn_blocking(parse_dns_from_ipconfig_blocking).await {
        Ok(Ok(result)) if !result.is_empty() => return Ok(result),
        _ => {}
    }

    // PowerShell を別スレッドで実行
    match tokio::task::spawn_blocking(get_dns_servers_from_powershell_blocking).await {
        Ok(result) => result,
        Err(_) => Err("DNSサーバ取得スレッドエラー".to_string()),
    }
}

// DNS サーバ情報の取得（互換性のための同期版）
#[allow(dead_code)]
fn get_dns_servers() -> Result<Vec<DnsServerInfo>, String> {
    // ipconfig /all を優先的に使用（最も確実）
    match parse_dns_from_ipconfig() {
        Ok(result) if !result.is_empty() => Ok(result),
        _ => get_dns_servers_from_powershell(),
    }
}

// PowerShellのエンコーディングを指定してUTF-8として出力を取得する
fn decode_command_output(bytes: &[u8]) -> String {
    // Shift-JISとしてデコードを試みる
    let (cow, _, _) = SHIFT_JIS.decode(bytes);
    cow.to_string()
}

// parse_dns_from_ipconfig のブロッキング版
fn parse_dns_from_ipconfig_blocking() -> Result<Vec<DnsServerInfo>, String> {
    parse_dns_from_ipconfig()
}

// get_dns_servers_from_powershell のブロッキング版
fn get_dns_servers_from_powershell_blocking() -> Result<Vec<DnsServerInfo>, String> {
    get_dns_servers_from_powershell()
}

// PowerShell を使用して DNS サーバ情報を取得
fn get_dns_servers_from_powershell() -> Result<Vec<DnsServerInfo>, String> {
    let ps_command = r#"Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | ForEach-Object {
        $iface = $_.Name
        Get-DnsClientServerAddress -InterfaceAlias $iface -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty ServerAddresses |
        ForEach-Object { "$iface : $_" }
    }"#;

    let output = Command::new("powershell")
        .args(&["-NoProfile", "-WindowStyle", "Hidden", "-Command", ps_command])
        .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("PowerShellコマンド実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("DNSサーバ情報の取得に失敗しました".to_string());
    }

    let output_str = decode_command_output(&output.stdout);
    let mut result = Vec::new();
    let mut current_adapter_map: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();

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

// ipconfig /all から DNS サーバ情報を取得
fn parse_dns_from_ipconfig() -> Result<Vec<DnsServerInfo>, String> {
    let output = Command::new("ipconfig")
        .args(&["/all"])
        .creation_flags(0x08000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("ipconfig コマンド実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("DNS サーバ情報の取得に失敗しました".to_string());
    }

    let output_str = decode_command_output(&output.stdout);
    let mut result = Vec::new();
    let mut current_adapter: Option<String> = None;
    let mut current_ipv4_dns: Vec<String> = Vec::new();
    let mut current_ipv6_dns: Vec<String> = Vec::new();

    for line in output_str.lines() {
        let line_lower = line.to_lowercase();
        let trimmed = line.trim();

        // アダプタ行の検出
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
                let extracted_name = if let Some(name_start) = adapter_name.to_lowercase().find("アダプター ") {
                    adapter_name[name_start + 5..].to_string()
                } else if let Some(name_start) = adapter_name.to_lowercase().find("adapter ") {
                    adapter_name[name_start + 8..].to_string()
                } else {
                    adapter_name
                };

                current_adapter = Some(extracted_name);
                current_ipv4_dns.clear();
                current_ipv6_dns.clear();
            }
        } else if current_adapter.is_some()
            && (line_lower.contains("dns サーバー") || line_lower.contains("dns servers"))
            && line.contains(':')
        {
            // DNS サーバー行
            if let Some(pos) = line.find(':') {
                let dns_part = line[pos + 1..].trim();
                if !dns_part.is_empty() && is_ip_address_like(dns_part) {
                    let colon_count = dns_part.matches(':').count();
                    if colon_count > 1 {
                        if !current_ipv6_dns.contains(&dns_part.to_string()) {
                            current_ipv6_dns.push(dns_part.to_string());
                        }
                    } else if dns_part.contains('.') {
                        if !current_ipv4_dns.contains(&dns_part.to_string()) {
                            current_ipv4_dns.push(dns_part.to_string());
                        }
                    }
                }
            }
        } else if current_adapter.is_some()
            && line.starts_with(' ')
            && !trimmed.is_empty()
            && !line.contains(" . ")
            && is_ip_address_like(trimmed)
        {
            // DNS サーバーの継続行（インデント付き）
            let colon_count = trimmed.matches(':').count();
            if colon_count > 1 {
                if !current_ipv6_dns.contains(&trimmed.to_string()) {
                    current_ipv6_dns.push(trimmed.to_string());
                }
            } else if trimmed.contains('.') {
                if !current_ipv4_dns.contains(&trimmed.to_string()) {
                    current_ipv4_dns.push(trimmed.to_string());
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


// ============ セキュリティ・入力検証関数 ============

// URLの検証
fn validate_url(url: &str) -> Result<(), String> {
    if url.is_empty() || url.len() > 2048 {
        return Err("URLが空またはサイズが大きすぎます".to_string());
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URLは http:// または https:// で始まる必要があります".to_string());
    }

    Ok(())
}

// ホスト名の検証（コマンドインジェクション対策）
fn validate_hostname(host: &str) -> Result<(), String> {
    if host.is_empty() || host.len() > 255 {
        return Err("ホスト名が無効です".to_string());
    }

    // 危険な文字列を検出
    let dangerous_chars = ['$', '`', '|', '&', ';', '>', '<', '(', ')'];
    if dangerous_chars.iter().any(|&c| host.contains(c)) {
        return Err("ホスト名に無効な文字が含まれています".to_string());
    }

    Ok(())
}

// アダプタ名のサニタイズ
fn is_valid_adapter_name(name: &str) -> bool {
    // 基本的な長さチェック
    if name.is_empty() || name.len() > 255 {
        return false;
    }

    // 制御文字がないかチェック
    name.chars().all(|c| !c.is_control())
}

// IP アドレスのようなパターンかどうかを判定
fn is_valid_ip_address(s: &str) -> bool {
    // パースして有効なIPか確認
    match s.parse::<IpAddr>() {
        Ok(ip) => {
            // ローカルホストアドレスはフィルタリング
            match ip {
                IpAddr::V4(v4) => !v4.is_loopback(),
                IpAddr::V6(v6) => !v6.is_loopback(),
            }
        }
        Err(_) => false,
    }
}

// IP アドレスのようなパターンかどうかを判定（一般的な確認）
fn is_ip_address_like(s: &str) -> bool {
    let dot_count = s.matches('.').count();
    let colon_count = s.matches(':').count();
    let has_hex = s.chars().any(|c| c.is_ascii_hexdigit());
    let has_digit = s.chars().any(|c| c.is_ascii_digit());

    (dot_count >= 3 && has_digit) || (colon_count >= 2 && has_hex)
}

// IP アドレス分析
fn analyze_ip_addresses(ip_addresses: &[String]) -> (bool, bool, bool, bool) {
    let mut has_ipv4 = false;
    let mut has_ipv6 = false;
    let mut has_ipv4_global = false;
    let mut has_ipv6_global = false;

    for ip_str in ip_addresses {
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

    (has_ipv4, has_ipv6, has_ipv4_global, has_ipv6_global)
}

// セキュリティ警告ログ
fn log_security_warning(message: &str) {
    eprintln!("⚠️  セキュリティ警告: {}", message);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![environment_check, ping_http_dual])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
