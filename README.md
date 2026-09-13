# markitdown-mcp (Rust)

A Rust implementation of a `microsoft/markitdown-mcp`-compatible MCP server. Exposes a single tool `convert_to_markdown` that converts documents specified by URI (`http` / `https` / `file` / `data`) to Markdown.

## Structure

A 2-crate Cargo workspace:

- `crates/markitdown` — conversion engine (library). Separates the URI fetching layer from format-specific converters.
- `crates/markitdown-mcp` — MCP server (binary). Uses [`rmcp`](https://crates.io/crates/rmcp).

## Supported Formats

PDF / Word (DOCX) / Excel (XLSX, XLS) / PowerPoint (PPTX) / HTML / CSV / JSON / XML / Plain text / ZIP (recursively expands and concatenates contained files) / EPub.

> By default, PDF conversion is text-extraction based (layout and table fidelity is not guaranteed). Optionally, when `GEMINI_API_KEY` is set, PDFs are converted via Gemini for higher fidelity — see [PDF Conversion via Gemini](#pdf-conversion-via-gemini-optional). Audio transcription, YouTube, Azure, and LLM image description are not supported.

## PDF Conversion via Gemini (optional)

When the `GEMINI_API_KEY` environment variable is set, **PDF** conversion is routed through the Google Gemini API instead of local text extraction:

1. The PDF is first converted with `gemini-flash-lite-latest` (`thinking_level: LOW`).
2. If the result is judged insufficient by a heuristic (empty output, garbled text, very low text density relative to the PDF size, or an incomplete `finishReason`), it automatically retries the same model at `thinking_level: HIGH`. The escalation buys reasoning depth, not a larger model.
3. If Gemini still cannot produce a usable result — or on a hard error (invalid key, rate limit, server error, timeout) — the request returns an error. It does **not** silently fall back to local extraction.

When `GEMINI_API_KEY` is unset (or empty/whitespace), behavior is unchanged: PDFs use the built-in local extractor and no network request is made. Only PDF is affected; all other formats convert exactly as before.

> Note: Enabling Gemini sends PDF contents to the Google Gemini API (external network).

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GEMINI_API_KEY` | (unset) | When non-empty, enables Gemini PDF conversion. Sent as the `x-goog-api-key` header. |
| `GEMINI_BASE_URL` | `https://generativelanguage.googleapis.com` | API base URL override (useful for testing). |
| `GEMINI_INLINE_MAX_BYTES` | `20971520` (20 MB) | Max PDF size sent inline; larger inputs return an error (File API support is planned). |
| `GEMINI_TIMEOUT_SECS` | `120` | HTTP request timeout (seconds). |
| `GEMINI_GARBLE_RATIO` | `0.02` | Escalate if the U+FFFD ratio exceeds this. |
| `GEMINI_MIN_CHARS_PER_KB` | `5.0` | Escalate if output chars-per-KB drops below this (for large PDFs). |
| `GEMINI_DENSITY_MIN_BYTES` | `51200` (50 KB) | The low-density check applies only to PDFs at least this large. |

Models and thinking levels are fixed (primary `gemini-flash-lite-latest`/`thinking_level=LOW`, escalation `gemini-flash-lite-latest`/`thinking_level=HIGH`). The `-latest` alias is deliberate: concrete model ids get retired (`gemini-2.5-flash-lite` started returning 404 on 2026-08-05), and the alias follows Google's pointer.

## Supported Schemes

`http:` / `https:` / `file:` / `data:`

## Install for Claude Code

### Linux / macOS

Clone the repo, `cd` into it, then run:

```bash
./install_claude_plugin.sh              # install (release download, falls back to a source build)
./install_claude_plugin.sh -s           # force a source build (skip the release download)
./install_claude_plugin.sh -d           # uninstall the plugin + remove the installed binary
```

Requirements: the `claude` CLI on `PATH`. `curl` and `tar` are used to fetch the release archive; `cargo` is only needed for the source-build fallback; `node` is used to grant tool permissions.

The installer first tries to download a prebuilt binary from this project's [GitHub Releases](https://github.com/siosig/mcp-markitdown-gemini/releases) for your platform (Linux x86_64/aarch64, macOS aarch64). If no matching release asset can be obtained, it falls back to `cargo build --release`. Either way the binary is placed at `plugins/markitdown-gemini/bin/markitdown-mcp.exe` inside this checkout (the `.exe` suffix is kept on every platform — see below), a local marketplace (`markitdown-gemini`) is registered, the `markitdown-gemini` plugin is installed, and this plugin's MCP server is granted permission to run all its tools without a per-call approval prompt (covers future tools too, with no reinstall needed). A binary left by an installer version predating this scheme (Rust's user-scope bin directory, or `~/.local/bin`) is removed automatically on both install and uninstall. Restart Claude Code afterward and verify with `claude mcp list`.

### Windows

Requires [PowerShell 7 or later](https://learn.microsoft.com/powershell/scripting/install/installing-powershell) (`winget install --id Microsoft.PowerShell`) and the `claude` CLI on `PATH`. No Rust toolchain is needed — the Windows installer never builds from source; it only downloads a prebuilt `x86_64` binary from [GitHub Releases](https://github.com/siosig/mcp-markitdown-gemini/releases).

```powershell
git clone https://github.com/siosig/mcp-markitdown-gemini
cd mcp-markitdown-gemini
.\install_claude_plugin.ps1              # install
.\install_claude_plugin.ps1 -Uninstall   # uninstall
.\install_claude_plugin.ps1 -Help        # usage
```

If script execution is blocked by policy, run: `pwsh -ExecutionPolicy Bypass -File .\install_claude_plugin.ps1`. If Claude Code is running, the installer may fail to replace the binary — quit Claude Code and re-run. The placed binary is always named `markitdown-mcp.exe`, even on Linux/macOS, because the plugin's launch configuration (`${CLAUDE_PLUGIN_ROOT}/bin/...`) admits only one command string, and Windows cannot launch an extension-less image through the plugin loader.

### Common

To enable Gemini PDF conversion, export `GEMINI_API_KEY` in the environment where Claude Code launches the server (see [PDF Conversion via Gemini](#pdf-conversion-via-gemini-optional)); it's optional and the installer works without it.

If a source build fails while fetching the `gemini-genai` dependency with `git@github.com: Permission denied (publickey)`, your global git config is rewriting anonymous HTTPS GitHub URLs to SSH and the key behind that rewrite can't reach `github.com/siosig`. Add a longer, self-mapping override (git uses the longest matching prefix) rather than removing the original rule:

```bash
git config --global url."https://github.com/siosig/".insteadOf "https://github.com/siosig/"
```

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

## Manual MCP Client Registration (Claude Desktop, other clients, etc.)

If you are not using the plugin installer above, register the binary manually — either build it yourself (`cargo build --release`, see [Build](#build)) or download it from [GitHub Releases](https://github.com/siosig/mcp-markitdown-gemini/releases):

```json
{
  "mcpServers": {
    "markitdown-gemini": {
      "command": "/absolute/path/to/target/release/markitdown-mcp"
    }
  }
}
```

To enable Gemini-based PDF conversion, add `GEMINI_API_KEY` to the server's environment:

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

You can switch from the existing Python `markitdown-mcp` by simply replacing the `command` value (tool name, arguments, and supported schemes are compatible).

## Tool

| name | Arguments | Return value |
|------|-----------|--------------|
| `convert_to_markdown` | `uri: string` (http/https/file/data) | Markdown text. On failure: `isError: true` + reason message |

For details, see [`specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md`](specs/001-markitdown-mcp-rust/contracts/convert_to_markdown.md). For the Gemini PDF conversion behavior, see [`specs/002-pdf-gemini-conversion/contracts/gemini_pdf_conversion.md`](specs/002-pdf-gemini-conversion/contracts/gemini_pdf_conversion.md).

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
