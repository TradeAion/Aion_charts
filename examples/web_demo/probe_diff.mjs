import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
const ctx = await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1.5 });
const p = await ctx.newPage();
const q = new URLSearchParams({ runtimeTest: "presentedFrame", backend: "canvas2d", dpr: "1.5", spacing: "50", theme: "light" });
await p.goto("http://127.0.0.1:4174/?" + q);
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
const aion = PNG.sync.read(await p.screenshot());
await p.goto("http://127.0.0.1:4174/reference.html?" + q);
await p.waitForFunction(() => document.documentElement.dataset.ready === "true");
const ref = PNG.sync.read(await p.screenshot());
const axis_w = 58 * 1.5;
const x0 = aion.width - axis_w;
const rows = [];
for (let y = 0; y < aion.height; y++) {
  let diff = 0;
  for (let x = x0; x < aion.width; x++) {
    const o = (y * aion.width + x) * 4;
    if (Math.abs(aion.data[o] - ref.data[o]) > 10 || Math.abs(aion.data[o+1] - ref.data[o+1]) > 10 || Math.abs(aion.data[o+2] - ref.data[o+2]) > 10) diff++;
  }
  if (diff > 3) rows.push([y, diff]);
}
// group contiguous
const groups = [];
for (const [y, d] of rows) {
  const g = groups[groups.length - 1];
  if (g && y - g.end <= 1) { g.end = y; g.total += d; } else groups.push({ start: y, end: y, total: d });
}
console.log(JSON.stringify(groups.slice(0, 20)));
await b.close();
