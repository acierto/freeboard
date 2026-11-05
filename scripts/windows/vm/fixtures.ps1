# Disposable test data for the Windows test VM (never run on a real machine).
$ErrorActionPreference = 'Stop'
$h = $env:USERPROFILE
function Fill($path, $mb) {
  New-Item -ItemType Directory -Force (Split-Path $path) | Out-Null
  $b = New-Object byte[] ($mb * 1MB); (New-Object Random 7).NextBytes($b)
  [IO.File]::WriteAllBytes($path, $b)
}
function Age($path, $days) {
  $t = (Get-Date).AddDays(-$days)
  Get-ChildItem -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.LastWriteTime = $t; $_.LastAccessTime = $t }
  $i = Get-Item -LiteralPath $path -Force; $i.LastWriteTime = $t; $i.LastAccessTime = $t
}
# Idle Node project (node_modules, Review/Safe by age).
$p = "$h\projects\old-shop"
Set-Content -Force (New-Item -ItemType File -Force "$p\package.json") '{"name":"old-shop","version":"1.0.0"}'
foreach ($m in 'left-pad','lodash','react') { Fill "$p\node_modules\$m\index.js" 20 }
Age $p 75
# A Git repository root must never be offered.
New-Item -ItemType Directory -Force "$h\projects\live-repo\.git" | Out-Null
# Old temp files (Windows temp detector).
Fill "$env:TEMP\fb-old-1.tmp" 15
Fill "$env:TEMP\fb-old-dir\cache.bin" 10
Age "$env:TEMP\fb-old-1.tmp" 30; Age "$env:TEMP\fb-old-dir" 30
# Old installer in Downloads.
Fill "$h\Downloads\old-tool-setup.msi" 40
Age "$h\Downloads\old-tool-setup.msi" 120
# Gradle caches with a log from a daemon that is no longer running.
Fill "$h\.gradle\caches\modules-2\files-2.1\dep.jar" 30
New-Item -ItemType Directory -Force "$h\.gradle\daemon\8.10" | Out-Null
Set-Content "$h\.gradle\daemon\8.10\daemon-99999.out.log" 'stopped daemon'
'fixtures ready'
