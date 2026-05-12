# PreToolUse: Read
# Materializes file from gitfs to disk before real Read proceeds.
# If file not in gitfs, exits 0 silently (real Read uses disk as normal).

$repo = $env:GITFS_REPO
if (-not $repo -or -not (Test-Path $repo)) { exit 0 }

$branchFile = Join-Path $PWD ".gitfs-session"
$branch = Get-Content $branchFile -ErrorAction SilentlyContinue
if (-not $branch) { exit 0 }

$rawInput = [Console]::In.ReadToEnd()
if (-not $rawInput.Trim()) { exit 0 }

try { $data = $rawInput | ConvertFrom-Json } catch { exit 0 }

$filePath = $data.tool_input.file_path
if (-not $filePath) { exit 0 }

$cwd = $PWD.Path.TrimEnd('\').TrimEnd('/')
$rel = $filePath -replace [regex]::Escape($cwd + '\'), '' `
                 -replace [regex]::Escape($cwd + '/'), ''
$rel = $rel.Replace('\', '/')
if ($rel -eq $filePath) { exit 0 }

# Try to read from gitfs
$content = gitfs read $branch $rel 2>$null
if ($LASTEXITCODE -ne 0) {
    # Not in gitfs — real Read proceeds from disk unchanged
    exit 0
}

# Materialize: write gitfs content to disk so real Read sees it
$dir = Split-Path $filePath -Parent
if ($dir -and -not (Test-Path $dir)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}
# Use .NET to avoid BOM and encoding issues
[System.IO.File]::WriteAllText($filePath, $content, [System.Text.UTF8Encoding]::new($false))

# Exit 0: let real Read proceed from the now-synced file
exit 0
