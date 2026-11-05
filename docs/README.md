# Yarubeshitara! Official Documentation Site

CLIバレットジャーナルツール「**Yarubeshitara (`yarube`)**」の公式ドキュメントサイト（日本語 / 英語対応）です。

## 公開方法 (GitHub Pages)

本ディレクトリ（`docs/`）は、GitHub Pages の標準機能（`main` ブランチの `/docs` フォルダ配信）にそのまま対応しています。

### GitHub での有効化手順
1. リポジトリの **Settings** > **Pages** を開きます。
2. **Build and deployment** の Source で **Deploy from a branch** を選択します。
3. Branch で **`main`** を選択し、フォルダとして **`/docs`** を選択して **Save** をクリックします。
4. 数分後、`https://boxdate.github.io/yarubeshitara-cli/`（英語ページは `/en.html`）でサイトが公開されます。

## ファイル構成
- `index.html`: 日本語版ランディングページ
- `en.html`: 英語版ランディングページ（右上ヘッダーの `JA / EN` トグルで相互切り替え）
- `styles.css`: 自然なウォームチャコール基調のレスポンシブスタイルシート
- `main.js`: 4テーマ（`tokyo-night`, `monokai`, `nord`, `sepia`）のプレビュー切り替えとワンクリックコピー機能
