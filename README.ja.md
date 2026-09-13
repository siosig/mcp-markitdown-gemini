# markitdown-mcp (Rust)

`microsoft/markitdown-mcp` 互換の MCP サーバーの Rust 実装です。URI（`http` / `https` / `file` / `data`）で指定した文書を Markdown に変換する単一ツール `convert_to_markdown` を公開します。

> English version: [README.md](README.md)

## 構成

2 クレートの Cargo ワークスペース:

- `crates/markitdown` — 変換エンジン（ライブラリ）。URI 取得層とフォーマット別コンバーターを分離。
- `crates/markitdown-mcp` — MCP サーバー（バイナリ）。[`rmcp`](https://crates.io/crates/rmcp) を使用。

## 対応フォーマット

PDF / Word (DOCX) / Excel (XLSX, XLS) / PowerPoint (PPTX) / HTML / CSV / JSON / XML / プレーンテキスト / ZIP（内包ファイルを再帰展開して連結）/ EPub。

> PDF 変換は既定ではテキスト抽出ベースです（レイアウト・表の完全再現は保証しません）。`GEMINI_API_KEY` を設定すると、より高精度な Gemini 変換に切り替わります（[Gemini による PDF 変換](#gemini-による-pdf-変換任意)を参照）。音声書き起こし・YouTube・Azure・LLM による画像説明には対応しません。

## Gemini による PDF 変換（任意）

環境変数 `GEMINI_API_KEY` が設定されている場合、**PDF** の変換はローカルのテキスト抽出ではなく Google Gemini API 経由で行われます:

1. まず `gemini-flash-lite-latest`（`thinking_level: LOW`）で変換します。
2. 結果がヒューリスティック判定で不十分（出力が空・文字化け・PDF サイズに対して文字密度が著しく低い・`finishReason` が不完全）な場合、同じモデルを `thinking_level: HIGH` で自動的に再試行します。エスカレーションで買うのは思考の深さであり、より大きいモデルではありません。
3. それでも有用な結果が得られない場合、またはハードエラー（無効なキー・レート制限・サーバーエラー・タイムアウト）の場合は、エラーを返します。ローカル抽出へ**暗黙的にフォールバックしません**。

`GEMINI_API_KEY` が未設定（または空白のみ）の場合は従来どおりで、PDF はローカル抽出を使用し、ネットワーク通信は一切発生しません。影響を受けるのは PDF のみで、他のフォーマットの変換挙動は完全に従来どおりです。

> 注: Gemini を有効化すると、PDF の内容が Google Gemini API（外部ネットワーク）へ送信されます。

### 環境変数

| 変数 | 既定値 | 説明 |
|------|--------|------|
| `GEMINI_API_KEY` | （未設定） | 非空のとき Gemini PDF 変換を有効化。`x-goog-api-key` ヘッダで送信。 |
| `GEMINI_BASE_URL` | `https://generativelanguage.googleapis.com` | API ベース URL の上書き（テスト用）。 |
| `GEMINI_INLINE_MAX_BYTES` | `20971520`（20 MB） | inline 送信する PDF の上限。超過はエラー（File API 対応は今後）。 |
| `GEMINI_TIMEOUT_SECS` | `120` | HTTP タイムアウト（秒）。 |
| `GEMINI_GARBLE_RATIO` | `0.02` | U+FFFD 比率がこれを超えるとエスカレーション。 |
| `GEMINI_MIN_CHARS_PER_KB` | `5.0` | 出力の文字/KB がこれを下回るとエスカレーション（大きい PDF 向け）。 |
| `GEMINI_DENSITY_MIN_BYTES` | `51200`（50 KB） | 低密度判定を適用する PDF の最小サイズ。 |

モデルと thinking level は固定です（一次 `gemini-flash-lite-latest`/`thinking_level=LOW`、エスカレーション `gemini-flash-lite-latest`/`thinking_level=HIGH`）。`-latest` エイリアスは意図的な選択です。具体名のモデルは退役します（`gemini-2.5-flash-lite` は 2026-08-05 に 404 を返し始めた）が、エイリアスは Google 側のポインタに追従するためです。

## 対応スキーム

`http:` / `https:` / `file:` / `data:`

## Claude Code へのインストール

### Linux / macOS

リポジトリを clone し、`cd` してから実行します:

```bash
./install_claude_plugin.sh              # インストール（リリースのダウンロード、取得できなければソースビルドへフォールバック）
./install_claude_plugin.sh -s           # ソースビルドを強制（ダウンロードを試みない）
./install_claude_plugin.sh -d           # プラグインをアンインストール + 導入バイナリを削除
```

要件: `claude` CLI が `PATH` にあること。リリースアーカイブの取得に `curl` と `tar` を使用し、ソースビルドのフォールバック時のみ `cargo` が必要です。許可設定の投入には `node` を使用します。

インストーラはまず、このプロジェクトの [GitHub Releases](https://github.com/siosig/mcp-markitdown-gemini/releases) から、実行環境（Linux x86_64/aarch64、macOS aarch64）向けのビルド済みバイナリのダウンロードを試みます。該当するリリース資産が取得できない場合は `cargo build --release` にフォールバックします。どちらの経路でも、バイナリはこのチェックアウト内の `plugins/markitdown-gemini/bin/markitdown-mcp.exe` に配置され（`.exe` 拡張子は全 OS で共通。理由は後述）、ローカル marketplace（`markitdown-gemini`）を登録、`markitdown-gemini` プラグインをインストールし、本プラグインの MCP サーバーが提供する全ツールを承認プロンプトなしで実行できる許可を付与します（将来ツールが増えても再インストール不要で対象に含まれます）。この方式より前のインストーラ版が残したバイナリ（Rust のユーザースコープ標準バイナリディレクトリ、または `~/.local/bin`）は、インストール・アンインストールの両方で自動的に削除されます。実行後は Claude Code を再起動し、`claude mcp list` で確認してください。

### Windows

[PowerShell 7 以降](https://learn.microsoft.com/ja-jp/powershell/scripting/install/installing-powershell)（`winget install --id Microsoft.PowerShell`）と、`claude` CLI が `PATH` にあることが必要です。Rust ツールチェーンは不要です — Windows 用インストーラはソースからビルドすることが一切なく、[GitHub Releases](https://github.com/siosig/mcp-markitdown-gemini/releases) からビルド済みの `x86_64` バイナリをダウンロードするだけです。

```powershell
git clone https://github.com/siosig/mcp-markitdown-gemini
cd mcp-markitdown-gemini
.\install_claude_plugin.ps1              # インストール
.\install_claude_plugin.ps1 -Uninstall   # アンインストール
.\install_claude_plugin.ps1 -Help        # 使い方
```

実行ポリシーでブロックされた場合は `pwsh -ExecutionPolicy Bypass -File .\install_claude_plugin.ps1` を実行してください。Claude Code が起動中だとバイナリの置き換えに失敗することがあります — その場合は Claude Code を終了してから再実行してください。配置するバイナリは Linux/macOS を含む全 OS で `markitdown-mcp.exe` という名前に統一しています。プラグインの起動設定（`${CLAUDE_PLUGIN_ROOT}/bin/...`）はコマンド文字列を 1 つしか持てず、Windows は拡張子なしの実行ファイルをプラグインローダー経由で起動できないためです。

### 共通

Gemini による PDF 変換を有効にするには、Claude Code がサーバーを起動する環境で `GEMINI_API_KEY` を export してください（[Gemini による PDF 変換](#gemini-による-pdf-変換任意)参照）。これは任意機能で、未設定でもインストーラは動作します。

ソースビルド時、`gemini-genai` 依存の取得中に `git@github.com: Permission denied (publickey)` でビルドが失敗する場合、グローバルな git 設定が匿名 HTTPS の GitHub URL を SSH へ書き換えており、その書き換え先の鍵が `github.com/siosig` に届いていません。元のルールを削除するのではなく、より長い自己マッピングを追加してください（git は最長一致の接頭辞を採用します）:

```bash
git config --global url."https://github.com/siosig/".insteadOf "https://github.com/siosig/"
```

## ビルド

```bash
cargo build --release
# 出力: target/release/markitdown-mcp
```

## 実行（stdio）

```bash
target/release/markitdown-mcp
```

`--http` / `--host` / `--port` オプションは将来の HTTP/SSE トランスポート用に予約済みです（v1 は stdio のみ）。ログは stderr に出力されます（stdout は MCP JSON-RPC 専用）。

## 手動での MCP クライアント登録（Claude Desktop ほか）

上記のプラグインインストーラーを使わない場合は、バイナリを手動登録します。自分でビルドする（`cargo build --release`。[ビルド](#ビルド)参照）か、[GitHub Releases](https://github.com/siosig/mcp-markitdown-gemini/releases) から取得してください:

```json
{
  "mcpServers": {
    "markitdown-gemini": {
      "command": "/absolute/path/to/target/release/markitdown-mcp"
    }
  }
}
```

Gemini による PDF 変換を有効にするには、サーバーの環境変数に `GEMINI_API_KEY` を追加します:

```json
{
  "mcpServers": {
    "markitdown-gemini": {
      "command": "/absolute/path/to/target/release/markitdown-mcp",
      "env": { "GEMINI_API_KEY": "your-api-key" }
    }
  }
}
```

既存の Python 版 `markitdown-mcp` からは、`command` の値を差し替えるだけで移行できます（ツール名・引数・対応スキームは互換）。

## ツール

| name | 引数 | 戻り値 |
|------|------|--------|
| `convert_to_markdown` | `uri: string`（http/https/file/data） | Markdown テキスト。失敗時: `isError: true` + 理由メッセージ |

詳細は [`specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md`](specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md) を参照。Gemini による PDF 変換の挙動は [`specs/002-pdf-gemini-conversion/contracts/gemini_pdf_conversion.md`](specs/002-pdf-gemini-conversion/contracts/gemini_pdf_conversion.md) を参照。

## ライブラリとしての利用

```rust
let markdown = markitdown::convert_to_string("file:///path/to/doc.pdf")?;
```

## 開発

```bash
cargo test --workspace                       # 全テスト実行
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# テスト用フィクスチャの再生成（標準ライブラリのみ）
python scripts/gen_fixtures.py
```

## ライセンス

MIT
