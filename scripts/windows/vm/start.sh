#!/usr/bin/env bash
# Start the disposable Freeboard Windows 11 ARM64 test VM with QEMU (HVF).
#   scripts/windows/vm/start.sh   creates the VM on first run (unattended install), then boots it
# Needs in ~/soft/vms/freeboard-windows/: windows.iso (Microsoft's Windows 11 Arm64 ISO)
# and virtio-win.iso (Fedora virtio drivers; NetKVM for the network card).
# SSH after setup: scripts/windows/vm/ssh.sh   QMP: qmp.py shot|keys
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
V="${VMS_DIR:-$HOME/soft/vms}/freeboard-windows"
KEY="$HOME/.ssh/freeboard_vm"
# Homebrew QEMU 9.x: its UEFI firmware boots from NVMe (the 2022 MacPorts
# QEMU 7.1 firmware cannot, although Windows setup itself sees the disk).
FW=/opt/homebrew/share/qemu/edk2-aarch64-code.fd
QEMU=/opt/homebrew/bin/qemu-system-aarch64
IMG=/opt/homebrew/bin/qemu-img
[ -f "$KEY" ] || ssh-keygen -q -t ed25519 -N "" -C freeboard-vm -f "$KEY"
INSTALL=()
if [ ! -f "$V/disk.qcow2" ]; then
  "$IMG" create -q -f qcow2 "$V/disk.qcow2" 64G
  truncate -s 64M "$V/vars.fd"
  rm -rf "$V/unattend" && mkdir -p "$V/unattend"
  cp "$HERE/autounattend.xml" "$HERE/install.cmd" "$V/unattend/"
  sed "s|__SSH_PUBKEY__|$(cat "$KEY.pub")|" "$HERE/setup.ps1" > "$V/unattend/setup.ps1"
  rm -f "$V/unattend.iso"
  hdiutil makehybrid -quiet -o "$V/unattend.iso" -iso -joliet -default-volume-name UNATTEND "$V/unattend"
fi
# The installer ISO is attached only for the first boot. install.cmd powers
# the VM off after applying the image; start.sh then marks it installed and
# boots the disk.
if [ ! -f "$V/installed" ]; then
  INSTALL=(-device usb-storage,drive=inst,bootindex=0 -drive if=none,id=inst,media=cdrom,readonly=on,file="$V/windows.iso")
fi
truncate -s 64M "$V/fw.fd" && dd if="$FW" of="$V/fw.fd" conv=notrunc 2>/dev/null
run() { "$QEMU" -name freeboard-windows -machine virt,highmem=on -accel hvf -cpu host -smp 4 -m 8192 \
  -drive if=pflash,format=raw,readonly=on,file="$V/fw.fd" \
  -drive if=pflash,format=raw,file="$V/vars.fd" \
  -device nvme,drive=sys,serial=fbsys,bootindex=1 -drive if=none,id=sys,format=qcow2,file="$V/disk.qcow2" \
  -device qemu-xhci -device usb-kbd -device usb-tablet \
  ${INSTALL[@]+"${INSTALL[@]}"} \
  -device usb-storage,drive=vio -drive if=none,id=vio,media=cdrom,readonly=on,file="$V/virtio-win.iso" \
  -device usb-storage,drive=una -drive if=none,id=una,media=cdrom,readonly=on,file="$V/unattend.iso" \
  -device virtio-net-pci,netdev=n0 -netdev user,id=n0,hostfwd=tcp:127.0.0.1:2223-:22 \
  -device ramfb -display cocoa \
  -qmp unix:"$V/qmp.sock",server,nowait
}
if [ ! -f "$V/installed" ]; then
  run
  touch "$V/installed"
  INSTALL=()
fi
run
