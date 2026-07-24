import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
await p.goto("http://127.0.0.1:4174/");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
await p.evaluate(() => {
  const now = Math.floor(Date.now() / 1000);
  const last = window.__data[window.__data.length - 1];
  const close = last.close - 2;
  window.__main.update({ time: now, open: last.close, high: last.close + 0.6, low: close - 0.6, close });
  window.__main.apply_options({ title: "AION", title_visible: true, countdown_visible: true });
});
await p.waitForTimeout(300);
const data_url = await p.evaluate(() => window.__chart.take_screenshot().toDataURL("image/png"));
const shot = PNG.sync.read(Buffer.from(data_url.split(",")[1], "base64"));
for (let y = 371; y <= 381; y++) {
  const row = [];
  for (let x = 1172; x <= 1188; x++) {
    const o = (y * shot.width + x) * 4;
    const a = shot.data[o+3];
    row.push(a === 255 ? "#" : a === 0 ? "." : "~");
  }
  console.log(`y${y}: ${row.join("")}  (x1172..1188)`);
}
await b.close();
