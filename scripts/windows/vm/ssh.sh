#!/usr/bin/env bash
# Run a command in the Windows test VM (PowerShell is the SSH shell): ssh.sh 'Get-Date'
exec ssh -q -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5 \
  -p 2223 -i "$HOME/.ssh/freeboard_vm" dev@127.0.0.1 "$@"
