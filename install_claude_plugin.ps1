#Requires -Version 7.4
<#
.SYNOPSIS
    Install (or uninstall with -Uninstall) the markitdown-gemini Claude Code plugin on Windows.

.DESCRIPTION
    Windows counterpart to install_claude_plugin.sh. The distribution model is the same:

      - The plugin (skills / agents / MCP server config) is registered as a Claude Code
        marketplace. By default the marketplace SOURCE is this local repo checkout, so the
        currently-checked-out plugin is installed as-is.
      - The MCP server is a native Rust binary, placed at
        plugins\markitdown-gemini\bin\markitdown-mcp.exe, which the plugin's
        .mcp.json launches via ${CLAUDE_PLUGIN_ROOT}/bin/markitdown-mcp.exe.

    Binary acquisition (install mode): downloaded from GitHub Releases. There is NO
    build-from-source fallback — Windows machines typically have neither a Rust compiler
    nor a linker, so a missing release asset is a hard, actionable failure.

    The .exe suffix is kept on every platform. .mcp.json admits one command string with no
    way to branch per OS, and Claude Code cannot launch an extension-less image on Windows
    (verified on Windows 11: the plugin registered, but the MCP server never started). The
    suffix is meaningless on POSIX, where the kernel reads the file header, not the name.

.PARAMETER Uninstall
    Remove everything this installer created: the plugin, the marketplace entry, the
    installed binary directory, and the tool permission rule.

.PARAMETER Help
    Show usage and exit.

.EXAMPLE
    .\install_claude_plugin.ps1
    Install or re-install the plugin.

.EXAMPLE
    .\install_claude_plugin.ps1 -Uninstall
    Remove everything the installer created.

.NOTES
    Requires PowerShell 7.4+ and the Claude Code CLI. If script execution is blocked, run
    with:
      pwsh -ExecutionPolicy Bypass -File .\install_claude_plugin.ps1
#>
[CmdletBinding()]
param(
    [Alias('d')]
    [switch] $Uninstall,

    [Alias('h')]
    [switch] $Help
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ── Constants ────────────────────────────────────────────────────────────────
$ScriptDir            = $PSScriptRoot
$Repo                 = 'siosig/mcp-markitdown-gemini'
$Binary               = 'markitdown-mcp'
$MarketplaceName      = 'markitdown-gemini'
$PluginName           = 'markitdown-gemini'
$PluginRef            = "${PluginName}@${MarketplaceName}"
$BinDir               = Join-Path $ScriptDir 'plugins' 'markitdown-gemini' 'bin'
# The placed file carries a .exe suffix on every platform: Claude Code cannot launch
# an extension-less image (verified on Windows 11 -- the plugin registered but the MCP
# server failed to start), and .mcp.json admits one command string with no per-OS branch.
$BinFile              = "$Binary.exe"
$BinPath              = Join-Path $BinDir $BinFile
$SettingsFile         = Join-Path $HOME '.claude' 'settings.json'
$PermissionRule       = 'mcp__plugin_markitdown-gemini_markitdown-gemini'
$LegacyPermissionRules = @('mcp__plugin_markitdown-gemini_markitdown-gemini__convert_to_markdown')

# ── Output helpers ───────────────────────────────────────────────────────────
# Same vocabulary as the POSIX installer, so the two transcripts read alike.
function Write-Step { param([string] $Message) Write-Host "→ $Message" }
function Write-Ok   { param([string] $Message) Write-Host "✓ $Message" }
function Write-Warn { param([string] $Message) Write-Host "⚠ $Message" }
function Write-Note { param([string] $Message) Write-Host "NOTE: $Message" }

function Stop-WithError {
    param([string] $Message, [string[]] $Hints = @())
    [Console]::Error.WriteLine("ERROR: $Message")
    foreach ($hint in $Hints) { [Console]::Error.WriteLine("  $hint") }
    exit 1
}

function Show-Usage {
    @"
Usage: install_claude_plugin.ps1 [-Uninstall] [-Help]

  (no flag)    Install / re-install the '$PluginName' plugin (default).
  -Uninstall   Remove everything this installer created:
                 - uninstall the '$PluginName' plugin
                 - remove the '$MarketplaceName' marketplace entry
                 - delete the installed binary at $BinDir
                 - remove the '$PermissionRule' tool permission rule
               Left untouched: this checkout, GEMINI_API_KEY, and any other
               settings or permission rules you added yourself.
  -Help        Show this help.

Requires PowerShell 7.4+ and the Claude Code CLI. Unlike the POSIX installer, this
script never builds from source: it installs a prebuilt binary from GitHub Releases.
If script execution is blocked, run with:
  pwsh -ExecutionPolicy Bypass -File .\install_claude_plugin.ps1
"@
}

function Get-RequiredCommand {
    param([string] $Name, [string] $Hint)
    $cmd = Get-Command $Name -CommandType Application -ErrorAction SilentlyContinue |
           Select-Object -First 1
    if (-not $cmd) { Stop-WithError "'$Name' not found." @($Hint) }
    Write-Ok "${Name}: $($cmd.Source)"
    return $cmd.Source
}

# Run the claude CLI, tolerating a non-zero exit (used for idempotent teardown steps).
function Invoke-Claude {
    param([string[]] $ClaudeArgs, [switch] $IgnoreFailure)
    $output = $null
    $code   = 0
    try {
        $global:LASTEXITCODE = 0
        $output = & claude @ClaudeArgs 2>&1
        $code = $LASTEXITCODE
    } catch {
        $output = $_.Exception.Message
        $code = -1
    }
    if ($code -ne 0 -and -not $IgnoreFailure) {
        Stop-WithError "claude $($ClaudeArgs -join ' ') failed (exit $code)." @($output -join "`n")
    }
    return $output
}

function Test-PluginInstalled {
    $global:LASTEXITCODE = 0
    $listed = & claude plugin list 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $listed) { return $false }
    return [bool]($listed | Select-String -SimpleMatch "❯ ${PluginName}@" -Quiet)
}

function Test-MarketplaceRegistered {
    $global:LASTEXITCODE = 0
    $listed = & claude plugin marketplace list 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $listed) { return $false }
    return [bool]($listed | Select-String -SimpleMatch $MarketplaceName -Quiet)
}

# ── Prerequisite checks ───────────────────────────────────────────────────────
function Test-SettingsJson {
    if (-not (Test-Path -LiteralPath $SettingsFile -PathType Leaf)) { return }
    try {
        Get-Content -LiteralPath $SettingsFile -Raw | ConvertFrom-Json -AsHashtable | Out-Null
    } catch {
        Stop-WithError "$SettingsFile exists but is not valid JSON." @(
            "'claude plugin install' cannot run until it is fixed (or removed)."
        )
    }
}

function Test-Checkout {
    $marketplaceManifest = Join-Path $ScriptDir '.claude-plugin' 'marketplace.json'
    $mcpManifest          = Join-Path $ScriptDir 'plugins' 'markitdown-gemini' '.mcp.json'
    if (-not (Test-Path -LiteralPath $marketplaceManifest -PathType Leaf) -or
        -not (Test-Path -LiteralPath $mcpManifest -PathType Leaf)) {
        Stop-WithError "this does not look like a $Repo checkout (missing $marketplaceManifest or $mcpManifest)." @(
            "Clone the repository and run this script from inside it: https://github.com/$Repo"
        )
    }
}

function Show-GeminiNotice {
    if (-not $env:GEMINI_API_KEY) {
        Write-Host ''
        Write-Note 'GEMINI_API_KEY is not set, so high-fidelity PDF conversion via Gemini is'
        Write-Host '      disabled and local conversion is used instead. To enable it:'
        Write-Host '  [Environment]::SetEnvironmentVariable("GEMINI_API_KEY", "<your-key>", "User")'
        Write-Host '  Then restart Claude Code.'
        Write-Host ''
    }
}

# ── Install: acquire the binary, then register the plugin ────────────────────
function Resolve-AssetName {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        'X64' { return "$Binary-windows-x86_64.zip" }
        default {
            Stop-WithError "unsupported Windows architecture: $arch." @(
                'Only x86_64 (X64) has a published release asset.'
            )
        }
    }
}

function Get-ApiHeaders {
    $headers = @{ 'User-Agent' = 'markitdown-gemini-installer' }
    $tok = if ($env:GH_TOKEN) { $env:GH_TOKEN } elseif ($env:GITHUB_TOKEN) { $env:GITHUB_TOKEN } else { $null }
    if ($tok) { $headers['Authorization'] = "Bearer $tok" }
    return $headers
}

function Get-LatestTag {
    $headers = Get-ApiHeaders
    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" `
                                     -Headers $headers -TimeoutSec 30
    } catch {
        $status = $null
        if ($_.Exception.PSObject.Properties.Name -contains 'Response' -and $_.Exception.Response) {
            $status = [int] $_.Exception.Response.StatusCode
        }
        if ($status -eq 404) {
            Stop-WithError "no published release found for $Repo." @(
                'The Windows installer never builds from source, so there is nothing to install.',
                "https://github.com/$Repo/releases"
            )
        }
        Stop-WithError "could not query the latest release of $Repo." @(
            "$($_.Exception.Message)",
            'Check network access to api.github.com and try again.'
        )
    }
    if (-not $release.tag_name) {
        Stop-WithError "the latest release of $Repo has no tag name." @(
            'The GitHub API returned an unexpected shape; report this to the maintainer.'
        )
    }
    return $release.tag_name
}

function Install-Binary {
    # Returns the tag the binary was acquired from.
    $asset = Resolve-AssetName
    $tag   = Get-LatestTag
    $url   = "https://github.com/$Repo/releases/download/$tag/$asset"

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("markitdown-gemini-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp -Force | Out-Null
    try {
        $zip = Join-Path $tmp $asset
        Write-Step "downloading $asset ($tag)"
        $headers = Get-ApiHeaders
        try {
            Invoke-WebRequest -Uri $url -OutFile $zip -TimeoutSec 300 -Headers $headers
        } catch {
            $status = $null
            if ($_.Exception.PSObject.Properties.Name -contains 'Response' -and $_.Exception.Response) {
                $status = [int] $_.Exception.Response.StatusCode
            }
            if ($status -eq 404) {
                Stop-WithError "release $tag has no asset $asset." @(
                    "Expected: $url",
                    'The release asset layout may have changed; report this to the maintainer.'
                )
            }
            Stop-WithError "failed to download $asset from release $tag." @(
                "$($_.Exception.Message)",
                "Expected asset: $url"
            )
        }

        try {
            Expand-Archive -LiteralPath $zip -DestinationPath $tmp -Force
        } catch {
            Stop-WithError "failed to extract $asset." @("$($_.Exception.Message)")
        }

        # The archive holds exactly one executable at its root, already named .exe --
        # which is exactly the name it keeps once placed.
        $extracted = Join-Path $tmp $BinFile
        if (-not (Test-Path -LiteralPath $extracted -PathType Leaf)) {
            Stop-WithError "the archive did not contain $BinFile." @(
                "Archive: $asset",
                'The release asset layout may have changed; report this to the maintainer.'
            )
        }

        # Files downloaded from the internet carry a zone identifier that can make
        # SmartScreen or antivirus refuse to run them. This is best-effort: the
        # cmdlet is Windows-only and -ErrorAction cannot suppress the platform
        # exception it raises elsewhere, so catch it rather than abort the install.
        try { Unblock-File -LiteralPath $extracted } catch { }

        # Move into place only after a successful extract, so a failed download can
        # never leave a broken binary at the path the plugin launches.
        New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
        try {
            Move-Item -LiteralPath $extracted -Destination $BinPath -Force
        } catch {
            # The usual cause is a running Claude Code still holding the old binary
            # open — Windows refuses to replace a file that is in use.
            Stop-WithError "could not write $BinPath." @(
                "$($_.Exception.Message)",
                'If Claude Code is running, quit it and re-run this installer:',
                'Windows will not replace an executable while it is in use.'
            )
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    Write-Ok "installed from GitHub Releases → $BinPath"
    return $tag
}

function Invoke-BinaryProbe {
    # Launch $Path through CreateProcess (never a shell, never ShellExecute) and
    # return what happened. This is how Claude Code itself starts the MCP server,
    # so it is the launch that actually has to work.
    param([string] $Path)

    $out = [System.IO.Path]::GetTempFileName()
    $err = [System.IO.Path]::GetTempFileName()
    try {
        $proc = Start-Process -FilePath $Path -ArgumentList '--version' `
                              -NoNewWindow -Wait -PassThru `
                              -RedirectStandardOutput $out -RedirectStandardError $err
        $text = ((Get-Content -LiteralPath $out -Raw -ErrorAction SilentlyContinue) +
                 (Get-Content -LiteralPath $err -Raw -ErrorAction SilentlyContinue))
        return @{ Ok = $true; Code = $proc.ExitCode; Text = "$text".Trim(); Error = $null }
    } catch {
        # A blocked or unlaunchable image throws here, and the Win32 code is the
        # single most useful fact for telling the causes apart.
        $native = $null
        if ($_.Exception -is [System.ComponentModel.Win32Exception]) {
            $native = $_.Exception.NativeErrorCode
        } elseif ($_.Exception.InnerException -is [System.ComponentModel.Win32Exception]) {
            $native = $_.Exception.InnerException.NativeErrorCode
        }
        return @{ Ok = $false; Code = -1; Text = ''
                  Error = "$($_.Exception.Message)$(if ($null -ne $native) { " (Win32 error $native)" })" }
    } finally {
        Remove-Item -LiteralPath $out, $err -Force -ErrorAction SilentlyContinue
    }
}

function Test-BinaryRuns {
    # Prove the placed file actually starts, rather than discovering later that the
    # plugin registered fine but its server never launches.
    Write-Step 'verifying the installed binary starts'

    # Exit code alone is not proof: PowerShell's call operator can return 0 with no
    # output for a file it never launched. Probe through CreateProcess and require the
    # binary to identify itself.
    $result = Invoke-BinaryProbe -Path $BinPath

    if ($result.Ok -and $result.Code -eq 0 -and $result.Text -match [regex]::Escape($Binary)) {
        Write-Ok "binary runs: $($result.Text)"
        return
    }

    $detail = if ($result.Error) { $result.Error }
              elseif ($result.Text) { $result.Text }
              else { "exit $($result.Code), no output" }
    $signature = try { (Get-AuthenticodeSignature -LiteralPath $BinPath).Status } catch { 'unknown' }

    Stop-WithError "the installed binary did not run: $BinPath" @(
        "launch    : $detail",
        "signature : $signature",
        'The download may be corrupt, or the machine may be refusing to run an unsigned executable.',
        'Check Smart App Control:  Get-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy | Select VerifiedAndReputablePolicyState',
        '  (1 = Enforce, and an unsigned binary stays blocked until it is signed or explicitly allowed.)'
    )
}

# ── Permission rule (settings.json) ───────────────────────────────────────────
# Reads/writes settings.json without depending on Node, using the round-trip
# method verified on this machine's pwsh 7.6.6 (research R7): ConvertFrom-Json
# -AsHashtable preserves key order and data types (null/bool/decimals/Japanese
# text/escapes/arrays) when paired with ConvertTo-Json -Depth 100 and
# Set-Content -Encoding utf8NoBOM.
function Grant-Permission {
    $s = if (Test-Path -LiteralPath $SettingsFile -PathType Leaf) {
        Get-Content -LiteralPath $SettingsFile -Raw | ConvertFrom-Json -AsHashtable
    } else {
        [ordered]@{}
    }
    if (-not $s.Contains('permissions')) { $s['permissions'] = [ordered]@{} }
    if (-not $s['permissions'].Contains('allow')) { $s['permissions']['allow'] = @() }

    $existing = @($s['permissions']['allow'])
    $kept     = @($existing | Where-Object { $_ -ne $PermissionRule -and $_ -notin $LegacyPermissionRules })
    $added    = if ($existing -contains $PermissionRule) { 0 } else { 1 }
    $removedLegacy = @($existing | Where-Object { $_ -in $LegacyPermissionRules }).Count
    $allow    = @($kept) + @($PermissionRule)
    $s['permissions']['allow'] = $allow

    $parent = Split-Path -Parent $SettingsFile
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    ($s | ConvertTo-Json -Depth 100) | Set-Content -LiteralPath $SettingsFile -Encoding utf8NoBOM

    Write-Ok "tool permissions: granted $PermissionRule ($added added); removed $removedLegacy legacy rule(s)"
}

function Revoke-Permission {
    if (-not (Test-Path -LiteralPath $SettingsFile -PathType Leaf)) {
        Write-Ok "settings ${SettingsFile}: not present (skip)"
        return
    }
    $s = $null
    try {
        $s = Get-Content -LiteralPath $SettingsFile -Raw | ConvertFrom-Json -AsHashtable
    } catch {
        Write-Warn "$SettingsFile is not valid JSON — skipping permission revoke, leaving the file untouched"
        return
    }

    if (-not $s.Contains('permissions') -or -not $s['permissions'].Contains('allow')) {
        Write-Ok "tool permissions: not present (skip)"
        return
    }

    $existing = @($s['permissions']['allow'])
    $kept     = @($existing | Where-Object { $_ -ne $PermissionRule -and $_ -notin $LegacyPermissionRules })
    $removed  = $existing.Count - $kept.Count
    $s['permissions']['allow'] = $kept

    ($s | ConvertTo-Json -Depth 100) | Set-Content -LiteralPath $SettingsFile -Encoding utf8NoBOM
    Write-Ok "tool permissions revoked ($removed removed)"
}

function Invoke-Install {
    # 1. Prerequisites.
    Get-RequiredCommand -Name 'claude' -Hint 'Install Claude Code: https://claude.ai/code' | Out-Null
    Test-SettingsJson
    Test-Checkout
    Show-GeminiNotice

    # 2. Acquire and prove the binary — before registration, because plugin
    #    installation copies the plugin directory.
    Write-Step "acquiring the $Binary binary"
    $tag = Install-Binary
    Test-BinaryRuns

    # 3. Register the marketplace (idempotent).
    if (Test-MarketplaceRegistered) {
        Write-Step "refreshing marketplace '$MarketplaceName'"
        Invoke-Claude @('plugin', 'marketplace', 'remove', $MarketplaceName) -IgnoreFailure | Out-Null
    }
    Write-Step "adding marketplace: $ScriptDir"
    Invoke-Claude @('plugin', 'marketplace', 'add', $ScriptDir) | Out-Null
    Write-Ok "marketplace '$MarketplaceName': registered"

    # 4. Remove any existing install, then install.
    if (Test-PluginInstalled) {
        Write-Step "uninstalling existing '$PluginName'"
        Invoke-Claude @('plugin', 'uninstall', $PluginName, '--yes') -IgnoreFailure | Out-Null
    }
    Write-Step "installing $PluginRef"
    Invoke-Claude @('plugin', 'install', $PluginRef) | Out-Null
    Write-Ok "plugin '$PluginName': installed"

    # 5. Grant the tool permission rule.
    Grant-Permission

    @"

------------------------------------------------------------------
Installation complete. Restart Claude Code to activate the plugin.

  binary      : $BinPath (release $tag)
  marketplace : $ScriptDir
  permission  : $PermissionRule

Verify with:      claude mcp list   (expect: markitdown-gemini … ✔ Connected)
Uninstall with:   .\install_claude_plugin.ps1 -Uninstall
------------------------------------------------------------------
"@ | Write-Host
}

# ── Uninstall: reverse everything the installer created ──────────────────────
function Invoke-Uninstall {
    Write-Step "uninstalling $PluginName (-Uninstall)"
    Get-RequiredCommand -Name 'claude' -Hint 'Install Claude Code: https://claude.ai/code' | Out-Null

    # 1. Uninstall the plugin.
    try {
        if (Test-PluginInstalled) {
            Invoke-Claude @('plugin', 'uninstall', $PluginName, '--yes') -IgnoreFailure | Out-Null
            Write-Ok "plugin '$PluginName': uninstalled"
        } else {
            Write-Ok "plugin '$PluginName': not installed (skip)"
        }
    } catch {
        Write-Warn "could not uninstall plugin '$PluginName': $($_.Exception.Message)"
    }

    # 2. Remove the marketplace entry.
    try {
        if (Test-MarketplaceRegistered) {
            Invoke-Claude @('plugin', 'marketplace', 'remove', $MarketplaceName) -IgnoreFailure | Out-Null
            Write-Ok "marketplace '$MarketplaceName': removed"
        } else {
            Write-Ok "marketplace '$MarketplaceName': not registered (skip)"
        }
    } catch {
        Write-Warn "could not remove marketplace '$MarketplaceName': $($_.Exception.Message)"
    }

    # 3. Remove the installed binary — only the bin directory the installer
    #    created, never anything else in the checkout.
    try {
        if (Test-Path -LiteralPath $BinDir) {
            Remove-Item -LiteralPath $BinDir -Recurse -Force
            Write-Ok "binary ${BinDir}: removed"
        } else {
            Write-Ok "binary ${BinDir}: not present (skip)"
        }
    } catch {
        Write-Warn "could not remove ${BinDir}: $($_.Exception.Message)"
    }

    # 4. Revoke the tool permission rule.
    try {
        Revoke-Permission
    } catch {
        Write-Warn "could not revoke tool permissions: $($_.Exception.Message)"
    }

    Write-Host ''
    Write-Host 'Uninstall complete. Restart Claude Code to drop the plugin from the session.'
}

# ── Entry point ──────────────────────────────────────────────────────────────
if ($Help) { Show-Usage; exit 0 }

# Guard the engine version before touching anything. #Requires already blocks
# older engines, but its error is terse — this one tells the user what to do.
if ($PSVersionTable.PSVersion -lt [version]'7.4') {
    [Console]::Error.WriteLine("ERROR: PowerShell 7.4 or later is required (found $($PSVersionTable.PSVersion)).")
    [Console]::Error.WriteLine('  Install it with:  winget install --id Microsoft.PowerShell')
    [Console]::Error.WriteLine('  Then re-run this script from a "pwsh" prompt.')
    exit 1
}

if ($Uninstall) { Invoke-Uninstall; exit 0 }
Invoke-Install
