import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

interface NetworkAdapter {
    name: string;
    ip_addresses: string[];
    has_ipv4: boolean;
    has_ipv6: boolean;
    has_ipv4_global: boolean;
    has_ipv6_global: boolean;
}

interface GlobalIPInfo {
    client_host: string;
    datetime_jst: string;
}

interface DnsServerInfo {
    interface_alias: string;
    ipv4_dns_servers: string[];
    ipv6_dns_servers: string[];
}

interface EnvironmentCheckResult {
    adapters: NetworkAdapter[];
    ipv4_connectivity: boolean;
    ipv6_connectivity: boolean;
    dns_resolution: boolean;
    internet_available: boolean;
    ipv4_global_ip?: GlobalIPInfo;
    ipv6_global_ip?: GlobalIPInfo;
    dns_servers: DnsServerInfo[];
    error_messages: string[];
}

interface HttpPingResult {
    url: string;
    ip_address?: string;
    status_code?: number;
    response_time_ms?: number;
    success: boolean;
    error_message?: string;
}

interface DnsResolution {
    ipv4_addresses: string[];
    ipv6_addresses: string[];
}

interface HttpPingDualResult {
    url: string;
    dns_resolution: DnsResolution;
    ipv4: HttpPingResult;
    ipv6: HttpPingResult;
}

let lastEnvResult: EnvironmentCheckResult | null = null;
let lastPingDualResult: HttpPingDualResult | null = null;
let environmentCheckCompleted: boolean = false;

// DOMが読み込まれたら初期化
window.addEventListener("DOMContentLoaded", () => {
    const checkEnvBtn = document.getElementById("check-env-btn");
    const pingBtn = document.getElementById("ping-btn");
    const saveResultBtn = document.getElementById("save-result-btn");
    const urlInput = document.getElementById("url-input") as HTMLInputElement;

    if (checkEnvBtn) {
        checkEnvBtn.addEventListener("click", checkEnvironment);
    }

    if (pingBtn) {
        pingBtn.addEventListener("click", performHttpPing);
        // 初期状態で無効化
        updatePingButtonState();
    }

    if (saveResultBtn) {
        saveResultBtn.addEventListener("click", saveResultAsTextFile);
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

// ピングボタンの状態を更新
function updatePingButtonState() {
    const pingBtn = document.getElementById("ping-btn");
    if (!pingBtn) return;

    if (environmentCheckCompleted) {
        pingBtn.removeAttribute("disabled");
    } else {
        pingBtn.setAttribute("disabled", "true");
    }
}

// 環境チェックを実行
async function checkEnvironment() {
    const resultDiv = document.getElementById("env-result");
    if (!resultDiv) return;

    resultDiv.innerHTML = '<div class="loading">環境をチェック中...</div>';

    try {
        const result = (await invoke("environment_check")) as EnvironmentCheckResult;
        lastEnvResult = result;

        // 環境チェック完了状態を更新
        environmentCheckCompleted = true;
        updatePingButtonState();

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

        // グローバルIPアドレス情報
        if (result.ipv4_global_ip || result.ipv6_global_ip) {
            html += "<h3>グローバルIPアドレス</h3>";
            html += '<div class="global-ip-info">';

            if (result.ipv4_global_ip) {
                html += `<div class="ip-item">`;
                html += `<strong>IPv4:</strong> ${result.ipv4_global_ip.client_host}<br>`;
                html += `<small>${result.ipv4_global_ip.datetime_jst}</small>`;
                html += `</div>`;
            }

            if (result.ipv6_global_ip) {
                html += `<div class="ip-item">`;
                html += `<strong>IPv6:</strong> ${result.ipv6_global_ip.client_host}<br>`;
                html += `<small>${result.ipv6_global_ip.datetime_jst}</small>`;
                html += `</div>`;
            }

            html += "</div>";
        }

        // DNSサーバ情報
        if (result.dns_servers.length > 0) {
            html += "<h3>DNSサーバ設定</h3>";
            html += '<div class="dns-server-info">';

            result.dns_servers.forEach((dns) => {
                if (dns.ipv4_dns_servers.length > 0 || dns.ipv6_dns_servers.length > 0) {
                    html += `<div class="dns-adapter-item">`;
                    html += `<strong>${dns.interface_alias}</strong><br>`;

                    if (dns.ipv4_dns_servers.length > 0) {
                        html += `<div class="dns-ipv4">`;
                        html += `<u>IPv4 DNSサーバ:</u><br>`;
                        dns.ipv4_dns_servers.forEach((server, idx) => {
                            const label = idx === 0 ? "Primary" : idx === 1 ? "Secondary" : `(${idx + 1})`;
                            html += `&nbsp;&nbsp;${label}: ${server}<br>`;
                        });
                        html += `</div>`;
                    }

                    if (dns.ipv6_dns_servers.length > 0) {
                        html += `<div class="dns-ipv6">`;
                        html += `<u>IPv6 DNSサーバ:</u><br>`;
                        dns.ipv6_dns_servers.forEach((server, idx) => {
                            const label = idx === 0 ? "Primary" : idx === 1 ? "Secondary" : `(${idx + 1})`;
                            html += `&nbsp;&nbsp;${label}: ${server}<br>`;
                        });
                        html += `</div>`;
                    }

                    html += `</div>`;
                }
            });

            html += "</div>";
        }

        // ネットワークアダプタ情報（UIから非表示）
        // if (result.adapters.length > 0) {
        //     html += "<h3>ネットワークアダプタ</h3>";
        //     html += '<div class="adapter-list">';
        //     result.adapters.forEach((adapter) => {
        //         html += `<div class="adapter-item">`;
        //         html += `<strong>${adapter.name}</strong><br>`;
        //         html += `IPv4: ${adapter.has_ipv4 ? "あり" : "なし"}`;
        //         if (adapter.has_ipv4_global) {
        //             html += " (グローバル)";
        //         }
        //         html += `<br>IPv6: ${adapter.has_ipv6 ? "あり" : "なし"}`;
        //         if (adapter.has_ipv6_global) {
        //             html += " (グローバル)";
        //         }
        //         if (adapter.ip_addresses.length > 0) {
        //             html += `<br>IPアドレス: ${adapter.ip_addresses.join(", ")}`;
        //         }
        //         html += `</div>`;
        //     });
        //     html += "</div>";
        // }

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
        // エラーの場合は完了状態をリセット
        environmentCheckCompleted = false;
        updatePingButtonState();
    }
}

// HTTP/HTTPS疎通確認を実行
async function performHttpPing() {
    const urlInput = document.getElementById("url-input") as HTMLInputElement;
    const resultDiv = document.getElementById("ping-result");
    const saveResultBtn = document.getElementById("save-result-btn");

    if (!urlInput || !resultDiv) return;

    // 環境チェック完了確認
    if (!environmentCheckCompleted) {
        resultDiv.innerHTML =
            '<div class="error">❌ 先に「環境チェック」を実行してください</div>';
        return;
    }

    const url = urlInput.value.trim();
    if (!url) {
        resultDiv.innerHTML = '<div class="error">URLを入力してください</div>';
        return;
    }

    resultDiv.innerHTML = '<div class="loading">疎通確認中...</div>';

    try {
        const result = (await invoke("ping_http_dual", {
            url,
            ignoreTlsErrors: false,
        })) as HttpPingDualResult;

        lastPingDualResult = result;

        let html = "";

        // 概要表示
        const ipv4Success = result.ipv4.success;
        const ipv6Success = result.ipv6.success;

        if (ipv4Success || ipv6Success) {
            html += '<div class="success">✅ 疎通確認成功</div>';
        } else {
            html += '<div class="error">❌ 疎通確認失敗</div>';
        }

        // DNS解決結果
        html += "<h3>🔍 DNS名前解決結果</h3>";
        html += "<div style='background: #f9f9f9; padding: 10px; border-radius: 4px; margin-bottom: 15px;'>";
        html += "<div style='display: grid; grid-template-columns: 1fr 1fr; gap: 15px;'>";

        // IPv4解決結果
        html += "<div>";
        html += "<strong>IPv4 (A record):</strong><br>";
        if (result.dns_resolution.ipv4_addresses.length > 0) {
            html += result.dns_resolution.ipv4_addresses.map(ip => `<code>${ip}</code>`).join(", ");
        } else {
            html += '<span style="color: #ff9800;">見つかりません</span>';
        }
        html += "</div>";

        // IPv6解決結果
        html += "<div>";
        html += "<strong>IPv6 (AAAA record):</strong><br>";
        if (result.dns_resolution.ipv6_addresses.length > 0) {
            html += result.dns_resolution.ipv6_addresses.map(ip => `<code>${ip}</code>`).join(", ");
        } else {
            html += '<span style="color: #ff9800;">見つかりません</span>';
        }
        html += "</div>";

        html += "</div>";
        html += "</div>";

        html += "<h3>結果詳細</h3>";
        html += "<div style='display: grid; grid-template-columns: 1fr 1fr; gap: 15px;'>";

        // IPv4 結果
        html += "<div style='border: 1px solid #e0e0e0; padding: 15px; border-radius: 4px;'>";
        html += "<h4 style='color: #4a90e2; margin-bottom: 10px;'>📡 IPv4限定</h4>";
        if (result.ipv4.success) {
            html += '<div style="color: #4caf50; font-weight: 600; margin-bottom: 10px;">✅ 接続成功</div>';
        } else {
            html += '<div style="color: #f44336; font-weight: 600; margin-bottom: 10px;">❌ 接続失敗</div>';
        }
        html += "<ul style='margin: 0; padding: 0 0 0 20px;'>";
        html += `<li><strong>URL:</strong> ${result.ipv4.url}</li>`;
        if (result.ipv4.ip_address) {
            html += `<li><strong>接続試行IPアドレス:</strong> <code>${result.ipv4.ip_address}</code></li>`;
        }
        if (result.ipv4.status_code !== undefined) {
            html += `<li><strong>ステータスコード:</strong> ${result.ipv4.status_code}</li>`;
        }
        if (result.ipv4.response_time_ms !== undefined) {
            html += `<li><strong>レスポンス時間:</strong> ${result.ipv4.response_time_ms} ms</li>`;
        }
        if (result.ipv4.error_message) {
            html += `<li><strong>エラー:</strong> ${result.ipv4.error_message}</li>`;
        }
        html += "</ul>";
        html += "</div>";

        // IPv6 結果
        html += "<div style='border: 1px solid #e0e0e0; padding: 15px; border-radius: 4px;'>";
        html += "<h4 style='color: #4a90e2; margin-bottom: 10px;'>📡 IPv6限定</h4>";
        if (result.ipv6.success) {
            html += '<div style="color: #4caf50; font-weight: 600; margin-bottom: 10px;">✅ 接続成功</div>';
        } else {
            html += '<div style="color: #f44336; font-weight: 600; margin-bottom: 10px;">❌ 接続失敗</div>';
        }
        html += "<ul style='margin: 0; padding: 0 0 0 20px;'>";
        html += `<li><strong>URL:</strong> ${result.ipv6.url}</li>`;
        if (result.ipv6.ip_address) {
            html += `<li><strong>接続試行IPアドレス:</strong> <code>${result.ipv6.ip_address}</code></li>`;
        }
        if (result.ipv6.status_code !== undefined) {
            html += `<li><strong>ステータスコード:</strong> ${result.ipv6.status_code}</li>`;
        }
        if (result.ipv6.response_time_ms !== undefined) {
            html += `<li><strong>レスポンス時間:</strong> ${result.ipv6.response_time_ms} ms</li>`;
        }
        if (result.ipv6.error_message) {
            html += `<li><strong>エラー:</strong> ${result.ipv6.error_message}</li>`;
        }
        html += "</ul>";
        html += "</div>";

        html += "</div>";

        resultDiv.innerHTML = html;

        // ファイル保存ボタンを有効化
        if (saveResultBtn) {
            saveResultBtn.removeAttribute("disabled");
        }
    } catch (error) {
        resultDiv.innerHTML = `<div class="error">エラーが発生しました: ${error}</div>`;
    }
}

// 結果をテキストファイルに保存
async function saveResultAsTextFile() {
    let body = "=== ghttpping-tauri 疎通確認結果 ===\n\n";

    if (lastEnvResult) {
        body += "■ 環境チェック結果\n";
        body += `インターネット接続: ${lastEnvResult.internet_available ? "可能" : "不可"}\n`;
        body += `IPv4接続: ${lastEnvResult.ipv4_connectivity ? "あり" : "なし"}\n`;
        body += `IPv6接続: ${lastEnvResult.ipv6_connectivity ? "あり" : "なし"}\n`;
        body += `DNS解決: ${lastEnvResult.dns_resolution ? "可能" : "不可"}\n\n`;

        // グローバルIPアドレス情報
        if (lastEnvResult.ipv4_global_ip || lastEnvResult.ipv6_global_ip) {
            body += "【グローバルIPアドレス】\n";
            if (lastEnvResult.ipv4_global_ip) {
                body += `IPv4: ${lastEnvResult.ipv4_global_ip.client_host}\n`;
                body += `  (取得時刻: ${lastEnvResult.ipv4_global_ip.datetime_jst})\n`;
            }
            if (lastEnvResult.ipv6_global_ip) {
                body += `IPv6: ${lastEnvResult.ipv6_global_ip.client_host}\n`;
                body += `  (取得時刻: ${lastEnvResult.ipv6_global_ip.datetime_jst})\n`;
            }
            body += "\n";
        }

        // ネットワークアダプタ情報
        if (lastEnvResult.adapters.length > 0) {
            body += "【ネットワークアダプタ】\n";
            lastEnvResult.adapters.forEach((adapter) => {
                body += `  - ${adapter.name}\n`;
                body += `    IPv4: ${adapter.has_ipv4 ? "あり" : "なし"}`;
                if (adapter.has_ipv4_global) body += " (グローバル)";
                body += `\n    IPv6: ${adapter.has_ipv6 ? "あり" : "なし"}`;
                if (adapter.has_ipv6_global) body += " (グローバル)";
                if (adapter.ip_addresses.length > 0) {
                    body += `\n    IPアドレス: ${adapter.ip_addresses.join(", ")}`;
                }
                body += "\n";
            });
            body += "\n";
        }

        // DNSサーバー情報
        if (lastEnvResult.dns_servers.length > 0) {
            body += "【DNSサーバ設定】\n";
            lastEnvResult.dns_servers.forEach((dns) => {
                if (dns.ipv4_dns_servers.length > 0 || dns.ipv6_dns_servers.length > 0) {
                    body += `  ${dns.interface_alias}\n`;
                    if (dns.ipv4_dns_servers.length > 0) {
                        body += `    IPv4 DNSサーバ:\n`;
                        dns.ipv4_dns_servers.forEach((server, idx) => {
                            const label = idx === 0 ? "Primary" : idx === 1 ? "Secondary" : `(${idx + 1})`;
                            body += `      ${label}: ${server}\n`;
                        });
                    }
                    if (dns.ipv6_dns_servers.length > 0) {
                        body += `    IPv6 DNSサーバ:\n`;
                        dns.ipv6_dns_servers.forEach((server, idx) => {
                            const label = idx === 0 ? "Primary" : idx === 1 ? "Secondary" : `(${idx + 1})`;
                            body += `      ${label}: ${server}\n`;
                        });
                    }
                }
            });
            body += "\n";
        }

        // エラーメッセージ
        if (lastEnvResult.error_messages.length > 0) {
            body += "【エラー・警告】\n";
            lastEnvResult.error_messages.forEach((msg) => {
                body += `  - ${msg}\n`;
            });
            body += "\n";
        }
    }

    if (lastPingDualResult) {
        body += "■ 疎通確認結果\n";
        body += `URL: ${lastPingDualResult.url}\n\n`;

        // DNS解決結果
        body += "【DNS名前解決結果】\n";
        if (lastPingDualResult.dns_resolution.ipv4_addresses.length > 0) {
            body += `IPv4 (A record): ${lastPingDualResult.dns_resolution.ipv4_addresses.join(", ")}\n`;
        } else {
            body += "IPv4 (A record): 見つかりません\n";
        }
        if (lastPingDualResult.dns_resolution.ipv6_addresses.length > 0) {
            body += `IPv6 (AAAA record): ${lastPingDualResult.dns_resolution.ipv6_addresses.join(", ")}\n`;
        } else {
            body += "IPv6 (AAAA record): 見つかりません\n";
        }
        body += "\n";

        body += "【IPv4限定テスト】\n";
        if (lastPingDualResult.ipv4.ip_address) {
            body += `接続試行IPアドレス: ${lastPingDualResult.ipv4.ip_address}\n`;
        }
        body += `結果: ${lastPingDualResult.ipv4.success ? "成功" : "失敗"}\n`;
        if (lastPingDualResult.ipv4.status_code !== undefined) {
            body += `ステータスコード: ${lastPingDualResult.ipv4.status_code}\n`;
        }
        if (lastPingDualResult.ipv4.response_time_ms !== undefined) {
            body += `レスポンス時間: ${lastPingDualResult.ipv4.response_time_ms} ms\n`;
        }
        if (lastPingDualResult.ipv4.error_message) {
            body += `エラー: ${lastPingDualResult.ipv4.error_message}\n`;
        }

        body += "\n【IPv6限定テスト】\n";
        if (lastPingDualResult.ipv6.ip_address) {
            body += `接続試行IPアドレス: ${lastPingDualResult.ipv6.ip_address}\n`;
        }
        body += `結果: ${lastPingDualResult.ipv6.success ? "成功" : "失敗"}\n`;
        if (lastPingDualResult.ipv6.status_code !== undefined) {
            body += `ステータスコード: ${lastPingDualResult.ipv6.status_code}\n`;
        }
        if (lastPingDualResult.ipv6.response_time_ms !== undefined) {
            body += `レスポンス時間: ${lastPingDualResult.ipv6.response_time_ms} ms\n`;
        }
        if (lastPingDualResult.ipv6.error_message) {
            body += `エラー: ${lastPingDualResult.ipv6.error_message}\n`;
        }
    }

    try {
        // ファイル保存ダイアログを開く
        const filePath = await save({
            filters: [
                {
                    name: "Text",
                    extensions: ["txt"],
                },
            ],
            defaultPath: `ghttpping_tauri_result_${new Date().toISOString().replace(/[:.]/g, "-").slice(0, -5)}.txt`,
        });

        console.log("Dialog result:", filePath);

        if (filePath) {
            console.log("Saving to:", filePath);
            try {
                // テキストファイルを保存
                await writeTextFile(filePath, body);
                console.log("File saved successfully");
                alert("ファイルを保存しました:\n" + filePath);
            } catch (writeError) {
                console.error("Write error:", writeError);
                alert(`ファイル保存エラー: ${writeError}`);
            }
        } else {
            console.log("Dialog was cancelled");
        }
    } catch (error) {
        console.error("Save dialog error:", error);
        alert(`ダイアログエラー: ${error}`);
    }
}
