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
  window.__main.update({ time: now, open: last.close, high: last.close + 0.6, low: close - 0.6, close });
  window.__main.apply_options({ title: "AION", title_visible: true, countdown_visible: true });
});
await p.waitForTimeout(300);
const y0 = await p.evaluate(() => Math.round(window.__main.price_to_coordinate(window.__cluster_close ?? 100)));
const pane_w = await p.evaluate(() => window.__chart.time_scale().width());
const data_url = await p.evaluate(() => window.__chart.take_screenshot().toDataURL("image/png"));
const shot = PNG.sync.read(Buffer.from(data_url.split(",")[1], "base64"));
// locate the chip's top-left by scanning for the chip color
let chip_left = -1, chip_top = -1;
outer: for (let y = 0; y < shot.height; y++) {
  for (let x = 0; x < pane_w - 4; x++) {
    const o = (y * shot.width + x) * 4;
    if (Math.abs(shot.data[o] - 172) < 12 && Math.abs(shot.data[o+1] - 60) < 12 && Math.abs(shot.data[o+2] - 58) < 12) {
      chip_left = x; chip_top = y; break outer;
    }
  }
}
console.log("chip_left", chip_left, "chip_top", chip_top, "y0", y0, "pane_w", pane_w);
for (let dy = 0; dy < 6; dy++) {
  const row = [];
  for (let dx = 0; dx < 8; dx++) {
    const o = ((chip_top + dy) * shot.width + (chip_left + dx)) * 4;
    row.push([shot.data[o], shot.data[o+1], shot.data[o+2], shot.data[o+3]].join("/"));
  }
  console.log(row.join("  "));
}
await b.close();
