#!/usr/bin/env python3
"""Drive the Windows test VM through QEMU's QMP socket (no tools needed inside the VM).

usage: qmp.py shot OUT.png        screenshot
       qmp.py keys KEY [KEY...]   press keys, e.g. ret, spc, ctrl-alt-delete
"""
import json, os, socket, subprocess, sys

s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(os.path.expanduser("~/Downloads/freeboard-winvm/qmp.sock"))
f = s.makefile("rw")
f.readline()  # greeting


def cmd(c, **a):
    f.write(json.dumps({"execute": c, "arguments": a} if a else {"execute": c}) + "\n")
    f.flush()
    while True:
        r = json.loads(f.readline())
        if "return" in r or "error" in r:
            return r


cmd("qmp_capabilities")
if sys.argv[1] == "shot":
    out = os.path.abspath(sys.argv[2])
    ppm = out + ".ppm"  # this QEMU build lacks libpng
    r = cmd("screendump", filename=ppm)
    if "error" in r:
        sys.exit(r["error"]["desc"])
    subprocess.run(["sips", "-s", "format", "png", ppm, "--out", out], check=True, capture_output=True)
    os.remove(ppm)
    print(out)
elif sys.argv[1] == "keys":
    for k in sys.argv[2:]:
        keys = [{"type": "qcode", "data": p} for p in k.split("-")]
        print(cmd("send-key", keys=keys))
