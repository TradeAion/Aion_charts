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
// find the chip's true left edge on the chip mid row
let left = -1, top = -1;
for (let x = 1000; x < 1222; x++) {
  const o = (378 * shot.width + x) * 4;
  if (shot.data[o+3] > 100 && Math.abs(shot.data[o] - 172) < 14 && Math.abs(shot.data[o+1] - 60) < 14) { left = x; break; }
}
for (let y = 300; y < 420; y++) {
  const o = (y * shot.width + (left + 6)) * 4;
  if (shot.data[o+3] > 100 && Math.abs(shot.data[o] - 172) < 14 && Math.abs(shot.data[o+1] - 60) < 14) { top = y; break; }
}
console.log("true box left:", left, "top:", top);
for (let dy = -2; dy < 6; dy++) {
  const row = [];
  for (let dx = -2; dx < 8; dx++) {
    const o = ((top + dy) * shot.width + (left + dx)) * 4;
    const a = shot.data[o+3];
    row.push(a === 255 ? "#" : a === 0 ? "." : "~");
  }
  console.log(`y${top + dy}: ${row.join("")}   x${left - 2}..${left + 7}`);
}
await b.close();
