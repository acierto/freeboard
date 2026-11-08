# Run a program on the logged-in desktop of the test VM (SSH runs in a
# separate session without a display): gui.ps1 <exe> [arguments] [-Name task]
# The task stays Running while the program runs, and stopping the task ends
# the program, so give each long-running program its own -Name.
param([Parameter(Mandatory)][string]$Exe, [string]$Arguments = '', [string]$Name = 'fbgui')
$a = if ($Arguments) { New-ScheduledTaskAction -Execute $Exe -Argument $Arguments } else { New-ScheduledTaskAction -Execute $Exe }
$p = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive
$s = New-ScheduledTaskSettingsSet -ExecutionTimeLimit 0
Register-ScheduledTask -TaskName $Name -Action $a -Principal $p -Settings $s -Force | Out-Null
Start-ScheduledTask -TaskName $Name
