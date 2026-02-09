---
applyTo: "**"
---
# ghttpping - プロジェクト指針
## 目的（Goal）

本リポジトリは、**Windows上 で HTTP/HTTPS の 疎通確認および遅延測定 GUI アプリ**を構築することを目的とする。

GUI フレームワークは **Tauri v2** を採用する。

---

## 技術スタック（Must）

- OS: Windows 11
- GUI: Tauri v2
- Frontend: TypeScript + Web UI（vanilla-ts）
- Backend: Rust（Tauri command）

---

## 明確な制約（Hard Constraints）

- ❌ C / C++ は使用しない
- ❌ Electron は使用しない
- ❌ TS から OS コマンドを直接実行しない
- ✅ OS 操作は **必ず Rust 側（Tauri command）に閉じ込める**

---

## アーキテクチャ方針

## 必須機能（Must Have）

### 1. 環境チェック（Environment Check）

Rust 側で以下を判定すること：

- IPv4 / IPv6 ネットワークアダプタの有無
- IPv4 Private or Global アドレスの有無
- IPv6 Private or Global アドレスの有無
- 外部(https://getipv4.0nyx.net/)への IPv4 接続可否
- 外部(https://getipv6.0nyx.net/)への IPv6 接続可否
- DNS 解決可否（例: example.com）

これらを総合して：

- ✅ インターネット疎通可能
- ❌ インターネット疎通不可

を判定できること。

---

### 2. 特定Webサイト疎通確認（HTTP/HTTPS Ping）

以下を実行できること：

- 一般利用者がGUI上よりWebサイトを入力し、HTTP/HTTPS リクエストを送信
- レスポンスステータスコードを受け取り、GUI上に表示
- レスポンス時間（ミリ秒）を計測し、GUI上に表示
- TLS 証明書の有効期限を取得し、GUI上に表示（HTTPS のみ）
- エラー発生時は、エラー内容を GUI 上に表示
- TLS証明書の照合をGUI上より手動で無効化できるチェックボックスを提供

---

## セキュリティ方針（重要）

- `tauri.conf.json` では allowlist を **最小化**
- TS 側から任意コマンド実行は禁止
- Rust 側で明示的に許可した操作のみ実装

---

## 実装ガイドライン（Rust）

- OS コマンド実行は `std::process::Command`
- エラーハンドリング：コマンド実行失敗時は理由（インストール未了など）を返す
- 出力は構造化フォーマット（JSON）で返す

---

## 実装ガイドライン（Frontend）

- Rust command は `invoke()` 経由でのみ呼び出す
- エラーは「次にやるべき行動」が分かる文言にする

---

## NG 実装例（禁止）

- Electron ベースの提案
- 管理者権限を常時要求する設計
- Windows 以外を想定した抽象化

---

## 最終ゴール

- 一般利用者が、ボタン1クリックで特定のWebサイト疎通が確認できること
- アウトプットを自動で出力し、mailto:リンクで送信できること
- 技術的前提をユーザーに意識させないツール
