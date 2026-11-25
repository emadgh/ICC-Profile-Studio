$ErrorActionPreference = 'Stop'
$root = Join-Path $PSScriptRoot '../assets/cie'
New-Item -ItemType Directory -Path $root -Force | Out-Null
$datasets = [ordered]@{
  'CIE_xyz_1931_2deg.csv' = 'fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1'
  'CIE_xyz_1964_10deg.csv' = '1b27fd4e8ca1167b47c3a6aee3aafe56abc57eae51fa20032cb83704224a27dc'
  'CIE_std_illum_D50.csv' = 'b23049c6f7b266c1c1fbe147aa271e8930ca02d6e569c5ae1804c036faea4193'
  'CIE_std_illum_D65.csv' = 'e76f210bffff3d552ef7113025da5f325d5dfec200dd4b878b1a2f3a507032cb'
  'CIE_std_illum_A_1nm.csv' = '61ef23fe146b8b665c74706717ab28cec7db6c9022993490bdc71991f43cb59b'
}
foreach ($name in $datasets.Keys) {
  $path = Join-Path $root $name
  if (!(Test-Path $path) -or (Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $datasets[$name]) {
    $tmp = "$path.download"
    try {
      Invoke-WebRequest -Uri "https://files.cie.co.at/Publications-datasets/$name" -OutFile $tmp -TimeoutSec 60
      if ((Get-FileHash $tmp -Algorithm SHA256).Hash.ToLowerInvariant() -ne $datasets[$name]) {
        throw "CIE data checksum mismatch: $name"
      }
      Move-Item -Force $tmp $path
    } finally { if (Test-Path $tmp) { Remove-Item $tmp -Force } }
  }
}
