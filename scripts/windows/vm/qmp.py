#!/usr/bin/env python3
"""Drive the Windows test VM through QEMU's QMP socket (no tools needed inside the VM).

usage: qmp.py shot OUT.png        screenshot
       qmp.py keys KEY [KEY...]   press keys, e.g. ret, spc, ctrl-alt-delete
       qmp.py type TEXT           type text (letters, digits, common symbols), no Enter
       qmp.py click X Y [W H]     left click at X,Y of a W x H screen (default 800x600)
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
elif sys.argv[1] == "type":
    import time
    sym = {" ": "spc", "\\": "backslash", ":": "shift-semicolon", ".": "dot", "-": "minus", "_": "shift-minus",
           "/": "slash", ";": "semicolon", "=": "equal", ",": "comma", "'": "apostrophe", '"': "shift-apostrophe",
           "|": "shift-backslash", "$": "shift-4", "%": "shift-5", ">": "shift-dot", "<": "shift-comma",
           "~": "shift-grave_accent", "@": "shift-2", "#": "shift-3", "!": "shift-1", "+": "shift-equal", "{": "shift-bracket_left", "}": "shift-bracket_right", "[": "bracket_left", "]": "bracket_right", "`": "grave_accent", "?": "shift-slash", "^": "shift-6", "(": "shift-9", ")": "shift-0", "*": "shift-8", "&": "shift-7"}
    for ch in sys.argv[2]:
        if ch in sym:
            k = sym[ch]
        elif ch.isupper():
            k = "shift-" + ch.lower()
        else:
            k = ch
        r = cmd("send-key", keys=[{"type": "qcode", "data": p} for p in k.split("-")], **{"hold-time": 40})
        if "error" in r:
            sys.exit(f"cannot type {ch!r}: {r['error']['desc']}")
        time.sleep(0.09)
elif sys.argv[1] == "click":
    import time
    x, y = int(sys.argv[2]), int(sys.argv[3])
    w, h = (int(sys.argv[4]), int(sys.argv[5])) if len(sys.argv) > 5 else (800, 600)
    ax = lambda a, v, m: {"type": "abs", "data": {"axis": a, "value": v * 32767 // m}}
    cmd("input-send-event", events=[ax("x", x, w), ax("y", y, h)])
    for down in (True, False):
        cmd("input-send-event", events=[{"type": "btn", "data": {"button": "left", "down": down}}])
        time.sleep(0.05)
elif sys.argv[1] == "keys":
    for k in sys.argv[2:]:
        keys = [{"type": "qcode", "data": p} for p in k.split("-")]
        print(cmd("send-key", keys=keys))
