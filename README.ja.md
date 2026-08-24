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

1. まず `gemini-flash-lite-latest`（`thinkingLevel: low`）で変換します。
2. 結果がヒューリスティック判定で不十分（出力が空・文字化け・PDF サイズに対して文字密度が著しく低い・`finishReason` が不完全）な場合、同じモデルを `thinkingLevel: high` で自動的に再試行します。エスカレーションで買うのは思考の深さであり、より大きいモデルではありません。
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

モデルと thinking level は固定です（一次 `gemini-flash-lite-latest`/`thinkingLevel=low`、エスカレーション `gemini-flash-lite-latest`/`thinkingLevel=high`）。`-latest` エイリアスは意図的な選択です。具体名のモデルは退役します（`gemini-2.5-flash-lite` は 2026-08-05 に 404 を返し始めた）が、エイリアスは Google 側のポインタに追従するためです。

## 対応スキーム

`http:` / `https:` / `file:` / `data:`

## Claude Code へのインストール

同梱スクリプトを使うのが最も簡単です。リポジトリを clone し、`cd` してから実行します:

```bash
./install_claude_plugin.sh          # ビルド + Claude Code プラグインとしてインストール
./install_claude_plugin.sh -n       # 再ビルドせずインストール（target/release/ を再利用）
./install_claude_plugin.sh -d       # プラグインをアンインストール + 導入バイナリを削除
```

要件: `cargo` と `claude` CLI が `PATH` にあること（プラグインの MCP サーバーをローカルバイナリへ向け直すために `node` を使用）。スクリプトはリリースバイナリをビルドして `~/.local/bin` に導入し、ローカル marketplace（`markitdown-gemini`）を登録、`markitdown-gemini` プラグインをインストールし、プラグインの MCP サーバーを導入バイナリへ向けます。実行後は Claude Code を再起動し、`claude plugin list` で確認してください。

環境変数による上書き:

| 変数 | 既定値 | 説明 |
|------|--------|------|
| `MARKITDOWN_BIN_DIR` | `~/.local/bin` | `markitdown-mcp` バイナリの導入先ディレクトリ |

Gemini による PDF 変換を有効にするには、Claude Code がサーバーを起動する環境で `GEMINI_API_KEY` を export してください（[Gemini による PDF 変換](#gemini-による-pdf-変換任意)参照）。

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

上記のプラグインインストーラーを使わない場合は、バイナリを手動登録します:

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
