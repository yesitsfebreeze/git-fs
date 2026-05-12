# PreToolUse: Edit
# Reads current file from gitfs, applies old->new replacement, writes back. Blocks disk edit.

$repo = $env:GITFS_REPO
if (-not $repo -or -not (Test-Path $repo)) { exit 0 }

$branchFile = Join-Path $PWD ".gitfs-session"
$branch = Get-Content $branchFile -ErrorAction SilentlyContinue
if (-not $branch) { exit 0 }

$rawInput = [Console]::In.ReadToEnd()
if (-not $rawInput.Trim()) { exit 0 }

try { $data = $rawInput | ConvertFrom-Json } catch { exit 0 }

$filePath   = $data.tool_input.file_path
$oldString  = $data.tool_input.old_string
$newString  = $data.tool_input.new_string
$replaceAll = $data.tool_input.replace_all
if (-not $filePath -or $null -eq $oldString) { exit 0 }

$cwd = $PWD.Path.TrimEnd('\').TrimEnd('/')
$rel = $filePath -replace [regex]::Escape($cwd + '\'), '' `
                 -replace [regex]::Escape($cwd + '/'), ''
$rel = $rel.Replace('\', '/')
if ($rel -eq $filePath) { exit 0 }

# Read current content from gitfs (fall back to disk if not in gitfs yet)
$current = gitfs read $branch $rel 2>$null
if ($LASTEXITCODE -ne 0) {
    # File not in gitfs: read from disk and seed into gitfs
    if (-not (Test-Path $filePath)) { exit 0 }
    $current = Get-Content $filePath -Raw -Encoding UTF8
    if (-not $current) { $current = "" }
    # Seed the file into gitfs so future reads come from there
    $current | gitfs write $branch $rel --message "seed $rel" 2>$null | Out-Null
}

# Apply replacement
if ($replaceAll) {
    $updated = $current.Replace($oldString, $newString)
} else {
    $idx = $current.IndexOf($oldString)
    if ($idx -lt 0) {
        Write-Output "gitfs-edit: old_string not found in '$rel'"
        exit 0  # let real Edit handle the error
    }
    $updated = $current.Substring(0, $idx) + $newString + $current.Substring($idx + $oldString.Length)
}

$msg = "edit $rel"
$out = $updated | gitfs write $branch $rel --message $msg 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Output "gitfs-edit: error writing '$rel': $out"
    exit 0
}

Write-Output "gitfs: edited $branch`:$rel"
exit 2  # block disk Edit
