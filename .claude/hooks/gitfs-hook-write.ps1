# PreToolUse: Write
# Intercepts Write tool calls, writes content to gitfs branch, blocks disk write.

$repo = $env:GITFS_REPO
if (-not $repo -or -not (Test-Path $repo)) { exit 0 }

$branchFile = Join-Path $PWD ".gitfs-session"
$branch = Get-Content $branchFile -ErrorAction SilentlyContinue
if (-not $branch) { exit 0 }

$rawInput = [Console]::In.ReadToEnd()
if (-not $rawInput.Trim()) { exit 0 }

try { $data = $rawInput | ConvertFrom-Json } catch { exit 0 }

$filePath = $data.tool_input.file_path
$content  = $data.tool_input.content
if (-not $filePath) { exit 0 }

# Translate absolute path to repo-relative
$cwd = $PWD.Path.TrimEnd('\').TrimEnd('/')
$rel = $filePath -replace [regex]::Escape($cwd + '\'), '' `
                 -replace [regex]::Escape($cwd + '/'), ''
$rel = $rel.Replace('\', '/')
if ($rel -eq $filePath) {
    # Path is not under cwd — skip gitfs, let Write proceed
    exit 0
}

$msg = "write $rel"
$out = $content | gitfs write $branch $rel --message $msg 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Output "gitfs-write: error writing '$rel': $out"
    exit 0  # fall through to real Write on failure
}

Write-Output "gitfs: wrote $branch`:$rel"
exit 2  # block disk Write
