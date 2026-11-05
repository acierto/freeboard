#!/usr/bin/env bash
# Screenshot the VM's X display into a host file: shot.sh OUT.png
set -euo pipefail
"$(dirname "$0")/ssh.sh" 'import -window root /tmp/freeboard-shot.png'
scp -q -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -P 2222 -i "$HOME/.ssh/freeboard_vm" dev@127.0.0.1:/tmp/freeboard-shot.png "$1"
