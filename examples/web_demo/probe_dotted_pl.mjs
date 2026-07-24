import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
await p.goto("http://127.0.0.1:4174/");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
await p.evaluate(() => { window.__main.apply_options({ price_line_style: 1, price_line_color: "#222222" }); window.__chart.render(); });
await p.waitForTimeout(200);
const info = await p.evaluate(() => {
  const c = window.__chart;
  return { pane_left: c.wasm.pane_left(), y: Math.round(c.price_to_coordinate(window.__data[window.__data.length - 1].close)) };
});
const data_url = await p.evaluate(() => window.__chart.take_screenshot().toDataURL("image/png"));
const shot = PNG.sync.read(Buffer.from(data_url.split(",")[1], "base64"));
const runs = [];
let cur = null;
for (let x = info.pane_left; x < info.pane_left + 200; x++) {
  const o = (info.y * shot.width + x) * 4;
  const dark = shot.data[o] < 120 && shot.data[o+1] < 120 && shot.data[o+2] < 120;
  if (dark && !cur) cur = { s: x, e: x };
  else if (dark) cur.e = x;
  else if (cur) { runs.push(cur.e - cur.s + 1); cur = null; }
}
console.log("dotted price-line dash widths (first 16):", runs.slice(0, 16).join(","));
await b.close();
