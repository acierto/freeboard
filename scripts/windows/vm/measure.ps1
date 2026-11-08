# Hidden-mode memory and idle CPU of Freeboard and its child processes
# (WebView2) in the Windows test VM. Run it on the desktop session, not over
# SSH (an SSH session drops during the 5 minutes):
#   gui.ps1 -Exe powershell.exe -Name measure -Arguments "-NoProfile -ExecutionPolicy Bypass -File C:\Users\dev\runmeasure.ps1"
# where runmeasure.ps1 is: & C:\Users\dev\measure.ps1 *> C:\Users\dev\measure-out.txt
# Memory: private working set (decision 2026-11-04). CPU: CPU seconds over wall time, one core.
param([int]$Seconds = 300, [int]$Interval = 10)
function Tree {
  $all = Get-CimInstance Win32_Process
  $root = @(Get-Process freeboard -ErrorAction SilentlyContinue | % Id)
  $ids = [Collections.Generic.List[int]]::new(); $root | % { $ids.Add($_) }
  do { $n = $ids.Count; $all | ? { $ids -contains $_.ParentProcessId -and -not ($ids -contains $_.ProcessId) } | % { $ids.Add($_.ProcessId) } } while ($ids.Count -ne $n)
  ,$ids
}
function Sample {
  $ids = Tree
  $perf = Get-CimInstance Win32_PerfRawData_PerfProc_Process | ? { $ids -contains $_.IDProcess }
  if ($ids.Count -eq 0) { return [pscustomobject]@{ Count = 0 } }
  $ps = Get-Process -Id $ids -ErrorAction SilentlyContinue
  [pscustomobject]@{ Count = $ids.Count; Names = (($ps | % Name) -join ','); WsPrivMB = (($perf | Measure WorkingSetPrivate -Sum).Sum / 1MB)
    PrivBytesMB = (($ps | Measure PrivateMemorySize64 -Sum).Sum / 1MB); WsMB = (($ps | Measure WorkingSet64 -Sum).Sum / 1MB); Cpu = ($ps | Measure CPU -Sum).Sum }
}
$first = Sample; if ($first.Count -eq 0) { 'no freeboard process'; exit 2 }
"processes: $($first.Names)"
$t = [Diagnostics.Stopwatch]::StartNew(); $s = @()
while ($t.Elapsed.TotalSeconds -lt $Seconds) { $s += Sample; Start-Sleep $Interval }
$last = Sample; $wall = $t.Elapsed.TotalSeconds
[pscustomobject]@{ seconds = [math]::Round($wall,1); processesAtEnd = $last.Names
  avgPrivateWorkingSetMB = [math]::Round(($s | Measure WsPrivMB -Average).Average,1); maxPrivateWorkingSetMB = [math]::Round(($s | Measure WsPrivMB -Maximum).Maximum,1)
  avgPrivateBytesMB_reference = [math]::Round(($s | Measure PrivBytesMB -Average).Average,1); avgWorkingSetMB_reference = [math]::Round(($s | Measure WsMB -Average).Average,1)
  avgCpuPercentOfOneCore = [math]::Round(($last.Cpu - $first.Cpu) / $wall * 100, 3); cores = $env:NUMBER_OF_PROCESSORS } | ConvertTo-Json
