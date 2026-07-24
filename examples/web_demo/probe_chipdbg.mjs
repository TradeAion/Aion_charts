import { chromium } from "playwright";
import { PNG } from "pngjs";

const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
await p.goto("http://127.0.0.1:4174/");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
await p.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
await p.evaluate(() => {
  const now = Math.floor(Date.now() / 1000);
  const last = window.__data[window.__data.length - 1];
  const close = last.close - 2;
  window.__cluster_close = close;
  window.__main.update({ time: now, open: last.close, high: last.close + 0.6, low: close - 0.6, close });
  window.__main.apply_options({ title: "AION", title_visible: true, countdown_visible: true, last_value_visible: false });
});
await p.waitForTimeout(300);
const out = await p.evaluate(() => {
  const chart = window.__chart;
  const y = window.__main.price_to_coordinate(window.__cluster_close);
  return { pane_w: chart.time_scale().width(), y, opts: window.__main.options() };
});
console.log(JSON.stringify({ pane_w: out.pane_w, y: out.y, title: out.opts.title, lvv: out.opts.last_value_visible, tv: out.opts.title_visible, cd: out.opts.countdown_visible }));
const data_url = await p.evaluate(() => window.__chart.take_screenshot().toDataURL("image/png"));
const shot = PNG.sync.read(Buffer.from(data_url.split(",")[1], "base64"));
const pane_w = out.pane_w, y = Math.round(out.y);
// dump colors around the chip region
for (let dy = -25; dy <= 25; dy += 5) {
  const row = [];
  for (let dx = -60; dx <= 10; dx += 5) {
    const o = ((y + dy) * shot.width + (pane_w + dx)) * 4;
    row.push([shot.data[o], shot.data[o+1], shot.data[o+2]].join("/"));
  }
  console.log(`y${dy >= 0 ? "+" : ""}${dy}:`, row.join(" "));
}
await b.close();
