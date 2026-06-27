# markitdown-mcp (Rust)

A Rust implementation of a `microsoft/markitdown-mcp`-compatible MCP server. Exposes a single tool `convert_to_markdown` that converts documents specified by URI (`http` / `https` / `file` / `data`) to Markdown.

## Structure

A 2-crate Cargo workspace:

- `crates/markitdown` — conversion engine (library). Separates the URI fetching layer from format-specific converters.
- `crates/markitdown-mcp` — MCP server (binary). Uses [`rmcp`](https://crates.io/crates/rmcp).

## Supported Formats

PDF / Word (DOCX) / Excel (XLSX, XLS) / PowerPoint (PPTX) / HTML / CSV / JSON / XML / Plain text / ZIP (recursively expands and concatenates contained files) / EPub.

> PDF conversion is text-extraction based (layout and table fidelity is not guaranteed). Audio transcription, YouTube, Azure, and LLM image description are not supported.

## Supported Schemes

`http:` / `https:` / `file:` / `data:`

## Build

```bash
cargo build --release
# Output: target/release/markitdown-mcp
```

## Run (stdio)

```bash
target/release/markitdown-mcp
```

The `--http` / `--host` / `--port` options are reserved for future HTTP/SSE transport support (v1 is stdio only). Logs are written to stderr (stdout is reserved for MCP JSON-RPC).

## MCP Client Registration (Claude Code / Claude Desktop, etc.)

```json
{
  "mcpServers": {
    "markitdown-rust": {
      "command": "/absolute/path/to/target/release/markitdown-mcp"
    }
  }
}
```

You can switch from the existing Python `markitdown-mcp` by simply replacing the `command` value (tool name, arguments, and supported schemes are compatible).

## Tool

| name | Arguments | Return value |
|------|-----------|--------------|
| `convert_to_markdown` | `uri: string` (http/https/file/data) | Markdown text. On failure: `isError: true` + reason message |

For details, see [`specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md`](specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md).

## Library Usage

```rust
let markdown = markitdown::convert_to_string("file:///path/to/doc.pdf")?;
```

## Development

```bash
cargo test --workspace                       # run all tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# Regenerate test fixtures (standard library only)
python scripts/gen_fixtures.py
```

## License

MIT
