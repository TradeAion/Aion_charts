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
// one label band: rows 86..100, cols x0..end
const x0 = aion.width - 58 * 1.5;
const row_report = [];
for (let y = 86; y <= 100; y++) {
  const a = [], r = [];
  for (let x = x0; x < aion.width; x++) {
    const o = (y * aion.width + x) * 4;
    a.push(aion.data[o] < 128 ? "1" : "0");
    r.push(ref.data[o] < 128 ? "1" : "0");
  }
  row_report.push(`y=${y} A:${a.join("")}`);
  row_report.push(`     R:${r.join("")}`);
}
console.log(row_report.join("\n"));
await b.close();
