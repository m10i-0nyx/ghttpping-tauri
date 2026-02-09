import { invoke } from "@tauri-apps/api/core";

interface NetworkAdapter {
    name: string;
    ip_addresses: string[];
    has_ipv4: boolean;
    has_ipv6: boolean;
    has_ipv4_global: boolean;
    has_ipv6_global: boolean;
}

interface EnvironmentCheckResult {
    adapters: NetworkAdapter[];
    ipv4_connectivity: boolean;
    ipv6_connectivity: boolean;
    dns_resolution: boolean;
    internet_available: boolean;
    error_messages: string[];
}

interface HttpPingResult {
    url: string;
    status_code?: number;
    response_time_ms?: number;
    tls_certificate_expiry?: string;
    success: boolean;
    error_message?: string;
}

let lastEnvResult: EnvironmentCheckResult | null = null;
let lastPingResult: HttpPingResult | null = null;

// DOMが読み込まれたら初期化
window.addEventListener("DOMContentLoaded", () => {
    const checkEnvBtn = document.getElementById("check-env-btn");
    const pingBtn = document.getElementById("ping-btn");
    const mailtoBtn = document.getElementById("mailto-btn");
    const urlInput = document.getElementById("url-input") as HTMLInputElement;

    if (checkEnvBtn) {
        checkEnvBtn.addEventListener("click", checkEnvironment);
    }

    if (pingBtn) {
        pingBtn.addEventListener("click", performHttpPing);
    }

    if (mailtoBtn) {
        mailtoBtn.addEventListener("click", sendMailto);
    }

    // Enterキーでも実行可能に
    if (urlInput) {
        urlInput.addEventListener("keypress", (e) => {
            if (e.key === "Enter") {
                performHttpPing();
            }
        });
    }
});

// 環境チェックを実行
async function checkEnvironment() {
    const resultDiv = document.getElementById("env-result");
    if (!resultDiv) return;

    resultDiv.innerHTML = '<div class="loading">環境をチェック中...</div>';

    try {
        const result = (await invoke("environment_check")) as EnvironmentCheckResult;
        lastEnvResult = result;

        let html = "";

        // インターネット接続状況
        if (result.internet_available) {
            html += '<div class="success">✅ インターネット接続可能</div>';
        } else {
            html += '<div class="error">❌ インターネット接続不可</div>';
        }

        // 詳細情報
        html += "<h3>詳細情報</h3>";
        html += "<ul>";
        html += `<li>IPv4接続: ${result.ipv4_connectivity ? "✅" : "❌"}</li>`;
        html += `<li>IPv6接続: ${result.ipv6_connectivity ? "✅" : "❌"}</li>`;
        html += `<li>DNS解決: ${result.dns_resolution ? "✅" : "❌"}</li>`;
        html += "</ul>";

        // ネットワークアダプタ情報
        if (result.adapters.length > 0) {
            html += "<h3>ネットワークアダプタ</h3>";
            html += '<div class="adapter-list">';
            result.adapters.forEach((adapter) => {
                html += `<div class="adapter-item">`;
                html += `<strong>${adapter.name}</strong><br>`;
                html += `IPv4: ${adapter.has_ipv4 ? "あり" : "なし"}`;
                if (adapter.has_ipv4_global) {
                    html += " (グローバル)";
                }
                html += `<br>IPv6: ${adapter.has_ipv6 ? "あり" : "なし"}`;
                if (adapter.has_ipv6_global) {
                    html += " (グローバル)";
                }
                if (adapter.ip_addresses.length > 0) {
                    html += `<br>IPアドレス: ${adapter.ip_addresses.join(", ")}`;
                }
                html += `</div>`;
            });
            html += "</div>";
        }

        // エラーメッセージ
        if (result.error_messages.length > 0) {
            html += "<h3>エラー・警告</h3>";
            html += '<div class="error">';
            result.error_messages.forEach((msg) => {
                html += `<p>${msg}</p>`;
            });
            html += "</div>";
        }

        resultDiv.innerHTML = html;
    } catch (error) {
        resultDiv.innerHTML = `<div class="error">エラーが発生しました: ${error}</div>`;
    }
}

// HTTP/HTTPS疎通確認を実行
async function performHttpPing() {
    const urlInput = document.getElementById("url-input") as HTMLInputElement;
    const ignoreTlsCheckbox = document.getElementById(
        "ignore-tls-checkbox"
    ) as HTMLInputElement;
    const resultDiv = document.getElementById("ping-result");
    const mailtoBtn = document.getElementById("mailto-btn");

    if (!urlInput || !resultDiv) return;

    const url = urlInput.value.trim();
    if (!url) {
        resultDiv.innerHTML = '<div class="error">URLを入力してください</div>';
        return;
    }

    resultDiv.innerHTML = '<div class="loading">疎通確認中...</div>';

    try {
        const ignoreTls = ignoreTlsCheckbox?.checked || false;
        const result = (await invoke("ping_http", {
            url,
            ignoreTlsErrors: ignoreTls,
        })) as HttpPingResult;

        lastPingResult = result;

        let html = "";

        if (result.success) {
            html += '<div class="success">✅ 疎通確認成功</div>';
        } else {
            html += '<div class="error">❌ 疎通確認失敗</div>';
        }

        html += "<h3>結果詳細</h3>";
        html += "<ul>";
        html += `<li><strong>URL:</strong> ${result.url}</li>`;

        if (result.status_code !== undefined) {
            html += `<li><strong>ステータスコード:</strong> ${result.status_code}</li>`;
        }

        if (result.response_time_ms !== undefined) {
            html += `<li><strong>レスポンス時間:</strong> ${result.response_time_ms} ms</li>`;
        }

        if (result.tls_certificate_expiry) {
            html += `<li><strong>TLS証明書:</strong> ${result.tls_certificate_expiry}</li>`;
        }

        if (result.error_message) {
            html += `<li><strong>エラー:</strong> ${result.error_message}</li>`;
        }

        html += "</ul>";

        resultDiv.innerHTML = html;

        // メール送信ボタンを有効化
        if (mailtoBtn) {
            mailtoBtn.removeAttribute("disabled");
        }
    } catch (error) {
        resultDiv.innerHTML = `<div class="error">エラーが発生しました: ${error}</div>`;
    }
}

// 結果をメールで送信
function sendMailto() {
    let body = "=== ghttpping 疎通確認結果 ===\n\n";

    if (lastEnvResult) {
        body += "■ 環境チェック結果\n";
        body += `インターネット接続: ${lastEnvResult.internet_available ? "可能" : "不可"}\n`;
        body += `IPv4接続: ${lastEnvResult.ipv4_connectivity ? "あり" : "なし"}\n`;
        body += `IPv6接続: ${lastEnvResult.ipv6_connectivity ? "あり" : "なし"}\n`;
        body += `DNS解決: ${lastEnvResult.dns_resolution ? "可能" : "不可"}\n\n`;

        if (lastEnvResult.adapters.length > 0) {
            body += "ネットワークアダプタ:\n";
            lastEnvResult.adapters.forEach((adapter) => {
                body += `  - ${adapter.name}\n`;
                body += `    IPv4: ${adapter.has_ipv4 ? "あり" : "なし"}`;
                if (adapter.has_ipv4_global) body += " (グローバル)";
                body += `\n    IPv6: ${adapter.has_ipv6 ? "あり" : "なし"}`;
                if (adapter.has_ipv6_global) body += " (グローバル)";
                body += "\n";
            });
            body += "\n";
        }
    }

    if (lastPingResult) {
        body += "■ 疎通確認結果\n";
        body += `URL: ${lastPingResult.url}\n`;
        body += `結果: ${lastPingResult.success ? "成功" : "失敗"}\n`;
        if (lastPingResult.status_code !== undefined) {
            body += `ステータスコード: ${lastPingResult.status_code}\n`;
        }
        if (lastPingResult.response_time_ms !== undefined) {
            body += `レスポンス時間: ${lastPingResult.response_time_ms} ms\n`;
        }
        if (lastPingResult.tls_certificate_expiry) {
            body += `TLS証明書: ${lastPingResult.tls_certificate_expiry}\n`;
        }
        if (lastPingResult.error_message) {
            body += `エラー: ${lastPingResult.error_message}\n`;
        }
    }

    const subject = "ghttpping 疎通確認結果";
    const mailtoLink = `mailto:?subject=${encodeURIComponent(
        subject
    )}&body=${encodeURIComponent(body)}`;

    window.location.href = mailtoLink;
}
