[CmdletBinding()]
param(
    [switch]$SelfTest,
    [switch]$Smoke,
    [string]$PluginRoot,
    [string]$Revision,
    [string]$Sha256,
    [string]$Workspace,
    [string]$Session,
    [string]$Output
)

$ErrorActionPreference = 'Stop'

function Assert-PlainPath([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Refusing reparse path: $cursor"
            }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if (-not $parent -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
}

function Resolve-Pin([string]$Root, [string]$Rev, [string]$Hash, [string]$Dir, [string]$Id) {
    if ($Rev -cnotmatch '^[0-9a-f]{40}$') { throw 'Revision must be a lowercase 40-character Git SHA' }
    if ($Hash -notmatch '^[0-9a-fA-F]{64}$') { throw 'SHA256 must be exactly 64 hexadecimal characters' }
    if ($Id -cnotmatch '^[a-zA-Z0-9][a-zA-Z0-9._-]{7,95}$') {
        throw 'Session must be an explicit, distinct Git-ref-safe identifier (8-96 characters)'
    }
    & git check-ref-format "refs/gitfs/$Id" 2>$null
    if ($LASTEXITCODE -ne 0) { throw 'Session is not a valid Git reference' }
    if (-not [IO.Path]::IsPathRooted($Root) -or -not [IO.Path]::IsPathRooted($Dir)) {
        throw 'PluginRoot and Workspace must be absolute paths'
    }
    Assert-PlainPath $Root
    Assert-PlainPath $Dir
    $rootPath = (Resolve-Path -LiteralPath $Root -ErrorAction Stop).ProviderPath
    $workPath = (Resolve-Path -LiteralPath $Dir -ErrorAction Stop).ProviderPath
    if (-not (Test-Path -LiteralPath $workPath -PathType Container)) {
        throw 'Workspace must be an existing directory'
    }
    $manifest = Join-Path $rootPath '.claude-plugin\plugin.json'
    $binary = Join-Path $rootPath ("bin\versions\$Rev\git-fs.exe")
    foreach ($path in @($rootPath, $workPath, $manifest, $binary)) {
        Assert-PlainPath $path
    }
    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf) -or
        (Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json).name -ne 'git-fs') {
        throw 'PluginRoot must be a git-fs plugin'
    }
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
        throw "Exact staged executable not found: $binary"
    }
    $actual = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
    if ($actual -ne $Hash) {
        throw "Staged binary SHA256 mismatch: expected $Hash, got $actual"
    }
    return @{ binary = $binary; root = $rootPath; workspace = $workPath; hash = $actual }
}

function New-McpServer([hashtable]$Pin, [string]$Id) {
    return @{ type = 'stdio'; command = $Pin.binary; args = @('mcp'); env = @{
        GIT_FS_COORDINATED = '1'; GIT_FS_SESSION = $Id; GIT_FS_DISK = $Pin.workspace
    } }
}

function Write-Config([hashtable]$Pin, [string]$Rev, [string]$Id, [string]$Destination) {
    if (-not [IO.Path]::IsPathRooted($Destination)) { throw 'Output must be an absolute path' }
    $target = [IO.Path]::GetFullPath($Destination)
    $parent = [IO.Path]::GetDirectoryName($target)
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        throw 'Output parent must already exist'
    }
    Assert-PlainPath $parent
    $rootPrefix = $Pin.root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if ($target.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Output must be outside installed plugin'
    }
    if (Test-Path -LiteralPath $target) { throw "Refusing to overwrite existing config: $target" }
    $json = @{ mcpServers = @{ 'git-fs-pinned' = (New-McpServer $Pin $Id) } } |
        ConvertTo-Json -Depth 7
    $temp = Join-Path $parent ([IO.Path]::GetFileName($target) + '.stage.' + [guid]::NewGuid().ToString('N'))
    try {
        [IO.File]::WriteAllText($temp, $json + [Environment]::NewLine,
            (New-Object Text.UTF8Encoding($false)))
        [IO.File]::Move($temp, $target)
    } finally {
        if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Force }
    }
    return $target
}

function Smoke-Mcp([hashtable]$Pin, [string]$Rev, [string]$Id) {
    $si = New-Object Diagnostics.ProcessStartInfo
    $server = New-McpServer $Pin $Id
    $si.FileName = $server.command
    $si.Arguments = ($server.args -join ' ')
    $si.WorkingDirectory = $Pin.workspace
    $si.UseShellExecute = $false
    $si.RedirectStandardInput = $true
    $si.RedirectStandardOutput = $true
    $si.RedirectStandardError = $true
    foreach ($key in @('GIT_FS_COORDINATED', 'GIT_FS_SESSION', 'GIT_FS_DISK')) {
        $si.EnvironmentVariables[$key] = $server.env[$key]
    }
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $si
    try {
        if (-not $process.Start()) { throw 'Failed to launch pinned MCP process' }
        function Rpc([int]$Number, [string]$Method, $Params) {
            $message = @{ jsonrpc = '2.0'; id = $Number; method = $Method; params = $Params }
            $process.StandardInput.WriteLine(($message | ConvertTo-Json -Compress -Depth 12))
            $process.StandardInput.Flush()
            $read = $process.StandardOutput.ReadLineAsync()
            if (-not $read.Wait(10000)) { throw "MCP $Method timed out" }
            if (-not $read.Result) { throw "MCP $Method closed without response" }
            $reply = $read.Result | ConvertFrom-Json
            if ($reply.jsonrpc -ne '2.0' -or $reply.id -ne $Number -or
                $reply.error -or -not $reply.result) {
                throw "Invalid MCP response to $Method"
            }
            return $reply.result
        }
        $init = Rpc 1 'initialize' @{
            protocolVersion = '2025-06-18'; capabilities = @{};
            clientInfo = @{ name = 'gfs10-fresh-client-smoke'; version = '1' }
        }
        if ($init.protocolVersion -ne '2025-06-18' -or
            $init._meta.'gitfs/runtimeIdentity'.build.revision -cne $Rev) {
            throw 'MCP protocol or embedded revision mismatch'
        }
        $process.StandardInput.WriteLine('{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}')
        $process.StandardInput.Flush()
        $list = Rpc 2 'tools/list' @{}
        $snapshot = @($list.tools | Where-Object name -eq 'gitfs_snapshot')[0]
        $integrate = @($list.tools | Where-Object name -eq 'integrate')[0]
        if (-not $snapshot -or -not $integrate) { throw 'Missing git-fs MCP tools' }
        if (@($snapshot.inputSchema.properties.kind.enum) -notcontains 'candidate' -or
            -not $snapshot.inputSchema.properties.expected_tree -or
            -not $snapshot.inputSchema.properties.expected_review -or
            -not $snapshot.inputSchema.properties.capability) {
            throw 'Old candidate snapshot MCP schema'
        }
        if (@($integrate.inputSchema.properties.phase.enum) -notcontains 'retry_sync') {
            throw 'Old integrate MCP schema'
        }
        $ls = Rpc 3 'tools/call' @{ name = 'gitfs_ls'; arguments = @{} }
        if ($ls.isError -or -not $ls.content -or -not $ls.content[0].text) {
            throw 'Fresh MCP gitfs_ls failed'
        }
        $identity = $ls.content[0].text | ConvertFrom-Json
        if ($identity.runtime.session.id -cne $Id) {
            throw 'Fresh MCP listing did not bind the exact requested session'
        }
        Write-Output "PASS: fresh native MCP $Rev session $Id candidate schema and retry_sync"
    } finally {
        if (-not $process.HasExited) { $process.Kill() }
        $process.Dispose()
    }
}

function Test-Config {
    $fixture = Join-Path ([IO.Path]::GetTempPath()) ('gfs10-optin-' + [guid]::NewGuid().ToString('N'))
    $root = Join-Path $fixture 'plugin'
    $work = Join-Path $fixture 'workspace'
    $rev = '0123456789abcdef0123456789abcdef01234567'
    try {
        $version = Join-Path $root ("bin\versions\$rev")
        New-Item -ItemType Directory -Path $version, $work, (Join-Path $root '.claude-plugin') -Force | Out-Null
        [IO.File]::WriteAllText((Join-Path $root '.claude-plugin\plugin.json'), '{"name":"git-fs"}')
        $binary = Join-Path $version 'git-fs.exe'
        [IO.File]::WriteAllText($binary, 'exact fixture')
        $sha = (Get-FileHash $binary -Algorithm SHA256).Hash
        $pin = Resolve-Pin $root $rev $sha $work 'gfs10-unique-fixture'
        $out = Join-Path $fixture 'fresh.json'
        Write-Config $pin $rev 'gfs10-unique-fixture' $out | Out-Null
        $config = Get-Content -LiteralPath $out -Raw | ConvertFrom-Json
        $server = $config.mcpServers.'git-fs-pinned'
        if ($server.command -cne $binary -or @($server.args).Count -ne 1 -or
            $server.args[0] -cne 'mcp' -or $server.env.GIT_FS_SESSION -cne 'gfs10-unique-fixture' -or
            $server.env.GIT_FS_COORDINATED -cne '1' -or $server.env.GIT_FS_DISK -cne $work) {
            throw 'Generated MCP config does not pin executable, workspace and session'
        }
        $before = (Get-FileHash $out -Algorithm SHA256).Hash
        try { Write-Config $pin $rev 'gfs10-other-fixture' $out | Out-Null; throw 'Allowed overwrite' }
        catch { if ($_.Exception.Message -eq 'Allowed overwrite') { throw } }
        if ((Get-FileHash $out -Algorithm SHA256).Hash -ne $before) {
            throw 'Existing config changed'
        }
        try { Resolve-Pin $root $rev ('0' * 64) $work 'gfs10-unique-fixture'; throw 'Accepted wrong hash' }
        catch { if ($_.Exception.Message -eq 'Accepted wrong hash') { throw } }
        try { Resolve-Pin $root $rev $sha $work 'x'; throw 'Accepted short session' }
        catch { if ($_.Exception.Message -eq 'Accepted short session') { throw } }
        foreach ($invalid in @('gfs10..x', 'gfs10.lock')) {
            try { Resolve-Pin $root $rev $sha $work $invalid; throw 'Accepted invalid Git ref' }
            catch { if ($_.Exception.Message -eq 'Accepted invalid Git ref') { throw } }
        }
        $junction = Join-Path $fixture 'plugin-junction'
        try {
            New-Item -ItemType Junction -Path $junction -Target $root -ErrorAction Stop | Out-Null
            try { Resolve-Pin $junction $rev $sha $work 'gfs10-unique-fixture'; throw 'Accepted reparse root' }
            catch { if ($_.Exception.Message -eq 'Accepted reparse root') { throw } }
        } finally {
            if (Test-Path -LiteralPath $junction) { [IO.Directory]::Delete($junction) }
        }
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf) -or
            (Get-Content -LiteralPath $binary -Raw) -ne 'exact fixture') {
            throw 'Junction cleanup touched the target fixture'
        }
        Write-Output 'PASS: opt-in config pins exact binary/session, rejects overwrite, invalid refs and reparse roots'
    } finally {
        if (Test-Path -LiteralPath $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
    }
}

if ($SelfTest) {
    if ($Smoke) { throw 'Select one mode' }
    Test-Config
    exit 0
}
foreach ($required in @('PluginRoot', 'Revision', 'Sha256', 'Workspace', 'Session')) {
    if (-not (Get-Variable -Name $required -ValueOnly)) { throw "Missing -$required" }
}
$pin = Resolve-Pin $PluginRoot $Revision $Sha256 $Workspace $Session
if ($Smoke) {
    Smoke-Mcp $pin $Revision $Session
} else {
    if (-not $Output) { throw 'Missing -Output' }
    Write-Config $pin $Revision $Session $Output
}
