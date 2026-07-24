import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
await p.goto("http://127.0.0.1:4174/");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
await p.evaluate(() => {
  const chart = window.__chart;
  chart.apply_options({ grid: { vertLines: { style: 1, color: "#888888" }, horzLines: { style: 1, color: "#888888" } } });
  chart.render();
});
await p.waitForTimeout(300);
const g = await p.evaluate(() => {
  const c = window.__chart;
  const r = document.querySelectorAll("canvas")[3].getBoundingClientRect();
  return { left: r.left, top: r.top, pane_left: c.wasm.pane_left(), y100: c.price_to_coordinate(100) };
});
const shot = PNG.sync.read(await p.screenshot());
for (const dy of [-1, 0, 1]) {
  const y = Math.round(g.top + g.y100) + dy;
  const runs = [];
  let cur = null;
  for (let x = Math.floor(g.left + g.pane_left); x < g.left + 400; x++) {
    const o = (y * shot.width + x) * 4;
    const dark = shot.data[o] < 200 && shot.data[o+1] < 200 && shot.data[o+2] < 200;
    if (dark && !cur) cur = { start: x, end: x };
    else if (dark) cur.end = x;
    else if (cur) { runs.push(cur.end - cur.start + 1); cur = null; }
  }
  if (runs.length > 3) { console.log(`dy=${dy} dash widths:`, runs.slice(0, 16).join(",")); break; }
}
await b.close();
