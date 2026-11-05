#!/usr/bin/env bash
# Run a command in the test VM as `dev`, inside the graphical session's
# environment (DISPLAY, D-Bus) so GUI tools work from the host.
exec ssh -q -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5 \
  -p 2222 -i "$HOME/.ssh/freeboard_vm" dev@127.0.0.1 \
  'export DISPLAY=:0 XAUTHORITY=$(ls /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1); [ -n "$XAUTHORITY" ] || export XAUTHORITY=/run/user/1000/gdm/Xauthority; export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus; '"$*"
