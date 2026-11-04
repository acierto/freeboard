// Debug aid: list differing pixel coordinates between two PNGs.
import { readFileSync } from "node:fs";
import { PNG } from "pngjs";
const [a, b] = process.argv.slice(2).map((f) => PNG.sync.read(readFileSync(f)));
const out = [];
for (let y = 0; y < Math.min(a.height, b.height); y++)
  for (let x = 0; x < Math.min(a.width, b.width); x++) {
    const i = (y * a.width + x) * 4, j = (y * b.width + x) * 4;
    if (a.data[i] !== b.data[j] || a.data[i + 1] !== b.data[j + 1] || a.data[i + 2] !== b.data[j + 2]) out.push(`${x},${y} ref=${a.data.slice(i, i + 3).join("/")} act=${b.data.slice(j, j + 3).join("/")}`);
  }
console.log(out.length, "differing", a.width + "x" + a.height, b.width + "x" + b.height);
console.log(out.slice(0, 30).join("\n"));
