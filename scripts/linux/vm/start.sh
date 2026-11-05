#!/usr/bin/env bash
# Start the disposable Freeboard Linux test VM with QEMU (HVF acceleration).
#   scripts/linux/vm/start.sh            create on first run, then boot
# VM files live in ~/Downloads/freeboard-vm/. SSH: ssh -p 2222 -i ~/.ssh/freeboard_vm dev@127.0.0.1
# QMP control socket: ~/Downloads/freeboard-vm/qmp.sock (screenshots and input from the host).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
V="$HOME/Downloads/freeboard-vm"
KEY="$HOME/.ssh/freeboard_vm"
FW=/opt/local/share/qemu/edk2-aarch64-code.fd
QEMU=/opt/local/bin/qemu-system-aarch64
IMG=/opt/local/bin/qemu-img
[ -f "$KEY" ] || ssh-keygen -q -t ed25519 -N "" -C freeboard-vm -f "$KEY"
if [ ! -f "$V/disk.qcow2" ]; then
  "$IMG" create -f qcow2 -F qcow2 -b "$V/ubuntu-24.04-server-cloudimg-arm64.img" "$V/disk.qcow2" 40G
  truncate -s 64M "$V/vars.fd"
  mkdir -p "$V/seed"
  sed "s|__SSH_PUBKEY__|$(cat "$KEY.pub")|" "$HERE/user-data" > "$V/seed/user-data"
  cp "$HERE/meta-data" "$V/seed/meta-data"
  hdiutil makehybrid -quiet -o "$V/seed.iso" -iso -joliet -default-volume-name cidata "$V/seed"
fi
truncate -s 64M "$V/fw.fd" && dd if="$FW" of="$V/fw.fd" conv=notrunc 2>/dev/null
exec "$QEMU" -name freeboard-linux -machine virt,highmem=on -accel hvf -cpu host -smp 4 -m 8192 \
  -drive if=pflash,format=raw,readonly=on,file="$V/fw.fd" \
  -drive if=pflash,format=raw,file="$V/vars.fd" \
  -drive if=virtio,format=qcow2,file="$V/disk.qcow2" \
  -drive if=virtio,format=raw,readonly=on,file="$V/seed.iso" \
  -device virtio-net-pci,netdev=n0 -netdev user,id=n0,hostfwd=tcp:127.0.0.1:2222-:22 \
  -device virtio-gpu-pci,xres=1440,yres=900 -device qemu-xhci -device usb-kbd -device usb-tablet \
  -display cocoa -serial file:"$V/serial.log" \
  -qmp unix:"$V/qmp.sock",server,nowait
