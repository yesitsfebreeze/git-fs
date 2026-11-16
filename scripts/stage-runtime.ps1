# Stage a pinned native Git-FS executable without changing live plugin entrypoints.
[CmdletBinding()]
param(
    [switch]$SelfTest,
    [string]$Repository = '',
    [string]$PluginRoot = $env:GIT_FS_PLUGIN_ROOT,
    [string]$ExpectedRevision = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-NoReparse([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $entry = Get-Item -LiteralPath $cursor -Force
            if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "Refusing reparse-point path: $cursor"
            }
        }
        $parent = [IO.Directory]::GetParent($cursor)
        if ($null -eq $parent) { break }
        $cursor = $parent.FullName
    }
}

function File-Hash([string]$Path) {
    Assert-NoReparse $Path
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Expected regular file: $Path"
    }
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Stage-Binary([string]$Source, [string]$Root, [string]$Revision) {
    if ($Revision -cnotmatch '^[0-9a-f]{40}$') { throw 'Expected full lowercase Git revision' }
    $sourceHash = File-Hash $Source
    Assert-NoReparse $Root
    $directory = Join-Path $Root "bin\versions\$Revision"
    Assert-NoReparse $directory
    [void](New-Item -ItemType Directory -Path $directory -Force)
    Assert-NoReparse $directory
    $destination = Join-Path $directory 'git-fs.exe'
    if (Test-Path -LiteralPath $destination) {
        if ((File-Hash $destination) -cne $sourceHash) {
            throw "Version collision: refusing to replace $destination"
        }
        return $destination
    }

    $temporary = Join-Path $directory ("git-fs.exe.stage." + [guid]::NewGuid().ToString('N'))
    try {
        Copy-Item -LiteralPath $Source -Destination $temporary -ErrorAction Stop
        if ((File-Hash $temporary) -cne $sourceHash) { throw 'Staged copy hash mismatch' }
        try {
            # Same-directory rename is atomic and never overwrites an existing version.
            [IO.File]::Move($temporary, $destination)
        } catch [IO.IOException] {
            if (-not (Test-Path -LiteralPath $destination)) { throw }
        }
        if ((File-Hash $destination) -cne $sourceHash) {
            throw "Version collision: refusing to replace $destination"
        }
        return $destination
    } finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
    }
}

function Read-Reply([Diagnostics.Process]$Process) {
    $read = $Process.StandardOutput.ReadLineAsync()
    if (-not $read.Wait(10000) -or [string]::IsNullOrWhiteSpace($read.Result)) {
        throw 'Timed out waiting for staged MCP JSON-RPC reply'
    }
    $read.Result | ConvertFrom-Json
}

function Smoke-Mcp([string]$Binary, [string]$Revision) {
    $smokeDir = Join-Path ([IO.Path]::GetTempPath()) ("gitfs-mcp-smoke-" + [guid]::NewGuid().ToString('N'))
    [void](New-Item -ItemType Directory -Path $smokeDir)
    $process = $null
    try {
        $start = New-Object Diagnostics.ProcessStartInfo
        $start.FileName = $Binary
        $start.Arguments = 'mcp'
        $start.WorkingDirectory = $smokeDir
        $start.UseShellExecute = $false
        $start.RedirectStandardInput = $true
        $start.RedirectStandardOutput = $true
        $start.RedirectStandardError = $true
        [void]$start.EnvironmentVariables.Remove('GIT_FS_CONFIG')
        [void]$start.EnvironmentVariables.Remove('GIT_FS_SESSION')
        [void]$start.EnvironmentVariables.Remove('GIT_FS_COORDINATED')
        $process = [Diagnostics.Process]::Start($start)
        $process.StandardInput.WriteLine('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"stage-smoke","version":"1"}}}')
        $process.StandardInput.Flush()
        $hello = Read-Reply $process
        if ($null -ne $hello.PSObject.Properties['error'] -or
            $hello.jsonrpc -cne '2.0' -or $hello.id -ne 1 -or
            $null -eq $hello.PSObject.Properties['result'] -or
            $hello.result.protocolVersion -cne '2025-06-18') {
            throw 'Staged MCP initialize negotiation failed'
        }
        $identity = $hello.result._meta.'gitfs/runtimeIdentity'
        if ($identity.build.revision -cne $Revision -or
            $identity.connection_binding_state -cne 'pending' -or
            [string]::IsNullOrEmpty($identity.tool_schema_fingerprint)) {
            throw 'Staged MCP runtime identity does not match built revision/profile'
        }
        $process.StandardInput.WriteLine('{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}')
        $process.StandardInput.Flush()
        $tools = Read-Reply $process
        if ($null -ne $tools.PSObject.Properties['error'] -or
            $tools.jsonrpc -cne '2.0' -or $tools.id -ne 2 -or
            $null -eq $tools.PSObject.Properties['result']) {
            throw 'Staged MCP tools/list reply identity failed'
        }
        $integrate = @($tools.result.tools | Where-Object { $_.name -ceq 'integrate' })
        if ($integrate.Count -ne 1 -or
            -not (@($integrate[0].inputSchema.properties.phase.enum) -ccontains 'retry_sync')) {
            throw 'Staged MCP integrate schema lacks retry_sync'
        }
        return $identity.tool_schema_fingerprint
    } finally {
        if ($null -ne $process) {
            $process.StandardInput.Close()
            if (-not $process.WaitForExit(4000)) { $process.Kill() }
            $process.Dispose()
        }
        if (Test-Path -LiteralPath $smokeDir) {
            Remove-Item -LiteralPath $smokeDir -Recurse -Force
        }
    }
}

if ($SelfTest) {
    $testRoot = Join-Path ([IO.Path]::GetTempPath()) ("gitfs-stage-selftest-" + [guid]::NewGuid().ToString('N'))
    [void](New-Item -ItemType Directory -Path $testRoot)
    try {
        $source = Join-Path $testRoot 'source.exe'
        $stable = Join-Path $testRoot 'bin\git-fs.exe'
        [void](New-Item -ItemType Directory -Path (Split-Path $stable) -Force)
        [IO.File]::WriteAllBytes($source, [byte[]](1, 2, 3))
        [IO.File]::WriteAllBytes($stable, [byte[]](8, 9))
        $stableHash = File-Hash $stable
        $revision = 'a' * 40
        $staged = Stage-Binary $source $testRoot $revision
        $firstHash = File-Hash $staged
        if ((Stage-Binary $source $testRoot $revision) -cne $staged) {
            throw 'Identical replay changed the destination'
        }
        [IO.File]::WriteAllBytes($source, [byte[]](4, 5))
        $rejected = $false
        try { [void](Stage-Binary $source $testRoot $revision) } catch { $rejected = $true }
        if (-not $rejected -or (File-Hash $staged) -cne $firstHash -or
            (File-Hash $stable) -cne $stableHash) {
            throw 'Collision or stable executable preservation check failed'
        }
        Write-Output 'PASS: immutable staged copy, repeat, collision rejection and stable executable preservation'
    } finally {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
    exit 0
}

if (-not $Repository) { $Repository = Join-Path $PSScriptRoot '..' }
if ([string]::IsNullOrWhiteSpace($PluginRoot)) {
    throw 'Supply -PluginRoot or set GIT_FS_PLUGIN_ROOT; no implicit installation target'
}
$repoPath = (Resolve-Path -LiteralPath $Repository).Path
$pluginPath = (Resolve-Path -LiteralPath $PluginRoot).Path
Assert-NoReparse $pluginPath
$manifest = Join-Path $pluginPath '.claude-plugin\plugin.json'
[void](File-Hash $manifest)
if ((Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json).name -cne 'git-fs') {
    throw 'Destination is not a git-fs plugin directory'
}
$head = (& git -C $repoPath rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $head -cnotmatch '^[0-9a-f]{40}$') {
    throw 'Expected a Git checkout with a full HEAD'
}
if ($ExpectedRevision -cnotmatch '^[0-9a-f]{40}$') {
    throw 'ExpectedRevision must be a full lowercase 40-character Git SHA'
}
if ($head -cne $ExpectedRevision) {
    throw "Repository revision changed: expected $ExpectedRevision, got $head"
}
$changedBuildInputs = @(& git -C $repoPath status --porcelain=v1 --untracked-files=all -- src Cargo.toml Cargo.lock build.rs)
if ($LASTEXITCODE -ne 0 -or $changedBuildInputs.Count -gt 0) {
    throw 'Build inputs are modified; commit or isolate them before versioned staging'
}
$protected = @('bin\git-fs.exe', 'bin\git-fs.cmd', '.claude-plugin\plugin.json', 'hooks\hooks.json')
$before = @{}
foreach ($relative in $protected) {
    $item = Join-Path $pluginPath $relative
    $before[$relative] = if (Test-Path -LiteralPath $item) { File-Hash $item } else { $null }
}
$previousRevision = $env:GIT_FS_BUILD_REVISION
try {
    $env:GIT_FS_BUILD_REVISION = $head
    Push-Location $repoPath
    try {
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Cargo release build failed' }
    } finally { Pop-Location }
} finally { $env:GIT_FS_BUILD_REVISION = $previousRevision }
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repoPath 'target' }
$built = Join-Path $targetDir 'release\git-fs.exe'
$builtHash = File-Hash $built
$headAfterBuild = (& git -C $repoPath rev-parse HEAD).Trim()
$changedAfterBuild = @(& git -C $repoPath status --porcelain=v1 --untracked-files=all -- src Cargo.toml Cargo.lock build.rs)
if ($LASTEXITCODE -ne 0 -or $headAfterBuild -cne $head -or $changedAfterBuild.Count -gt 0) {
    throw 'Source revision or Rust build inputs changed during release build'
}
$staged = Stage-Binary $built $pluginPath $head
if ((File-Hash $staged) -cne $builtHash) { throw 'Built and staged binary hashes disagree' }
$fingerprint = Smoke-Mcp $staged $head
foreach ($relative in $protected) {
    $item = Join-Path $pluginPath $relative
    $after = if (Test-Path -LiteralPath $item) { File-Hash $item } else { $null }
    if ($before[$relative] -cne $after) {
        throw "Protected plugin entrypoint changed during staging: $relative"
    }
}
Write-Output "STAGED $staged"
Write-Output "REVISION $head"
Write-Output "SHA256 $builtHash"
Write-Output "MCP_SCHEMA $fingerprint"
