# Run a program on the logged-in desktop of the test VM (SSH runs in a
# separate session without a display): gui.ps1 <exe> [arguments]
param([Parameter(Mandatory)][string]$Exe, [string]$Arguments = '')
$a = New-ScheduledTaskAction -Execute $Exe -Argument $Arguments
$p = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive
Register-ScheduledTask -TaskName fbgui -Action $a -Principal $p -Force | Out-Null
Start-ScheduledTask -TaskName fbgui
