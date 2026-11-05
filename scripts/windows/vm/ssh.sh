#!/usr/bin/env bash
# Run a command in the Windows test VM (PowerShell is the SSH shell): ssh.sh 'Get-Date'
# Gives up after FB_SSH_TIMEOUT seconds (default 90): the VM sometimes stalls.
exec perl -e 'alarm shift; exec @ARGV' "${FB_SSH_TIMEOUT:-90}" \
  ssh -q -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10 \
  -o ServerAliveInterval=10 -o ServerAliveCountMax=3 \
  -p 2223 -i "$HOME/.ssh/freeboard_vm" dev@127.0.0.1 "$@"
