# Icons Directory

Tauriアプリのアイコンを配置するディレクトリです。

## 必要なアイコン

以下のアイコンファイルを配置してください：

- `32x32.png` - 32x32ピクセルのPNG
- `128x128.png` - 128x128ピクセルのPNG
- `128x128@2x.png` - 256x256ピクセルのPNG（Retina対応）
- `icon.icns` - macOS用（将来対応の場合）
- `icon.ico` - Windows用ICOファイル

## アイコン生成方法

元画像（SVGまたは高解像度PNG）から以下のコマンドでアイコンを生成できます：

```powershell
# Tauri CLIを使用してアイコンを自動生成
pnpm tauri icon src-tauri/icons/original.webp 
```

このコマンドにより、必要な全てのサイズのアイコンが自動生成されます。

## デフォルトアイコン

アイコンがない場合、Tauriはデフォルトアイコンを使用しますが、本番環境では必ず独自のアイコンを設定してください。
