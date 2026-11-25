param([Parameter(Mandatory=$true)][string]$Archive,[Parameter(Mandatory=$true)][string]$Version)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$required = @('ICC-Profile-Studio.exe','ICC-Profile-Studio.exe.sha256','README.md','ROADMAP.md','PROFILE_WORKFLOW.md','CIE_DATA.md','STUDIO_VERSION','RELEASE_QA.md','BUILD_INFO.json','Cargo.lock')
$zip = [IO.Compression.ZipFile]::OpenRead((Resolve-Path $Archive).Path)
try {
  $names = @($zip.Entries | ForEach-Object FullName)
  if ($names.Count -ne $required.Count -or @($names | Select-Object -Unique).Count -ne $required.Count) { throw 'Unexpected archive entry count or duplicates' }
  foreach ($name in $required) { if ($name -cnotin $names) { throw "Missing archive entry: $name" } }
  function Read-EntryText([string]$name) {
    $r = [IO.StreamReader]::new($zip.GetEntry($name).Open())
    try { $r.ReadToEnd() } finally { $r.Dispose() }
  }
  if ((Read-EntryText 'STUDIO_VERSION').Trim() -cne $Version) { throw 'Archive version mismatch' }
  $info = Read-EntryText 'BUILD_INFO.json' | ConvertFrom-Json
  if ($info.version -cne $Version -or $info.target -cne 'x86_64-pc-windows-msvc' -or $info.source_commit -notmatch '^[0-9a-f]{40}$' -or $info.lock_sha256 -notmatch '^[0-9a-f]{64}$') { throw 'Invalid build provenance' }
  $lockStream = $zip.GetEntry('Cargo.lock').Open()
  $lockSha = [Security.Cryptography.SHA256]::Create()
  try { $actualLock = [Convert]::ToHexString($lockSha.ComputeHash($lockStream)).ToLowerInvariant() } finally { $lockStream.Dispose(); $lockSha.Dispose() }
  if ($actualLock -cne $info.lock_sha256) { throw 'Packaged Cargo.lock checksum mismatch' }
  $hashLine = (Read-EntryText 'ICC-Profile-Studio.exe.sha256').Trim()
  if ($hashLine -notmatch '^([0-9a-f]{64})  ICC-Profile-Studio\.exe$') { throw 'Invalid checksum format' }
  $expected = $Matches[1]
  $exe = $zip.GetEntry('ICC-Profile-Studio.exe')
  $stream = $exe.Open()
  $sha = [Security.Cryptography.SHA256]::Create()
  try { $actual = [Convert]::ToHexString($sha.ComputeHash($stream)).ToLowerInvariant() } finally { $stream.Dispose(); $sha.Dispose() }
  if ($actual -cne $expected -or $actual -cne $info.exe_sha256) { throw 'Packaged executable checksum mismatch' }
  $stream = $exe.Open()
  $reader = [IO.BinaryReader]::new($stream)
  try {
    $header = $reader.ReadBytes(64)
    if ($header.Length -ne 64 -or $header[0] -ne 0x4d -or $header[1] -ne 0x5a) { throw 'Not an MZ executable' }
    $pe = [BitConverter]::ToInt32($header,60)
    if ($pe -lt 64 -or $pe -gt 1048576 -or $pe + 6 -gt $exe.Length) { throw 'Invalid PE header offset' }
    if ($reader.ReadBytes($pe-64).Length -ne $pe-64) { throw 'Truncated PE header' }
    if ($reader.ReadUInt32() -ne 0x00004550 -or $reader.ReadUInt16() -ne 0x8664) { throw 'Expected Windows AMD64 executable' }
  } finally { $reader.Dispose() }
  Write-Host "Verified $Version Windows x64 archive; SHA256 $actual; source $($info.source_commit)"
} finally { $zip.Dispose() }
