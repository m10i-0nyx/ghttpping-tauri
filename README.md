# ghttpping-tauri

Windows上でHTTP/HTTPSの疎通確認および遅延測定を行うGUIアプリケーションです。

## 特徴

- ✅ **環境チェック** - ネットワークアダプタ情報、IPv4/IPv6接続状況、DNS解決、インターネット接続確認
- ✅ **グローバルIP取得** - IPv4/IPv6のグローバルIPアドレス情報を自動取得
- ✅ **DNS情報表示** - ネットワークインターフェースごとのDNSサーバー情報を表示
- ✅ **デュアルスタック疎通確認** - HTTP/HTTPSのIPv4/IPv6両対応テスト
- ✅ **詳細な接続情報** - DNS名前解決結果、ステータスコード、レスポンス時間を表示
- ✅ **結果管理** - テキストファイルへの保存

## 技術スタック

- **GUI Framework**: Tauri v2
- **Frontend**: TypeScript + Vite
- **Backend**: Rust
- **対象OS**: Windows 11

## 前提条件

- Windows 11
- Node.js (v24以上)
- pnpm
- Rust (1.70以上)
- Tauri CLI

## セットアップ

### 1. 依存関係のインストール

```powershell
pnpm install
```

### 2. Rustのセットアップ

Rustがインストールされていない場合:

```powershell
# Rustupをインストール
winget install --id Rustlang.Rustup
```

### 3. 開発サーバーの起動

```powershell
pnpm tauri dev
```

### 4. ビルド

```powershell
pnpm tauri build
```

ビルドされた実行ファイルは `src-tauri/target/release/` に生成されます。

## 使用方法

### 環境チェック

1. アプリを起動
2. 「環境を確認」ボタンをクリック
3. 結果が表示されます

### HTTP/HTTPS疎通確認

1. URLを入力（例: https://example.com）
2. 必要に応じて「TLS証明書の検証を無効化」をチェック（自己署証明書のテスト用）
3. 「疎通確認を実行」ボタンをクリック
4. 以下の結果が表示されます：
   - **DNS名前解決結果** - IPv4 (A record) / IPv6 (AAAA record) の解決情報
   - **保存・共有

#### テキストファイルに保存
1. 環境チェックまたは疎通確認を実行後、「結果をファイルに保存」ボタンが有効になります
2. クリックすると、保存先を指定するダイアログが表示されます
3. ファイル名は自動生成されます（例: `ghttpping_tauri_result_2026-02-09T10-30-45.txt`）

#### メールで送信
1. 結果を保存後、「結果をメールで送信」ボタンをクリック
2. デフォルトのメールクライアントが起動し、結果を本文に含めて送信でき
### 結果のメール送信

1. 疎通確認を実行後、「結果をメールで送信」ボタンが有効になります
2. クリックすると、デフォルトのメールクライアントが起動します

## プロジェクト構造

```
ghttpping-tauri/
├── src/                  # Frontend (TypeScript)
│   ├── main.ts
│   └── style.css
├── src-tauri/            # Backend (Rust)
│   ├── src/
│   │   ├── main.rs
│   │   └── lib.rs
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── build.rs
├── index.html
├── package.json
├─**OS操作の厳格な分離** - すべてのOS操作（ネットワーク診断、HTTP通信）はRust側で実行
- **TypeScriptからのコマンド実行禁止** - TypeScriptから直接OSコマンドを実行しない
- **Tauri allowlistの最小化** - 必要最小限の権限のみを使用
- **明示的な証明書検証制御** - TLS証明書検証の無効化は利用者の明示的な操作でのみ可能
- **ネットワーク通信の透明化** - DNS解決結果や接続IPアドレスを明示的に表示
## セキュリティ

- OS操作は全てRust側で実行
- TypeScriptから直接OSコマンドを実行しない
- Tauri allowlistを最小化
- 証明書検証の無効化は明示的な操作でのみ可能
主な Tauri Commands

利用可能な Tauri Commands：

- `environment_check()` - 環境チェックを実行
- `ping_http_dual(url: string, ignoreTlsErrors: boolean)` - HTTP/HTTPSのIPv4/IPv6デュアルテスト
- `resolve_dns(domain: string)` - DNS名前解決

### Frontend から Tauri Command を呼び出し

```typescript
import { invoke } from "@tauri-apps/api/core";

// 環境チェック
const envResult = await invoke("environment_check");

// HTTP/HTTPSテスト（IPv4/IPv6デュアル）
const pingResult = await invoke("ping_http_dual", {
    url: "https://example.com",
    ignoreTlsErrors: false
});
```

### ビルド済みバイナリの場所

開発ビルド: `src-tauri/target/debug/`
リリースビルド: `src-tauri/target/release/
### Frontend から呼び出し

```typescript
import { invoke } from "@tauri-apps/api/core";

const result = await invoke("my_command");
```

## トラブルシューティング

### ビルドエラー

```powershell
# Rustツールチェーンの更新
rustup update
ネットワーク接続を確認
- ファイアウォール設定を確認（特にDNS、IPv6）
- ゲストOSやVPN環境ではIPv6が利用できないことがあります

### HTTP/HTTPS疎通確認が失敗する

- URLが正しく入力されているか確認
- ファイアウォールがHTTP/HTTPSを許可しているか確認
- 対象サーバーがオンラインか確認
- TLS証明書エラーの場合は「TLS証明書の検証を無効化」をチェックして再試行--force
```

### 環境チェックが失敗する

- PowerShellの実行ポリシーを確認
- ファイアウォール設定を確認
- ネットワーク接続を確認

---

## 参考リンク
- [Tauri Documentation](https://tauri.app/)
