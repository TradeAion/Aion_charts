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
const pane_w = await p.evaluate(() => window.__chart.time_scale().width());
const data_url = await p.evaluate(() => window.__chart.take_screenshot().toDataURL("image/png"));
const shot = PNG.sync.read(Buffer.from(data_url.split(",")[1], "base64"));
// dump the region around (chip_left-4 .. chip_left+3, chip_top-3 .. chip_top+3)
for (let dy = -3; dy < 4; dy++) {
  const row = [];
  for (let dx = -4; dx < 5; dx++) {
    const o = ((375 + dy) * shot.width + (1183 + dx)) * 4;
    const a = shot.data[o+3];
    row.push(a === 255 ? "#" : a === 0 ? "." : "~");
  }
  console.log(`y${375+dy}: ${row.join("")}`);
}
await b.close();
