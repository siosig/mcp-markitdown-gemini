#!/usr/bin/env bash
set -euo pipefail

# Install / uninstall markitdown-mcp (Rust) as a Claude Code plugin.
#
# Usage model: clone the repository yourself, `cd` into it, and run this script.
# It installs FROM the current checkout — it does not clone anything.
#
# Distribution model:
#   1. Download a pre-built binary from GitHub Releases (siosig/mcp-markitdown-gemini)
#      that matches this OS/arch.
#   2. If none is published, or none matches this platform, fall back to building
#      it from source (cargo build --release --locked -p markitdown-mcp).
#   3. If neither works, print an actionable error and exit 1.
#
# The binary is placed at plugins/markitdown-gemini/bin/markitdown-mcp.exe, INSIDE
# this checkout — never onto a shared PATH directory. The plugin's .mcp.json
# launches it via ${CLAUDE_PLUGIN_ROOT}/bin/markitdown-mcp.exe, which Claude Code
# resolves the same way on every OS, so no PATH setup or plugin-cache rewrite is
# needed. The .exe suffix is kept on every platform (including Linux/macOS): Claude
# Code's plugin loader only launches this file by that exact name, and .mcp.json
# admits one command string with no way to branch per OS. The suffix has no effect
# on POSIX, where the kernel reads the file's header, not its name.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd 2>/dev/null || pwd)"
MARKETPLACE_NAME="markitdown-gemini"
PLUGIN_NAME="markitdown-gemini"

REPO="siosig/mcp-markitdown-gemini"
BINARY="markitdown-mcp"
BIN_FILE="markitdown-mcp.exe"
BIN_DIR="${SCRIPT_DIR}/plugins/markitdown-gemini/bin"

usage() {
  cat >&2 << EOF
Usage: ${0##*/} [-s|--from-source] [-d|--uninstall] [-h|--help]

  Clone the repo yourself, cd into it, then run this script.

  (no flag)              Install the markitdown-gemini plugin from this checkout:
                           • acquire markitdown-mcp for this OS/arch: download a
                             GitHub Releases asset first, and if none is published
                             or none matches this platform, build it from source
                             (cargo build --release --locked -p markitdown-mcp)
                           • place it at plugins/markitdown-gemini/bin/${BIN_FILE}
                           • verify the binary runs (--version) before registering
                             anything
                           • remove any binary left by an older installer version
                           • register the '${MARKETPLACE_NAME}' marketplace + install the plugin
                           • grant this plugin's MCP server all-tools permission
  -s, --from-source     Skip the GitHub Releases download; build from source directly
                           (cargo build --release --locked -p markitdown-mcp).
  -d, --uninstall        Remove what the installer added (this repo folder is left untouched):
                           • uninstall the '${PLUGIN_NAME}' plugin
                           • remove the '${MARKETPLACE_NAME}' marketplace entry
                           • remove plugins/markitdown-gemini/bin/
                           • revoke the granted permission
                           • remove any binary left by an older installer version
  -h, --help             Show this help.

Optional: export GEMINI_API_KEY before launching Claude Code to enable
Gemini-based PDF conversion (the MCP server inherits Claude Code's environment).
Without it, PDF conversion falls back to local extraction.

Troubleshooting (source build only): if building fails with
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
FROM_SOURCE=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    -s|--from-source) FROM_SOURCE=true ;;
    -d|--uninstall|--delete) MODE="uninstall" ;;
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

# Directory an installer version before 007-windows-installer (the 005-era
# cargo-install-style placement) may have put the binary in. Resolved with the
# same precedence 005 used to pick its *install* directory, but here only to
# find what to clean up — it is never where THIS installer places the binary
# (that is always BIN_DIR, inside the plugin checkout).
_legacy_bin_dir() {
  if [[ -n "${MARKITDOWN_BIN_DIR:-}" ]]; then
    printf '%s' "${MARKITDOWN_BIN_DIR}"
  elif [[ -n "${CARGO_INSTALL_ROOT:-}" ]]; then
    printf '%s' "${CARGO_INSTALL_ROOT}/bin"
  elif [[ -n "${CARGO_HOME:-}" ]]; then
    printf '%s' "${CARGO_HOME}/bin"
  else
    printf '%s' "${HOME}/.cargo/bin"
  fi
}

# Remove binaries left by installer versions before 007-windows-installer: the
# 005-era cargo-install-style location (_legacy_bin_dir) and the pre-005
# default (~/.local/bin). Never touches BIN_DIR, this installer's own
# placement inside the plugin.
_remove_legacy_binaries() {
  local legacy_dir
  legacy_dir="$(_legacy_bin_dir)"
  if [[ -f "${legacy_dir}/${BINARY}" ]]; then
    rm -f "${legacy_dir}/${BINARY}"
    echo "✓ removed legacy binary: ${legacy_dir}/${BINARY}"
  fi
  if [[ -f "${HOME}/.local/bin/${BINARY}" ]]; then
    rm -f "${HOME}/.local/bin/${BINARY}"
    echo "✓ removed legacy binary: ${HOME}/.local/bin/${BINARY}"
  fi
  return 0
}

# Print the release asset name for this platform (contracts/release-assets.md's
# platform table) on stdout, or print a reason to stderr and return 1 if this
# OS/arch has no published asset.
_resolve_asset() {
  local os arch
  case "$(uname -s)" in
    Linux*)  os="linux" ;;
    Darwin*) os="darwin" ;;
    *) echo "  (unsupported OS: $(uname -s))" >&2; return 1 ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64)  arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *) echo "  (unsupported architecture: $(uname -m))" >&2; return 1 ;;
  esac
  case "${os}-${arch}" in
    linux-x86_64|linux-aarch64|darwin-aarch64)
      printf '%s' "${BINARY}-${os}-${arch}.tar.gz"
      ;;
    *)
      echo "  (no release asset published for ${os}-${arch})" >&2
      return 1
      ;;
  esac
}

# Resolve a GitHub token for Authorization headers. The repository is PUBLIC, so
# this is optional (it only raises the API rate limit) — no `gh` dependency.
_github_token() {
  if [[ -n "${GH_TOKEN:-}" ]];     then printf '%s' "${GH_TOKEN}";     return 0; fi
  if [[ -n "${GITHUB_TOKEN:-}" ]]; then printf '%s' "${GITHUB_TOKEN}"; return 0; fi
  return 1
}

# Download the release asset for this platform and place it at BIN_DIR/BIN_FILE.
# Sets ACQUIRED_FROM on success. Returns 1 (with a reason on stderr) on any
# failure, so the caller can fall back to build_from_source.
download_release() {
  local asset
  asset="$(_resolve_asset)" || return 1

  local tmp
  tmp="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '${tmp}'" RETURN

  local -a auth=()
  local tok
  if tok="$(_github_token)"; then auth=(-H "Authorization: Bearer ${tok}"); fi

  echo "→ checking for a published release (${REPO})"
  local http_code
  http_code="$(curl -sS "${auth[@]}" \
    -H "Accept: application/vnd.github+json" \
    -o "${tmp}/release.json" -w '%{http_code}' \
    "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null)" || http_code="000"

  if [[ "${http_code}" != "200" ]]; then
    if [[ "${http_code}" == "404" ]]; then
      echo "  (no published release for ${REPO})" >&2
    else
      echo "  (could not reach api.github.com — HTTP ${http_code})" >&2
    fi
    return 1
  fi

  local tag
  tag="$(grep -m1 '"tag_name"' "${tmp}/release.json" | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
  if [[ -z "${tag}" ]]; then
    echo "  (unexpected response shape from api.github.com)" >&2
    return 1
  fi

  echo "→ downloading ${asset} (${tag})"
  if ! curl -fsSL --retry 3 "${auth[@]}" \
    -o "${tmp}/${asset}" \
    "https://github.com/${REPO}/releases/download/${tag}/${asset}"; then
    echo "  (release ${tag} has no asset ${asset})" >&2
    return 1
  fi

  tar xzf "${tmp}/${asset}" -C "${tmp}" || return 1
  if [[ ! -f "${tmp}/${BINARY}" ]]; then
    echo "  (archive ${asset} did not contain ${BINARY})" >&2
    return 1
  fi

  mkdir -p "${BIN_DIR}"
  install -m 0755 "${tmp}/${BINARY}" "${BIN_DIR}/${BIN_FILE}"
  ACQUIRED_FROM="release ${tag}"
}

# Build markitdown-mcp from this checkout and place it at BIN_DIR/BIN_FILE.
# Sets ACQUIRED_FROM on success. Returns 1 on any failure.
build_from_source() {
  [[ -f "${SCRIPT_DIR}/Cargo.toml" ]] || return 1
  # rustup installs cargo to ~/.cargo/bin but does not always export it onto PATH
  # (e.g. non-login shells that never source ~/.cargo/env). Locate it ourselves.
  if ! command -v cargo &>/dev/null; then
    if [[ -r "${CARGO_HOME:-${HOME}/.cargo}/env" ]]; then
      # shellcheck disable=SC1091
      . "${CARGO_HOME:-${HOME}/.cargo}/env"
    fi
    case ":${PATH}:" in
      *":${HOME}/.cargo/bin:"*) ;;
      *) PATH="${HOME}/.cargo/bin:${PATH}" ;;
    esac
  fi
  command -v cargo &>/dev/null || return 1
  echo "✓ cargo: $(command -v cargo)"
  echo "→ building ${BINARY} from source (cargo build --release --locked -p markitdown-mcp)"
  ( cd "${SCRIPT_DIR}" && cargo build --release --locked -p markitdown-mcp ) || return 1
  mkdir -p "${BIN_DIR}"
  install -m 0755 "${SCRIPT_DIR}/target/release/${BINARY}" "${BIN_DIR}/${BIN_FILE}"
  ACQUIRED_FROM="source build"
}

# Acquire the binary: release download first (unless -s/--from-source), then a
# source build fallback. Exits 1 with actionable next steps if both fail.
_acquire() {
  echo "→ acquiring the ${BINARY} binary"
  local ok=false
  if [[ "${FROM_SOURCE}" == "true" ]]; then
    build_from_source && ok=true
  else
    if download_release; then
      ok=true
    elif build_from_source; then
      ok=true
    fi
  fi
  if [[ "${ok}" != "true" ]]; then
    echo "ERROR: could not obtain ${BINARY}." >&2
    echo "  Check your network connection and https://github.com/${REPO}/releases." >&2
    echo "  Or install Rust and re-run: https://rustup.rs" >&2
    exit 1
  fi
  echo "✓ acquired ${BINARY} (${ACQUIRED_FROM}) → ${BIN_DIR}/${BIN_FILE}"
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

  if [[ -d "${BIN_DIR}" ]]; then
    rm -rf "${BIN_DIR}"
    echo "✓ binary dir ${BIN_DIR}: removed"
  else
    echo "✓ binary dir ${BIN_DIR}: not present (skip)"
  fi

  _revoke_permissions
  _remove_legacy_binaries

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

# `claude plugin install` (step below) writes into ~/.claude/settings.json
# and refuses to run at all if that file exists but isn't valid JSON — this
# is Claude Code's own behavior, not something this script can route around
# or safely repair on your behalf. Caught here, before any acquisition work,
# so the failure is one clear message instead of a confusing one several
# steps in. This is a stronger requirement than the tool-permission grant
# below, which degrades to a warning instead of failing (see
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
if [[ ! -d "${SCRIPT_DIR}/crates" || ! -f "${SCRIPT_DIR}/.claude-plugin/marketplace.json" || ! -f "${SCRIPT_DIR}/plugins/markitdown-gemini/.mcp.json" ]]; then
  echo "ERROR: this does not look like a markitdown-gemini checkout: ${SCRIPT_DIR}" >&2
  echo "       Clone the repo, cd into it, and run ./${0##*/} from there." >&2
  exit 1
fi
echo "✓ source: ${SCRIPT_DIR}"

if [[ -z "${GEMINI_API_KEY:-}" ]]; then
  echo ""
  echo "NOTE: GEMINI_API_KEY is not set. PDF conversion will use local extraction"
  echo "      instead of Gemini. To enable it:"
  echo "        export GEMINI_API_KEY=<your-key>"
  echo "      then restart Claude Code."
  echo ""
fi

# 1. Acquire the binary (release download, or source build).
_acquire

# 2. Verify the binary runs before registering anything with Claude Code.
echo "→ verifying the binary runs"
out="$("${BIN_DIR}/${BIN_FILE}" --version 2>&1)" || {
  echo "ERROR: the installed binary did not run: ${BIN_DIR}/${BIN_FILE}" >&2
  echo "  output: ${out}" >&2
  exit 1
}
if [[ "${out}" != *"${BINARY}"* ]]; then
  echo "ERROR: the installed binary did not run: ${BIN_DIR}/${BIN_FILE}" >&2
  echo "  output: ${out}" >&2
  exit 1
fi
echo "✓ binary runs: ${out}"

# 3. Clean up anything left by an older installer version.
_remove_legacy_binaries

# 4. Register the local marketplace (this checkout). Re-register if it points elsewhere.
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

# 5. (Re)install the plugin.
if claude plugin list 2>/dev/null | grep -q "❯ ${PLUGIN_NAME}@"; then
  echo "→ reinstalling existing '${PLUGIN_NAME}' plugin"
  claude plugin uninstall "${PLUGIN_NAME}" --yes
fi
claude plugin install "${PLUGIN_NAME}@${MARKETPLACE_NAME}"
echo "✓ '${PLUGIN_NAME}@${MARKETPLACE_NAME}': installed"

# 6. Grant tool permissions so all plugin tools run without per-call approval.
_grant_permissions

echo ""
echo "Installation complete. Restart Claude Code to activate the plugin."
echo "  binary      : ${BIN_DIR}/${BIN_FILE} (${ACQUIRED_FROM})"
echo "  marketplace : ${SCRIPT_DIR}"
echo "  permission  : ${TOOL_PERMISSIONS[0]}"
echo ""
echo "Verify with:    claude mcp list (expect: markitdown-gemini … ✔ Connected)"
echo "Uninstall with: ${0##*/} -d"
