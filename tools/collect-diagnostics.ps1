<#
.SYNOPSIS
  收集 G-OpenSteamTool / OpenSteamTool 排障所需的最小日志包（脱敏）。

.DESCRIPTION
  「无互联网连接」= Steam 拿不到 manifest request code（见 docs/NO_INTERNET.md）。
  定位需要三类证据：DLL 侧（<Steam>\opensteamtool\*.log）、Steam 侧（<Steam>\logs\*.txt）、
  现场文件（config\lua\*.lua 与 opensteamtool.toml）。

  本脚本把这些文件按「一份目录 + 一个 zip」打包，并对 Depot Key、token、ticket、
  SteamID 做脱敏替换，保留行结构与计数，方便直接发给别人看。

.EXAMPLE
  .\collect-diagnostics.ps1
  .\collect-diagnostics.ps1 -SteamDir 'D:\Steam' -OutDir 'D:\diag'

.NOTES
  只读：不修改 Steam 目录里的任何文件。
#>
#Requires -Version 5.1
[CmdletBinding()]
param(
  [string]$SteamDir,
  [string]$OutDir = [Environment]::GetFolderPath('Desktop'),
  [int]$MaxLogKB = 2048,
  [switch]$NoZip
)

$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'

function Resolve-SteamDir {
  param([string]$Given)
  $candidates = New-Object System.Collections.Generic.List[string]
  if ($Given) { $candidates.Add($Given) }
  foreach ($key in @(
      'HKCU:\Software\Valve\Steam',
      'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam',
      'HKLM:\SOFTWARE\Valve\Steam')) {
    try {
      $item = Get-ItemProperty -Path $key -ErrorAction Stop
      foreach ($name in @('SteamPath', 'InstallPath')) {
        if ($item.PSObject.Properties.Name -contains $name -and $item.$name) {
          $candidates.Add([string]$item.$name)
        }
      }
    } catch { }
  }
  foreach ($drive in @('C', 'D', 'E', 'F', 'G')) {
    foreach ($sub in @('\Steam', '\Program Files (x86)\Steam', '\Program Files\Steam', '\Games\Steam')) {
      $candidates.Add("${drive}:$sub")
    }
  }
  foreach ($candidate in $candidates) {
    if (-not $candidate) { continue }
    $trimmed = $candidate.TrimEnd('\')
    if (Test-Path (Join-Path $trimmed 'steam.exe')) { return $trimmed }
  }
  throw "没找到 Steam 目录：请用 -SteamDir 指定包含 steam.exe 的目录（例如 -SteamDir 'D:\Steam'）。"
}

function Get-TailText {
  param([string]$Path, [int]$MaxKB)
  $limit = [Math]::Max(64, $MaxKB) * 1024
  $length = (Get-Item -LiteralPath $Path).Length
  if ($length -le $limit) { return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 }
  $bytes = New-Object byte[] $limit
  $stream = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
  try {
    [void]$stream.Seek(-$limit, 'End')
    [void]$stream.Read($bytes, 0, $limit)
  } finally { $stream.Dispose() }
  $text = [Text.Encoding]::UTF8.GetString($bytes)
  $cut = $text.IndexOf("`n")
  if ($cut -ge 0 -and $cut -lt $text.Length - 1) { $text = $text.Substring($cut + 1) }
  return "[... 已截断，只保留最后 $MaxKB KB ...]`n" + $text
}

function Protect-Text {
  param([string]$Text, [string]$Kind)
  if ($null -eq $Text) { return '' }
  if ($Kind -eq 'vdf') {
    $Text = [regex]::Replace($Text, '(?i)("DecryptionKey"\s*")[^"]+(")', '$1<REDACTED-KEY>$2')
    $Text = [regex]::Replace($Text, '(?i)("SteamID"\s*")[^"]+(")', '$1<REDACTED-ID>$2')
    $Text = [regex]::Replace($Text, '(?i)("AccountName"\s*")[^"]+(")', '$1<REDACTED-NAME>$2')
    return $Text
  }
  if ($Kind -eq 'lua') {
    $Text = [regex]::Replace($Text, '(addappid\(\s*\d+\s*,\s*\d+\s*,\s*")[^"]+(")', '$1<REDACTED-KEY>$2')
    $Text = [regex]::Replace($Text, '(addappid\(\s*\d+\s*,\s*")[^"]+(")', '$1<REDACTED-KEY>$2')
    $Text = [regex]::Replace($Text, '(addtoken\(\s*\d+\s*,\s*")[^"]+(")', '$1<REDACTED-TOKEN>$2')
    $Text = [regex]::Replace($Text, '(setAppTicket\(\s*\d+\s*,\s*")[^"]+(")', '$1<REDACTED-TICKET>$2')
    $Text = [regex]::Replace($Text, '(setETicket\(\s*\d+\s*,\s*")[^"]+(")', '$1<REDACTED-TICKET>$2')
    $Text = [regex]::Replace($Text, '(setStat\(\s*\d+\s*,\s*")[^"]+(")', '$1<REDACTED-ID>$2')
    $Text = [regex]::Replace($Text, '"[0-9A-Fa-f]{40,}"', '"<REDACTED-BLOB>"')
    return $Text
  }
  return $Text
}

function Save-Text {
  param([string]$Path, [string]$Text)
  $dir = Split-Path -Parent $Path
  if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
  [IO.File]::WriteAllText($Path, $Text, (New-Object Text.UTF8Encoding($false)))
}

function Add-Report {
  param([System.Collections.Generic.List[string]]$Lines, [string]$Line)
  [void]$Lines.Add($Line)
}

# ── 定位 Steam ────────────────────────────────────────────────────────────────
$SteamDir = Resolve-SteamDir -Given $SteamDir
$stage = Join-Path $OutDir ("gost-diag-" + $env:COMPUTERNAME + "-" + $stamp)
New-Item -ItemType Directory -Path $stage -Force | Out-Null
$report = New-Object System.Collections.Generic.List[string]

Add-Report $report "G-OpenSteamTool 诊断包"
Add-Report $report ("生成时间 : " + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz'))
Add-Report $report ("计算机   : $env:COMPUTERNAME / $env:USERNAME")
Add-Report $report ("Steam 目录: $SteamDir")
Add-Report $report ("PowerShell: " + $PSVersionTable.PSVersion.ToString())
try {
  $os = Get-CimInstance Win32_OperatingSystem
  Add-Report $report ("系统     : " + $os.Caption + " build " + $os.BuildNumber + " / " + $os.OSArchitecture)
} catch { Add-Report $report "系统     : 读取失败" }
Add-Report $report ""

# ── Steam 版本与 DLL 指纹 ─────────────────────────────────────────────────────
Add-Report $report "== Steam 构建 =="
$exe = Join-Path $SteamDir 'steam.exe'
if (Test-Path $exe) {
  $vi = (Get-Item $exe).VersionInfo
  Add-Report $report ("steam.exe : " + $vi.FileVersion + " / " + $vi.ProductVersion)
  $sig = Join-Path $SteamDir 'version.txt'
  if (Test-Path $sig) {
    $ver = (Get-Content $sig -Raw) -replace '\s+', ' '
    Add-Report $report ("version.txt: " + $ver)
  }
}
foreach ($dll in @('steamclient64.dll', 'steamui.dll')) {
  $path = Join-Path $SteamDir $dll
  if (Test-Path $path) {
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLower()
    $size = (Get-Item $path).Length
    Add-Report $report ("{0}: sha256={1} size={2}" -f $dll, $hash, $size)
  } else {
    Add-Report $report ("$dll : 缺失")
  }
}
foreach ($dll in @('OpenSteamTool.dll', 'dwmapi.dll', 'xinput1_4.dll')) {
  $path = Join-Path $SteamDir $dll
  if (Test-Path $path) {
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLower()
    $size = (Get-Item $path).Length
    Add-Report $report ("{0}: sha256={1} size={2}" -f $dll, $hash, $size)
    if ($dll -eq 'OpenSteamTool.dll') {
      $bytes = [IO.File]::ReadAllBytes($path)
      $text = [Text.Encoding]::ASCII.GetString($bytes)
      $idx = $text.IndexOf('OSTLAYOUT:')
      if ($idx -ge 0) {
        $tail = $text.Substring($idx + 10)
        $end = $tail.IndexOf([char]0)
        if ($end -lt 0) { $end = [Math]::Min(32, $tail.Length) }
        Add-Report $report ("  layout 标记: " + $tail.Substring(0, $end).Trim())
      }
    }
  } else {
    Add-Report $report ("$dll : 缺失")
  }
}
Add-Report $report ""

# ── 通道文件（按本机构建 sha256 命名）──────────────────────────────────────────
Add-Report $report "== 数据通道 =="
foreach ($channel in @('pattern', 'ipc')) {
  foreach ($component in @('steamclient', 'steamui')) {
    $dir = Join-Path $SteamDir (Join-Path 'opensteamtool' (Join-Path $channel $component))
    if (Test-Path $dir) {
      $files = Get-ChildItem -LiteralPath $dir -File -ErrorAction SilentlyContinue
      if ($files) {
        foreach ($file in $files) { Add-Report $report ("{0}\{1}\{2}: {3} B" -f $channel, $component, $file.Name, $file.Length) }
      } else {
        Add-Report $report ("{0}\{1}: 目录为空" -f $channel, $component)
      }
    } else {
      Add-Report $report ("{0}\{1}: 缺失" -f $channel, $component)
    }
  }
}
Add-Report $report ""

# ── 现场文件：toml / lua ──────────────────────────────────────────────────────
$tomlPath = Join-Path $SteamDir 'opensteamtool.toml'
Add-Report $report "== opensteamtool.toml =="
if (Test-Path $tomlPath) {
  $toml = Get-Content -LiteralPath $tomlPath -Raw
  Save-Text -Path (Join-Path $stage 'steam\opensteamtool.toml') -Text $toml
  foreach ($line in ($toml -split "`r?`n")) {
    if ($line -match '^\s*(level|url|manage_manifest_lua|manifest_lua_endpoint|paths)\s*=') {
      Add-Report $report ("  " + $line.Trim())
    }
  }
} else {
  Add-Report $report "  缺失（DLL 会按默认值跑，默认 log level = info）"
}
Add-Report $report ""

$luaDir = Join-Path $SteamDir 'config\lua'
Add-Report $report "== config\lua =="
if (Test-Path $luaDir) {
  $luaFiles = Get-ChildItem -LiteralPath $luaDir -File -Filter '*.lua*' -ErrorAction SilentlyContinue | Sort-Object Name
  if (-not $luaFiles) { Add-Report $report "  目录存在但没有 .lua" }
  foreach ($file in $luaFiles) {
    $raw = Get-Content -LiteralPath $file.FullName -Raw
    $safe = Protect-Text -Text $raw -Kind 'lua'
    Save-Text -Path (Join-Path $stage ('steam\config-lua\' + $file.Name)) -Text $safe
    $addApp = ([regex]::Matches($raw, '(?m)^\s*addappid\(')).Count
    $setMan = ([regex]::Matches($raw, '(?m)^\s*setManifestid\(')).Count
    $sentinel = if ($raw -match 'GOST-MANIFEST-LUA') { 'yes' } else { 'no' }
    $prefixes = @([regex]::Matches($raw, 'prefix\s*=\s*"([^"]*)"') | ForEach-Object { $_.Groups[1].Value })
    Add-Report $report ("  {0}: {1} B, addappid×{2}, setManifestid×{3}, 托管标记={4}" -f $file.Name, $file.Length, $addApp, $setMan, $sentinel)
    if ($prefixes.Count -gt 0) { Add-Report $report ("    取码源: " + ($prefixes -join ' | ')) }
  }
} else {
  Add-Report $report "  缺失：config\lua 不存在（manifest.lua 就没写进去）"
}
Add-Report $report ""

# ── config.vdf：只统计，不落 Key ──────────────────────────────────────────────
$vdfPath = Join-Path $SteamDir 'config\config.vdf'
Add-Report $report "== config\config.vdf =="
if (Test-Path $vdfPath) {
  $vdf = Get-Content -LiteralPath $vdfPath -Raw
  $keys = ([regex]::Matches($vdf, '"DecryptionKey"')).Count
  $cell = [regex]::Match($vdf, '"CurrentCellID"\s*"(\d+)"')
  Add-Report $report ("  DecryptionKey 条目: $keys")
  if ($cell.Success) { Add-Report $report ("  CurrentCellID: " + $cell.Groups[1].Value) }
  Save-Text -Path (Join-Path $stage 'steam\config.vdf.redacted') -Text (Protect-Text -Text $vdf -Kind 'vdf')
} else {
  Add-Report $report "  缺失"
}
Add-Report $report ""

# ── DLL 日志：最重要的证据 ───────────────────────────────────────────────────
Add-Report $report "== opensteamtool 日志 =="
$ostDir = Join-Path $SteamDir 'opensteamtool'
$ostLog = Join-Path $ostDir 'opensteamtool.log'
if (Test-Path $ostDir) {
  $logs = Get-ChildItem -LiteralPath $ostDir -File -Filter '*.log' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending
  if (-not $logs) { Add-Report $report "  opensteamtool 目录存在但没有 .log（说明 DLL 没加载/没写过日志）" }
  foreach ($log in $logs) {
    $tail = Get-TailText -Path $log.FullName -MaxKB $MaxLogKB
    Save-Text -Path (Join-Path $stage ('steam\opensteamtool\' + $log.Name)) -Text $tail
    $errs = ([regex]::Matches($tail, '\[error\]')).Count
    $warns = ([regex]::Matches($tail, '\[warn\]')).Count
    Add-Report $report ("  {0}: {1} B, 修改 {2}, error×{3}, warn×{4}" -f $log.Name, $log.Length, $log.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'), $errs, $warns)
  }
  if (Test-Path $ostLog) {
    $tail = Get-TailText -Path $ostLog -MaxKB $MaxLogKB
    Add-Report $report "  -- 关键行 --"
    $seen = @{}
    foreach ($pattern in @('pattern|signature|Missing|Unsupported', 'manifest|setManifestid|addappid', 'lua|Lua|fetch_manifest_code', 'exception|suppressed')) {
      $hits = ($tail -split "`r?`n") | Where-Object { $_ -match $pattern } | Select-Object -Last 6
      foreach ($hit in $hits) {
        $line = $hit.Trim()
        if (-not $seen.ContainsKey($line)) {
          $seen[$line] = 1
          Add-Report $report ("    " + $line)
        }
      }
    }
    $lastLines = ($tail -split "`r?`n") | Where-Object { $_.Trim() } | Select-Object -Last 15
    Add-Report $report "  -- 末尾 15 行 --"
    foreach ($line in $lastLines) { Add-Report $report ("    " + $line.Trim()) }
  }
} else {
  Add-Report $report "  opensteamtool 目录缺失：DLL 从未成功加载"
}
Add-Report $report ""

# ── Steam 自己的日志 ────────────────────────────────────────────────────────
Add-Report $report "== Steam 日志 =="
$steamLogs = Join-Path $SteamDir 'logs'
if (Test-Path $steamLogs) {
  foreach ($name in @('content_log.txt', 'console_log.txt', 'connection_log.txt', 'bootstrap_log.txt', 'cloud_log.txt', 'stderr.txt')) {
    $path = Join-Path $steamLogs $name
    if (-not (Test-Path $path)) { continue }
    $tail = Get-TailText -Path $path -MaxKB $MaxLogKB
    Save-Text -Path (Join-Path $stage ('steam\logs\' + $name)) -Text $tail
    Add-Report $report ("  {0}: {1} B" -f $name, (Get-Item $path).Length)
  }

  $content = Join-Path $steamLogs 'content_log.txt'
  if (Test-Path $content) {
    $lines = (Get-TailText -Path $content -MaxKB $MaxLogKB) -split "`r?`n"
    Add-Report $report "  -- manifest request 行（最后 12 条）--"
    $hits = $lines | Where-Object { $_ -match 'manifest' }
    foreach ($hit in ($hits | Select-Object -Last 12)) { Add-Report $report ("    " + $hit.Trim()) }
    $code200 = ($lines | Where-Object { $_ -match 'manifest request.*200' }).Count
    $code4xx = ($lines | Where-Object { $_ -match 'manifest request.*(40[0-9]|41[0-9])' }).Count
    Add-Report $report ("  统计: 200×{0}, 4xx×{1}" -f $code200, $code4xx)
    Add-Report $report "  -- 最近 CDN 主机（最后 6 条）--"
    $hosts = $lines | Where-Object { $_ -match 'host: ' } | Select-Object -Last 6
    foreach ($hit in $hosts) { Add-Report $report ("    " + $hit.Trim()) }
  }

  $console = Join-Path $steamLogs 'console_log.txt'
  if (Test-Path $console) {
    $version = (Get-TailText -Path $console -MaxKB 512) -split "`r?`n" | Where-Object { $_ -match 'Client version:' } | Select-Object -Last 1
    if ($version) { Add-Report $report ("  Client version: " + ($version -replace '.*Client version:\s*', '').Trim()) }
  }

  $conn = Join-Path $steamLogs 'connection_log.txt'
  if (Test-Path $conn) {
    $last = (Get-TailText -Path $conn -MaxKB 256) -split "`r?`n" | Where-Object { $_.Trim() } | Select-Object -Last 6
    Add-Report $report "  -- connection_log 末尾 --"
    foreach ($line in $last) { Add-Report $report ("    " + $line.Trim()) }
  }
} else {
  Add-Report $report "  logs 目录缺失"
}
Add-Report $report ""

# ── 崩溃转储 ────────────────────────────────────────────────────────────────
Add-Report $report "== dumps =="
$dumpsDir = Join-Path $SteamDir 'dumps'
if (Test-Path $dumpsDir) {
  $dumps = Get-ChildItem -LiteralPath $dumpsDir -File -Filter '*.dmp' | Sort-Object LastWriteTime -Descending | Select-Object -First 3
  if (-not $dumps) { Add-Report $report "  无" }
  $dumpTarget = Join-Path $stage 'steam\dumps'
  if ($dumps -and -not (Test-Path $dumpTarget)) { New-Item -ItemType Directory -Path $dumpTarget -Force | Out-Null }
  foreach ($dump in $dumps) {
    Copy-Item -LiteralPath $dump.FullName -Destination (Join-Path $dumpTarget $dump.Name) -Force
    Add-Report $report ("  {0}: {1} B, {2}" -f $dump.Name, $dump.Length, $dump.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))
  }
} else {
  Add-Report $report "  无 dumps 目录"
}
Add-Report $report ""

# ── 主机侧：代理 / hosts / DNS ───────────────────────────────────────────────
Add-Report $report "== 网络环境 =="
$proxyLines = @()
try { $proxyLines += (netsh winhttp show proxy 2>&1 | Out-String) } catch { }
try {
  $ie = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction Stop
  $proxyLines += "ProxyEnable=$($ie.ProxyEnable) ProxyServer=$($ie.ProxyServer) AutoConfigURL=$($ie.AutoConfigURL)"
} catch { }
Save-Text -Path (Join-Path $stage 'host\proxy.txt') -Text ($proxyLines -join "`r`n")
Add-Report $report ("  WinHTTP/IE 代理: " + (($proxyLines -join ' ') -replace '\s+', ' '))

$hostsPath = Join-Path $env:SystemRoot 'System32\drivers\etc\hosts'
if (Test-Path $hostsPath) {
  $hostsText = Get-Content -LiteralPath $hostsPath -Raw
  Save-Text -Path (Join-Path $stage 'host\hosts.txt') -Text $hostsText
  $custom = ($hostsText -split "`r?`n") | Where-Object { $_.Trim() -and $_.Trim() -notmatch '^#' }
  Add-Report $report ("  hosts 有效行: " + ($custom.Count) + " 条（已随包）")
}

try {
  $dns = Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction Stop |
    Where-Object { $_.ServerAddresses } |
    ForEach-Object { "$($_.InterfaceAlias): $($_.ServerAddresses -join ', ')" }
  Save-Text -Path (Join-Path $stage 'host\dns.txt') -Text ($dns -join "`r`n")
  Add-Report $report ("  DNS: " + (($dns -join ' ; ')))
} catch { }

# 取码源连通性（只做 TCP 443/80 与 HTTP HEAD，不改任何状态）
Add-Report $report ""
Add-Report $report "== 取码源连通性 =="
foreach ($target in @(
    @{ Name = 'manifestdex'; Url = 'https://manifest.manifestdex.com/' },
    @{ Name = 'opensteamtool'; Url = 'https://manifest.opensteamtool.com/' },
    @{ Name = 'steamrun'; Url = 'https://manifest.steam.run/api/manifest/' }
  )) {
  try {
    $null = Invoke-WebRequest -Uri $target.Url -Method Head -TimeoutSec 8 -UseBasicParsing -ErrorAction Stop
    Add-Report $report ("  {0}: 可达" -f $target.Name)
  } catch {
    $status = ''
    if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
    Add-Report $report ("  {0}: {1} {2}" -f $target.Name, $status, $_.Exception.Message.Split("`n")[0])
  }
}

# ── 落盘 ────────────────────────────────────────────────────────────────────
Save-Text -Path (Join-Path $stage 'MANIFEST.txt') -Text (($report -join "`r`n") + "`r`n")

$readme = @"
G-OpenSteamTool 诊断包（已脱敏）
生成：$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')

先看 MANIFEST.txt，它已经把关键行摘好了：
  1) 现场文件：config\lua 里有没有 manifest.lua（是否带 GOST-MANIFEST-LUA 标记）、
     取码源里有没有 127.0.0.1 这类只有你自己机器才有的端点。
  2) DLL 日志：steam\opensteamtool\ 里的 pattern/signature 行决定了钩子有没有装上。
  3) Steam 日志：steam\logs\content_log.txt 里 "manifest request received <状态码>"
     就是「无互联网连接」的直接判据。

已脱敏（保留结构与行数，值替换成 <REDACTED-*>）：
  Depot Key / addtoken / setAppTicket / setETicket / setStat / SteamID / AccountName。

对照说明见 docs\NO_INTERNET.md。
"@
Save-Text -Path (Join-Path $stage 'README.txt') -Text $readme

$zip = "$stage.zip"
if (-not $NoZip) {
  if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
  Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
}

Write-Output "诊断包已生成："
Write-Output ("  目录: " + $stage)
if (-not $NoZip) { Write-Output ("  压缩: " + $zip) }
Write-Output ""
Write-Output "先看 MANIFEST.txt 的这几段：config\lua / opensteamtool 日志 / manifest request 行。"
