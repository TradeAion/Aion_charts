import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
await p.goto("http://127.0.0.1:4174/");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
await p.evaluate(() => {
  const chart = window.__chart;
  chart.apply_options({ grid: { vertLines: { style: 1 }, horzLines: { style: 1 } } });
  chart.render();
});
await p.waitForTimeout(300);
// measure the horz grid dot pattern along a row inside the pane
const row_y = await p.evaluate(() => Math.round(window.__chart.coordinate_to_price ? window.__chart.price_to_coordinate(100) : 300));
const shot = PNG.sync.read(await p.screenshot());
const g = await p.evaluate(() => { const c = window.__chart; const r = document.querySelectorAll("canvas")[3].getBoundingClientRect(); return { left: r.left, top: r.top, pane_left: c.wasm.pane_left() }; });
// scan the row for the grid line (dotted): record run lengths of colored pixels
const runs = [];
let cur = null;
for (let x = Math.floor(g.left + g.pane_left); x < g.left + 600; x++) {
  const o = (row_y * shot.width + x) * 4;
  const r = shot.data[o], gg = shot.data[o+1], b2 = shot.data[o+2];
  const colored = r < 230 || gg < 230 || b2 < 230;
  if (colored && !cur) cur = { start: x, end: x };
  else if (colored) cur.end = x;
  else if (cur) { runs.push(cur); cur = null; }
}
console.log(JSON.stringify({ row_y, runs: runs.slice(0, 20) }));
await b.close();
