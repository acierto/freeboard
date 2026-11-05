# First-logon setup for the Freeboard Windows test VM (runs once, log in C:\setup.log).
$ErrorActionPreference = 'Continue'
# Network driver: virtio NetKVM from the virtio-win ISO.
foreach ($d in Get-PSDrive -PSProvider FileSystem) {
  $inf = Join-Path $d.Root 'NetKVM\w11\ARM64\netkvm.inf'
  if (Test-Path $inf) { pnputil /add-driver $inf /install }
}
# QEMU user networking passes no ICMP, so check with HTTPS instead of ping.
for ($i = 0; $i -lt 60; $i++) { if ((curl.exe -s -o NUL -w '%{http_code}' https://www.microsoft.com) -eq '200') { break }; Start-Sleep 5 }
Set-NetConnectionProfile -NetworkCategory Private -ErrorAction SilentlyContinue
# OpenSSH server with key login for the administrator account.
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
Set-Service sshd -StartupType Automatic
Start-Service sshd
Set-Content -Encoding ascii C:\ProgramData\ssh\administrators_authorized_keys '__SSH_PUBKEY__'
icacls C:\ProgramData\ssh\administrators_authorized_keys /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F'
New-ItemProperty -Path HKLM:\SOFTWARE\OpenSSH -Name DefaultShell -Value 'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe' -PropertyType String -Force
New-NetFirewallRule -Name sshd-in -DisplayName 'OpenSSH (test VM)' -Protocol TCP -LocalPort 22 -Action Allow -ErrorAction SilentlyContinue
# Keep the test session awake and unlocked.
powercfg /change monitor-timeout-ac 0
powercfg /change standby-timeout-ac 0
reg add HKLM\SOFTWARE\Policies\Microsoft\Windows\Personalization /v NoLockScreen /t REG_DWORD /d 1 /f
'done' | Set-Content C:\setup.done
