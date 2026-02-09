# ghttpping-tauri

Windows上でHTTP/HTTPSの疎通確認および遅延測定を行うGUIアプリケーションです。

## 特徴

- ✅ 環境チェック（IPv4/IPv6接続、DNS解決）
- ✅ HTTP/HTTPS疎通確認
- ✅ レスポンス時間測定
- ✅ TLS証明書情報取得
- ✅ 結果をメールで送信

## 技術スタック

- **GUI Framework**: Tauri v2
- **Frontend**: TypeScript + Vite
- **Backend**: Rust
- **対象OS**: Windows 11

## 前提条件

- Windows 11
- Node.js (v18以上)
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

### Webサイト疎通確認

1. URLを入力（例: https://example.com）
2. 必要に応じて「TLS証明書の検証を無効化」をチェック
3. 「疎通確認を実行」ボタンをクリック
4. 結果が表示されます

### 結果のメール送信

1. 疎通確認を実行後、「結果をメールで送信」ボタンが有効になります
2. クリックすると、デフォルトのメールクライアントが起動します

## プロジェクト構造

```
ghttpping-windows/
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
├── vite.config.ts
└── README.md
```

## セキュリティ

- OS操作は全てRust側で実行
- TypeScriptから直接OSコマンドを実行しない
- Tauri allowlistを最小化
- 証明書検証の無効化は明示的な操作でのみ可能

## ライセンス

See [LICENSE](LICENSE) file.

## 開発者向けメモ

### Tauri Command の追加

`src-tauri/src/lib.rs` にコマンドを追加し、`invoke_handler` に登録:

```rust
#[tauri::command]
fn my_command() -> String {
    "Hello from Rust!".to_string()
}

// invoke_handler に追加
.invoke_handler(tauri::generate_handler![check_environment, http_ping, my_command])
```

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

# 依存関係の再インストール
pnpm install --force
```

### 環境チェックが失敗する

- PowerShellの実行ポリシーを確認
- ファイアウォール設定を確認
- ネットワーク接続を確認

## 今後の改善予定

- [ ] TLS証明書の詳細情報取得の完全実装
- [ ] 複数URLの一括チェック機能
- [ ] 結果のエクスポート（CSV、JSON）
- [ ] 定期実行機能

---

© 2026 ghttpping-windows

- [Tauri Documentation](https://tauri.app/)
