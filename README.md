# markitdown-mcp (Rust)

`microsoft/markitdown-mcp` 互換の MCP サーバーを Rust で実装したもの。URI（`http` / `https` / `file` / `data`）で指定した文書を Markdown に変換する単一ツール `convert_to_markdown` を公開する。

## 構成

2-crate の Cargo workspace:

- `crates/markitdown` — 変換エンジン（ライブラリ）。URI 取得層とフォーマット別コンバーターを分離。
- `crates/markitdown-mcp` — MCP サーバー（バイナリ）。[`rmcp`](https://crates.io/crates/rmcp) を利用。

## 対応フォーマット

PDF / Word(DOCX) / Excel(XLSX, XLS) / PowerPoint(PPTX) / HTML / CSV / JSON / XML / プレーンテキスト / ZIP（内包ファイルを再帰展開して連結）/ EPub。

> PDF は本文テキスト抽出ベース（レイアウト・表の完全再現は非保証）。音声文字起こし・YouTube・Azure・LLM 画像説明は対象外。

## 対応スキーム

`http:` / `https:` / `file:` / `data:`

## ビルド

```bash
cargo build --release
# 生成物: target/release/markitdown-mcp
```

## 起動（stdio）

```bash
target/release/markitdown-mcp
```

`--http` / `--host` / `--port` オプションは将来の HTTP/SSE トランスポート用に予約（v1 は stdio のみ）。ログは stderr に出力する（stdout は MCP の JSON-RPC 専用）。

## MCP クライアント登録例（Claude Code / Claude Desktop など）

```json
{
  "mcpServers": {
    "markitdown-rust": {
      "command": "/absolute/path/to/target/release/markitdown-mcp"
    }
  }
}
```

既存の Python 版 `markitdown-mcp` 設定からは `command` を差し替えるだけで利用できる（ツール名・引数・対応スキーム互換）。

## ツール

| name | 引数 | 返り値 |
|------|------|--------|
| `convert_to_markdown` | `uri: string`（http/https/file/data） | Markdown テキスト。失敗時は `isError: true` + 理由メッセージ |

詳細は [`specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md`](specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md)。

## ライブラリとして利用

```rust
let markdown = markitdown::convert_to_string("file:///path/to/doc.pdf")?;
```

## 開発

```bash
cargo test --workspace                       # 全テスト
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# テスト用フィクスチャの再生成（標準ライブラリのみ使用）
python scripts/gen_fixtures.py
```

## ライセンス

MIT
