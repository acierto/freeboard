import { PNG } from "pngjs";
import pixelmatch from "pixelmatch";

/**
 * Compare two PNG buffers. Strict by default: threshold 0 and anti-aliased
 * pixels included, so any differing pixel counts. Returns
 * { diffPixels, total, sizeMismatch, diffPng }.
 */
export function comparePng(refBuf, actBuf, tolerance = { threshold: 0, maxDiffPixels: 0 }) {
  const a = PNG.sync.read(refBuf);
  const b = PNG.sync.read(actBuf);
  const w = Math.max(a.width, b.width);
  const h = Math.max(a.height, b.height);
  const pad = (img) => {
    if (img.width === w && img.height === h) return img;
    const out = new PNG({ width: w, height: h });
    out.data.fill(0);
    PNG.bitblt(img, out, 0, 0, img.width, img.height, 0, 0);
    return out;
  };
  const pa = pad(a);
  const pb = pad(b);
  const diff = new PNG({ width: w, height: h });
  const diffPixels = pixelmatch(pa.data, pb.data, diff.data, w, h, {
    threshold: tolerance.threshold ?? 0,
    includeAA: true,
    alpha: 0.2,
    diffColor: [255, 0, 0],
  });
  return {
    diffPixels,
    total: w * h,
    sizeMismatch: a.width !== b.width || a.height !== b.height ? `${a.width}x${a.height} vs ${b.width}x${b.height}` : null,
    diffPng: PNG.sync.write(diff),
  };
}
