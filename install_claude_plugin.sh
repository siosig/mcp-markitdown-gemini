#!/usr/bin/env bash
set -euo pipefail

# Install / uninstall markitdown-mcp (Rust) as a Claude Code plugin.
#
# Usage model: clone the repository yourself, `cd` into it, and run this script.
# It installs FROM the current checkout — it does not clone anything. It builds
# the release binary, installs it onto PATH, registers the local marketplace,
# installs the plugin, and points the plugin's MCP server at the local binary.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd 2>/dev/null || pwd)"
BIN_DIR="${MARKITDOWN_BIN_DIR:-${HOME}/.local/bin}"
MARKETPLACE_NAME="markitdown-gemini"
PLUGIN_NAME="markitdown-gemini"

# Binary entrypoint shipped by the Rust workspace (the stdio MCP server).
BINS=(markitdown-mcp)

# cargo is usually in ~/.cargo/bin, which non-interactive shells may not have on PATH.
export PATH="${HOME}/.cargo/bin:${PATH}"

usage() {
  cat >&2 << EOF
Usage: ${0##*/} [-d|--uninstall] [-n|--no-build] [-h|--help]

  Clone the repo yourself, cd into it, then run this script.

  (no flag)        Install the markitdown-gemini plugin from this checkout:
                     • build the release binary (cargo build --release -p markitdown-mcp)
                     • install it into ${BIN_DIR}
                     • register the '${MARKETPLACE_NAME}' marketplace + install the plugin
                     • point the installed plugin's MCP server at the local binary
  -n, --no-build   Skip 'cargo build --release' and use the binary already in
                     target/release/ (useful when you already built manually).
  -d, --uninstall  Remove what the installer added (this repo folder is left untouched):
                     • uninstall the '${PLUGIN_NAME}' plugin
                     • remove the '${MARKETPLACE_NAME}' marketplace entry
                     • remove the installed binary from ${BIN_DIR}
  -h, --help       Show this help.

Env: MARKITDOWN_BIN_DIR (default ${HOME}/.local/bin)

Optional: export GEMINI_API_KEY before launching Claude Code to enable
Gemini-based PDF conversion (the MCP server inherits Claude Code's environment).
EOF
}

MODE="install"
BUILD=true
while [[ $# -gt 0 ]]; do
  case "$1" in
    -d|--uninstall|--delete) MODE="uninstall" ;;
    -n|--no-build) BUILD=false ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

_require() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: '$1' not found. Please install it and try again." >&2
    [[ -n "${2:-}" ]] && echo "       $2" >&2
    exit 1
  fi
  echo "✓ $1: $(command -v "$1")"
}

# Patch the *installed* plugin's plugin.json so the MCP server runs the local
# binary by absolute path (survives a `cargo clean` of this checkout). Idempotent;
# only touches the installed plugin cache, never the tracked source.
_point_plugin_at_binary() {
  local bin="$1"
  local db="${HOME}/.claude/plugins/installed_plugins.json"
  command -v node &>/dev/null || { echo "⚠ node missing — cannot repoint plugin MCP (skip; relies on PATH)"; return 0; }
  [[ -f "$db" ]] || { echo "⚠ ${db} absent — cannot repoint plugin MCP (skip; relies on PATH)"; return 0; }
  if node -e '
    const fs = require("fs");
    const [dbPath, pluginKey, serverName, binPath] = process.argv.slice(1);
    const db = JSON.parse(fs.readFileSync(dbPath, "utf8"));
    const entry = ((db.plugins && db.plugins[pluginKey]) || [])[0];
    if (!entry || !entry.installPath) { console.error("no installPath for " + pluginKey); process.exit(1); }
    const pj = entry.installPath + "/.claude-plugin/plugin.json";
    const p = JSON.parse(fs.readFileSync(pj, "utf8"));
    p.mcpServers = p.mcpServers || {};
    p.mcpServers[serverName] = { command: binPath, args: [] };
    fs.writeFileSync(pj, JSON.stringify(p, null, 2) + "\n");
  ' "$db" "${PLUGIN_NAME}@${MARKETPLACE_NAME}" "${PLUGIN_NAME}" "$bin"; then
    echo "✓ plugin MCP repointed to ${bin}"
  else
    echo "⚠ could not repoint plugin MCP — it will rely on '${BINS[0]}' being on PATH"
  fi
}

_uninstall() {
  echo "→ uninstalling markitdown-gemini (-d)"
  _require claude "https://claude.ai/code"

  if claude plugin list 2>/dev/null | grep -q "❯ ${PLUGIN_NAME}@"; then
    claude plugin uninstall "${PLUGIN_NAME}" --yes
    echo "✓ '${PLUGIN_NAME}': uninstalled"
  else
    echo "✓ '${PLUGIN_NAME}': not installed (skip)"
  fi

  if claude plugin marketplace list 2>/dev/null | grep -q "^  ❯ ${MARKETPLACE_NAME}"; then
    claude plugin marketplace remove "${MARKETPLACE_NAME}"
    echo "✓ marketplace '${MARKETPLACE_NAME}': removed"
  else
    echo "✓ marketplace '${MARKETPLACE_NAME}': not registered (skip)"
  fi

  local removed=0
  for b in "${BINS[@]}"; do
    if [[ -f "${BIN_DIR}/${b}" ]]; then rm -f "${BIN_DIR}/${b}"; removed=$((removed + 1)); fi
  done
  echo "✓ binaries: removed ${removed} from ${BIN_DIR}"
  echo "✓ repo folder: ${SCRIPT_DIR} left untouched (you cloned it; you manage it)"
  echo ""
  echo "Uninstall complete. Restart Claude Code to drop the plugin from the session."
}

if [[ "${MODE}" == "uninstall" ]]; then
  _uninstall
  exit 0
fi

# ── Install (from this checkout) ─────────────────────────────────────────────
_require claude "https://claude.ai/code"

# Sanity-check that we are inside a markitdown-gemini checkout.
if [[ ! -d "${SCRIPT_DIR}/crates" || ! -f "${SCRIPT_DIR}/.claude-plugin/marketplace.json" ]]; then
  echo "ERROR: this does not look like a markitdown-gemini checkout: ${SCRIPT_DIR}" >&2
  echo "       Clone the repo, cd into it, and run ./${0##*/} from there." >&2
  exit 1
fi
echo "✓ source: ${SCRIPT_DIR}"

# 1. Build the release binary.
if [[ "${BUILD}" == "true" ]]; then
  _require cargo "https://rustup.rs/"
  echo "→ building release binary (cargo build --release -p markitdown-mcp)"
  ( cd "${SCRIPT_DIR}" && cargo build --release -p markitdown-mcp )
  echo "✓ build complete"
else
  echo "✓ build skipped (--no-build); using existing target/release/ binary"
fi

# 2. Install the binary onto PATH.
mkdir -p "${BIN_DIR}"
for b in "${BINS[@]}"; do
  src="${SCRIPT_DIR}/target/release/${b}"
  if [[ ! -x "${src}" ]]; then
    echo "ERROR: binary not found: ${src} (run without --no-build)" >&2
    exit 1
  fi
  install -m 0755 "${src}" "${BIN_DIR}/${b}"
done
echo "✓ installed ${#BINS[@]} binary into ${BIN_DIR}"
case ":${PATH}:" in
  *":${BIN_DIR}:"*) ;;
  *) echo "⚠ ${BIN_DIR} is not on PATH — add it: export PATH=\"${BIN_DIR}:\$PATH\"" ;;
esac

# 3. Register the local marketplace (this checkout). Re-register if it points elsewhere.
if claude plugin marketplace list 2>/dev/null | grep -q "^  ❯ ${MARKETPLACE_NAME}"; then
  if claude plugin marketplace list 2>/dev/null | grep -A2 "^  ❯ ${MARKETPLACE_NAME}" | grep -q "Source: Directory (${SCRIPT_DIR})"; then
    echo "✓ marketplace '${MARKETPLACE_NAME}': already pointing to this checkout"
  else
    echo "→ re-registering marketplace '${MARKETPLACE_NAME}' → ${SCRIPT_DIR}"
    claude plugin marketplace remove "${MARKETPLACE_NAME}"
    claude plugin marketplace add "${SCRIPT_DIR}"
    echo "✓ marketplace '${MARKETPLACE_NAME}': updated to local checkout"
  fi
else
  echo "→ registering marketplace '${MARKETPLACE_NAME}': ${SCRIPT_DIR}"
  claude plugin marketplace add "${SCRIPT_DIR}"
  echo "✓ marketplace '${MARKETPLACE_NAME}': registered"
fi

# 4. (Re)install the plugin.
if claude plugin list 2>/dev/null | grep -q "❯ ${PLUGIN_NAME}@"; then
  echo "→ reinstalling existing '${PLUGIN_NAME}' plugin"
  claude plugin uninstall "${PLUGIN_NAME}" --yes
fi
claude plugin install "${PLUGIN_NAME}@${MARKETPLACE_NAME}"
echo "✓ '${PLUGIN_NAME}@${MARKETPLACE_NAME}': installed"

# 5. Point the installed plugin's MCP server at the local binary.
_point_plugin_at_binary "${BIN_DIR}/${BINS[0]}"

echo ""
echo "Installation complete. Restart Claude Code to activate the plugin."
echo "  • MCP tool: convert_to_markdown (http/https/file/data URIs → Markdown)"
echo "  • Optional: export GEMINI_API_KEY for Gemini-based PDF conversion"
