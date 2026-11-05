#!/usr/bin/env python3
"""Capture the VM's screen through QEMU's QMP socket (no tools needed inside the VM).

usage: screendump.py OUT.png
"""
import json, os, socket, sys

sock = os.path.expanduser("~/Downloads/freeboard-vm/qmp.sock")
out = os.path.abspath(sys.argv[1])
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(sock)
f = s.makefile("rw")
f.readline()  # greeting
def cmd(c, **a):
    f.write(json.dumps({"execute": c, "arguments": a} if a else {"execute": c}) + "\n"); f.flush()
    while True:
        r = json.loads(f.readline())
        if "return" in r or "error" in r:
            return r
cmd("qmp_capabilities")
# This QEMU build lacks libpng: dump PPM, then convert with macOS sips.
ppm = out + ".ppm"
r = cmd("screendump", filename=ppm)
if "error" in r:
    sys.exit(r["error"]["desc"])
import subprocess
subprocess.run(["sips", "-s", "format", "png", ppm, "--out", out], check=True, capture_output=True)
os.remove(ppm)
print(out)
