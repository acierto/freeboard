// Measure a running Freeboard process: memory and average CPU.
// Targets: hidden-in-tray memory <= 40 MB (webview destroyed), idle CPU < 1%
// averaged over 5 minutes. Reports actual numbers; never assumes a pass.
//
// Memory target measure (decision 2026-11-04): the app's own memory as the OS
// reports it — physical footprint on macOS (`footprint`), proportional set
// size on Linux (/proc/<pid>/smaps_rollup Pss), private working set on Windows.
// RSS is reported too, for reference only.
//
// Usage: node scripts/perf/measure-process.mjs --name freeboard [--seconds 300] [--interval 5]
// Run it after the window has been closed for more than 60 seconds so the
// webview is destroyed. On macOS and Linux it uses `ps`; on Windows PowerShell.
import { execFileSync } from "node:child_process";

const args = Object.fromEntries(
  process.argv.slice(2).reduce((a, v, i, all) => (v.startsWith("--") ? [...a, [v.slice(2), all[i + 1]]] : a), []),
);
const name = args.name ?? "freeboard";
const seconds = Number(args.seconds ?? 300);
const interval = Number(args.interval ?? 5);

function sampleUnix() {
  const out = execFileSync("ps", ["-axo", "pid=,rss=,time=,comm="], { encoding: "utf8" });
  const rows = out.trim().split("\n").map((l) => l.trim().split(/\s+/));
  const mine = rows.filter((r) => r.slice(3).join(" ").toLowerCase().endsWith(name.toLowerCase()));
  // Include child webview processes only when the window exists; the target
  // is about the app process tree.
  return mine.map((r) => ({ pid: r[0], rssKb: Number(r[1]), cpuSecs: toSecs(r[2]) }));
}
function toSecs(t) {
  // [[dd-]hh:]mm:ss(.xx)
  const [d, rest] = t.includes("-") ? t.split("-") : [0, t];
  const parts = rest.split(":").map(Number);
  while (parts.length < 3) parts.unshift(0);
  return Number(d) * 86400 + parts[0] * 3600 + parts[1] * 60 + parts[2];
}
function sampleWindows() {
  const ps = `Get-Process -Name ${name} | Select-Object Id,WorkingSet64,CPU | ConvertTo-Json`;
  const out = JSON.parse(execFileSync("powershell", ["-NoProfile", "-Command", ps], { encoding: "utf8" }) || "[]");
  return [].concat(out).map((p) => ({ pid: String(p.Id), rssKb: p.WorkingSet64 / 1024, cpuSecs: p.CPU }));
}
const sample = process.platform === "win32" ? sampleWindows : sampleUnix;

function ownMemoryMB(pids) {
  let total = 0;
  for (const pid of pids) {
    try {
      if (process.platform === "darwin") {
        const out = execFileSync("footprint", [String(pid)], { encoding: "utf8" });
        const m = out.match(/Footprint:\s*([\d.]+)\s*(KB|MB|GB)/);
        if (!m) return null;
        total += Number(m[1]) * { KB: 1 / 1024, MB: 1, GB: 1024 }[m[2]];
      } else if (process.platform === "linux") {
        const out = execFileSync("cat", [`/proc/${pid}/smaps_rollup`], { encoding: "utf8" });
        const m = out.match(/^Pss:\s*(\d+) kB/m);
        if (!m) return null;
        total += Number(m[1]) / 1024;
      } else if (process.platform === "win32") {
        const ps = `(Get-Process -Id ${pid}).PrivateMemorySize64`;
        total += Number(execFileSync("powershell", ["-NoProfile", "-Command", ps], { encoding: "utf8" })) / 1048576;
      }
    } catch {
      return null;
    }
  }
  return total;
}

const first = sample();
if (!first.length) {
  console.error(`No process named ${name} found.`);
  process.exit(2);
}
const start = Date.now();
let maxRss = 0;
const rssSeries = [];
let maxOwn = 0;
const ownSeries = [];
while (Date.now() - start < seconds * 1000) {
  const s = sample();
  const rss = s.reduce((t, p) => t + p.rssKb, 0) / 1024;
  rssSeries.push(rss);
  maxRss = Math.max(maxRss, rss);
  const own = ownMemoryMB(s.map((p) => p.pid));
  if (own !== null) {
    ownSeries.push(own);
    maxOwn = Math.max(maxOwn, own);
  }
  await new Promise((r) => setTimeout(r, interval * 1000));
}
const last = sample();
const cpu0 = first.reduce((t, p) => t + p.cpuSecs, 0);
const cpu1 = last.reduce((t, p) => t + p.cpuSecs, 0);
const wall = (Date.now() - start) / 1000;
const avgCpu = ((cpu1 - cpu0) / wall) * 100;
const avgRss = rssSeries.reduce((a, b) => a + b, 0) / rssSeries.length;
const measure = { darwin: "physical footprint", linux: "PSS", win32: "private working set" }[process.platform];
const result = {
  platform: process.platform,
  process: name,
  seconds: wall,
  memoryMeasure: measure,
  avgOwnMemoryMB: ownSeries.length ? +(ownSeries.reduce((a, b) => a + b, 0) / ownSeries.length).toFixed(1) : null,
  maxOwnMemoryMB: ownSeries.length ? +maxOwn.toFixed(1) : null,
  avgRssMB_reference: +avgRss.toFixed(1),
  maxRssMB_reference: +maxRss.toFixed(1),
  avgCpuPercent: +avgCpu.toFixed(3),
  memoryTargetMB: 40,
  cpuTargetPercent: 1,
  // Unknown (null) when the platform measure could not be read: never a pass.
  memoryTargetMet: ownSeries.length ? maxOwn <= 40 : null,
  cpuTargetMet: avgCpu < 1,
};
console.log(JSON.stringify(result, null, 2));
