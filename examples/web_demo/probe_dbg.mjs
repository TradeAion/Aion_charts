import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
await p.goto("http://127.0.0.1:4174/?backend=canvas2d");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
const out = await p.evaluate(() => {
  const c = window.__chart;
  c.apply_options({ grid: { vertLines: { style: 1, color: "#333333" }, horzLines: { style: 1, color: "#333333" } } });
  c.render();
  const o = c.options();
  return { grid: o.grid, y100: c.price_to_coordinate(100), pane_h: c.wasm.pane_height(0), vw: innerWidth, vh: innerHeight };
});
console.log(JSON.stringify(out, null, 1));
await b.close();
