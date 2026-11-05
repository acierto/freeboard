@echo off
rem Apply Windows to the VM's own NVMe disk (disk 0) without the new Setup UI,
rem which fails in this VM ("Calculating disk space failed", 0xC1900101).
rem Runs from the windowsPE pass of autounattend.xml. Test VM only.
for %%d in (C D E F G H I J K L M N O P Q R) do if exist %%d:\sources\install.wim set SRC=%%d:
for %%d in (C D E F G H I J K L M N O P Q R) do if exist %%d:\install.cmd set UNA=%%d:
if not defined SRC (echo Windows ISO not found & exit /b 1)
(
echo select disk 0
echo clean
echo convert gpt
echo create partition efi size=300
echo format quick fs=fat32 label=System
echo assign letter=S
echo create partition msr size=16
echo create partition primary
echo format quick fs=ntfs label=Windows
echo assign letter=W
) > X:\disk.txt
diskpart /s X:\disk.txt || exit /b 1
dism /apply-image /imagefile:%SRC%\sources\install.wim /name:"Windows 11 Pro" /applydir:W:\ || exit /b 1
bcdboot W:\Windows /s S: /f UEFI || exit /b 1
mkdir W:\Windows\Panther
copy /y %UNA%\autounattend.xml W:\Windows\Panther\unattend.xml
rem Power off: start.sh then marks the VM installed and boots the disk.
wpeutil shutdown
