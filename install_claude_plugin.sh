#!/usr/bin/env bash
set -euo pipefail

# Install / uninstall markitdown-mcp (Rust) as a Claude Code plugin.
#
# Usage model: clone the repository yourself, `cd` into it, and run this script.
# It installs FROM the current checkout — it does not clone anything. It builds
# the release binary, installs it onto PATH, registers the local marketplace,
# installs the plugin, and points the plugin's MCP server at the local binary.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd 2>/dev/null || pwd)"
MARKETPLACE_NAME="markitdown-gemini"
PLUGIN_NAME="markitdown-gemini"

# Binary entrypoint shipped by the Rust workspace (the stdio MCP server).
BINS=(markitdown-mcp)

# Binary from an older installer version that used to live in ~/.local/bin
# (before 005-genai-git-deps-installer moved the default to the Rust
# user-scope bin dir). Cleaned up on both install and uninstall so a stale
# copy never shadows the current one on PATH.
LEGACY_BIN_DIR="${HOME}/.local/bin"

# Resolve the install directory in the same order `cargo install` would: an
# explicit override for this tool first, then cargo's own install root, then
# cargo's home, then its hardcoded default. Sets BIN_DIR and BIN_DIR_SOURCE;
# the latter is logged below so it's never a mystery where the binary landed
# (FR-012 / FR-012a).
if [[ -n "${MARKITDOWN_BIN_DIR:-}" ]]; then
  BIN_DIR="${MARKITDOWN_BIN_DIR}"
  BIN_DIR_SOURCE="MARKITDOWN_BIN_DIR"
elif [[ -n "${CARGO_INSTALL_ROOT:-}" ]]; then
  BIN_DIR="${CARGO_INSTALL_ROOT}/bin"
  BIN_DIR_SOURCE="CARGO_INSTALL_ROOT"
elif [[ -n "${CARGO_HOME:-}" ]]; then
  BIN_DIR="${CARGO_HOME}/bin"
  BIN_DIR_SOURCE="CARGO_HOME"
else
  BIN_DIR="${HOME}/.cargo/bin"
  BIN_DIR_SOURCE="default (~/.cargo/bin)"
fi

# cargo is usually in ~/.cargo/bin, which non-interactive shells may not have on PATH.
export PATH="${HOME}/.cargo/bin:${PATH}"

usage() {
  cat >&2 << EOF
Usage: ${0##*/} [-d|--uninstall] [-n|--no-build] [-h|--help]

  Clone the repo yourself, cd into it, then run this script.

  (no flag)        Install the markitdown-gemini plugin from this checkout:
                     • build the release binary (cargo build --release -p markitdown-mcp)
                     • install it into the Rust user-scope bin dir (see below)
                     • register the '${MARKETPLACE_NAME}' marketplace + install the plugin
                     • point the installed plugin's MCP server at the local binary
                     • grant this plugin's MCP server all-tools permission
  -n, --no-build   Skip 'cargo build --release' and use the binary already in
                     target/release/ (useful when you already built manually).
  -d, --uninstall  Remove what the installer added (this repo folder is left untouched):
                     • uninstall the '${PLUGIN_NAME}' plugin
                     • remove the '${MARKETPLACE_NAME}' marketplace entry
                     • remove the installed binary (current and legacy locations)
                     • revoke the granted permission
  -h, --help       Show this help.

Install directory (first match wins):
  1. \$MARKITDOWN_BIN_DIR       (explicit override for this installer)
  2. \$CARGO_INSTALL_ROOT/bin   (cargo's own install-root override)
  3. \$CARGO_HOME/bin           (cargo's home directory)
  4. \${HOME}/.cargo/bin         (Rust's default user-scope bin dir)
Currently resolves to: ${BIN_DIR} (via ${BIN_DIR_SOURCE})

Optional: export GEMINI_API_KEY before launching Claude Code to enable
Gemini-based PDF conversion (the MCP server inherits Claude Code's environment).

Troubleshooting: if building fails with
  "git@github.com: Permission denied (publickey)"
while fetching the gemini-genai dependency, your global git config is
rewriting anonymous HTTPS GitHub URLs to SSH (an "insteadOf" rule) and the
key behind that rewrite cannot reach github.com/siosig. This script does
not touch your git config. Fix it yourself with a longer, self-mapping
override (git uses the longest matching prefix), e.g.:
  git config --global url."https://github.com/siosig/".insteadOf "https://github.com/siosig/"
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

# Server-scoped permission rule granted during install: matches ANY tool this
# plugin's MCP server exposes (documented Claude Code allow-rule syntax —
# "mcp__<server>" with no "__<tool>" suffix matches every tool from that
# server). A new tool the server adds later is covered automatically, with no
# re-run of this installer needed (FR-014 / SC-008).
TOOL_PERMISSIONS=(
  "mcp__plugin_${PLUGIN_NAME}_${PLUGIN_NAME}"
)

# Tool-scoped rule granted by installer versions before 005-genai-git-deps-installer.
# Superseded by the server-scoped rule above; removed on install (so it isn't
# left behind as dead-but-harmless config) and on uninstall.
LEGACY_TOOL_PERMISSIONS=(
  "mcp__plugin_${PLUGIN_NAME}_${PLUGIN_NAME}__convert_to_markdown"
)

# Add the server-scoped permission to ~/.claude/settings.json (and drop the
# legacy tool-scoped one) so all of this plugin's tools run without a
# per-call approval prompt, present and future. Idempotent. If the file
# exists but isn't valid JSON, this makes no changes at all rather than
# clobbering it with `{}` — the caller is warned and installation continues
# (FR-018).
_grant_permissions() {
  local settings="${HOME}/.claude/settings.json"
  command -v node &>/dev/null || { echo "⚠ node missing — cannot auto-grant tool permissions"; return 0; }
  node -e '
    const fs = require("fs");
    const [path, addRule, ...dropRules] = process.argv.slice(1);
    let s = {};
    if (fs.existsSync(path)) {
      const raw = fs.readFileSync(path, "utf8");
      try { s = JSON.parse(raw); }
      catch (e) {
        console.error("⚠ " + path + " is not valid JSON (" + e.message + ") — skipping permission grant, leaving the file untouched");
        process.exit(0);
      }
    }
    s.permissions = s.permissions || {};
    s.permissions.allow = s.permissions.allow || [];
    const drop = new Set(dropRules);
    const before = s.permissions.allow.length;
    s.permissions.allow = s.permissions.allow.filter(t => !drop.has(t));
    const removed = before - s.permissions.allow.length;
    let added = 0;
    if (!s.permissions.allow.includes(addRule)) { s.permissions.allow.push(addRule); added = 1; }
    fs.writeFileSync(path, JSON.stringify(s, null, 2) + "\n");
    console.log("✓ tool permissions: granted " + addRule + " (" + added + " added)"
      + (removed > 0 ? "; removed " + removed + " legacy rule(s)" : ""));
  ' "$settings" "${TOOL_PERMISSIONS[0]}" "${LEGACY_TOOL_PERMISSIONS[@]}"
}

# Remove every permission rule this installer (current or legacy) may have
# added. Idempotent. Same non-destructive behavior on invalid JSON as above.
_revoke_permissions() {
  local settings="${HOME}/.claude/settings.json"
  command -v node &>/dev/null || return 0
  [[ -f "$settings" ]] || return 0
  node -e '
    const fs = require("fs");
    const [path, ...tools] = process.argv.slice(1);
    const drop = new Set(tools);
    let s;
    try { s = JSON.parse(fs.readFileSync(path, "utf8")); }
    catch (e) {
      console.error("⚠ " + path + " is not valid JSON (" + e.message + ") — skipping permission revoke, leaving the file untouched");
      process.exit(0);
    }
    if (s?.permissions?.allow) {
      const before = s.permissions.allow.length;
      s.permissions.allow = s.permissions.allow.filter(t => !drop.has(t));
      fs.writeFileSync(path, JSON.stringify(s, null, 2) + "\n");
      console.log("✓ tool permissions revoked (" + (before - s.permissions.allow.length) + " removed)");
    }
  ' "$settings" "${TOOL_PERMISSIONS[@]}" "${LEGACY_TOOL_PERMISSIONS[@]}"
}

# Remove any binary this installer left in the pre-005 default location
# (~/.local/bin), so a stale copy never shadows the current one on PATH.
# Skipped if that legacy path happens to BE the current install dir (e.g.
# MARKITDOWN_BIN_DIR=~/.local/bin) — never delete what we just installed.
_remove_legacy_binaries() {
  if [[ "${LEGACY_BIN_DIR}" == "${BIN_DIR}" ]]; then
    return 0
  fi
  local removed=0
  for b in "${BINS[@]}"; do
    if [[ -f "${LEGACY_BIN_DIR}/${b}" ]]; then
      rm -f "${LEGACY_BIN_DIR}/${b}"
      removed=$((removed + 1))
    fi
  done
  if [[ "${removed}" -gt 0 ]]; then
    echo "✓ removed ${removed} legacy binary(ies) from ${LEGACY_BIN_DIR}"
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
  _remove_legacy_binaries

  _revoke_permissions

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

# `claude plugin install` (step 4 below) writes into ~/.claude/settings.json
# and refuses to run at all if that file exists but isn't valid JSON — this
# is Claude Code's own behavior, not something this script can route around
# or safely repair on your behalf. Caught here, before any build/install
# work, so the failure is one clear message instead of a confusing one
# several steps in. This is a stronger requirement than the tool-permission
# grant below, which degrades to a warning instead of failing (see
# _grant_permissions) — that only covers *this script's own* write, not
# Claude Code's.
_settings_file="${HOME}/.claude/settings.json"
if [[ -f "${_settings_file}" ]] && command -v node &>/dev/null; then
  if ! node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "${_settings_file}" 2>/dev/null; then
    echo "ERROR: ${_settings_file} exists but is not valid JSON." >&2
    echo "       'claude plugin install' cannot run until it's fixed (or removed)." >&2
    exit 1
  fi
fi

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
echo "→ install dir: ${BIN_DIR} (via ${BIN_DIR_SOURCE})"
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
_remove_legacy_binaries
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

# 6. Grant tool permissions so all plugin tools run without per-call approval.
_grant_permissions

echo ""
echo "Installation complete. Restart Claude Code to activate the plugin."
echo "  • MCP tool: convert_to_markdown (http/https/file/data URIs → Markdown)"
echo "  • Optional: export GEMINI_API_KEY for Gemini-based PDF conversion"
