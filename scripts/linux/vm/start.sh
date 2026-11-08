#!/usr/bin/env bash
# Start the disposable Freeboard Linux test VM with QEMU (HVF acceleration).
#   scripts/linux/vm/start.sh            create on first run, then boot
# VM files live in ~/soft/vms/linux/. SSH: ssh -p 2222 -i ~/.ssh/freeboard_vm dev@127.0.0.1
# QMP control socket: ~/soft/vms/linux/qmp.sock (screenshots and input from the host).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
V="${VMS_DIR:-$HOME/soft/vms}/linux"
KEY="$HOME/.ssh/freeboard_vm"
FW=/opt/local/share/qemu/edk2-aarch64-code.fd
QEMU=/opt/local/bin/qemu-system-aarch64
IMG=/opt/local/bin/qemu-img
[ -f "$KEY" ] || ssh-keygen -q -t ed25519 -N "" -C freeboard-vm -f "$KEY"
# Ubuntu's minimal cloud image (about 220 MB, the smallest official one);
# cloud-init adds the minimal GNOME desktop on first boot.
BASE=ubuntu-24.04-minimal-cloudimg-arm64.img
URL=https://cloud-images.ubuntu.com/minimal/releases/noble/release
mkdir -p "$V"
if [ ! -f "$V/$BASE" ]; then
  curl -fL -o "$V/$BASE.part" "$URL/$BASE"
  (cd "$V" && curl -fsSL "$URL/SHA256SUMS" | grep " \*\?$BASE\$" | sed "s/$BASE.part/$BASE/" > "$BASE.sha256" && mv "$BASE.part" "$BASE" && shasum -a 256 -c "$BASE.sha256")
fi
if [ ! -f "$V/disk.qcow2" ]; then
  "$IMG" create -f qcow2 -F qcow2 -b "$V/$BASE" "$V/disk.qcow2" 40G
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
