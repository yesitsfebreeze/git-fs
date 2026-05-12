# Runs on SessionStart. Creates a fresh gitfs branch for this conversation.
# Branch name written to .gitfs-session in project root so other hooks can find it.

$repo = $env:GITFS_REPO
if (-not $repo -or -not (Test-Path $repo)) { exit 0 }

$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$rand      = '{0:D4}' -f (Get-Random -Maximum 9999)
$branch    = "session/$timestamp-$rand"

$out = gitfs branch create $branch 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Output "gitfs-session: failed to create branch '$branch': $out"
    exit 0
}

$branchFile = Join-Path $PWD ".gitfs-session"
$branch | Set-Content -Path $branchFile -Encoding UTF8 -NoNewline

Write-Output "gitfs-session: branch '$branch' ready"
