import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 } })).newPage();
p.on("console", (m) => { if (m.text().includes("aion-dbg")) console.log(m.text()); });
await p.goto("http://127.0.0.1:4174/");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
await p.evaluate(() => {
  const now = Math.floor(Date.now() / 1000);
  const last = window.__data[window.__data.length - 1];
  const close = last.close - 2;
  window.__main.update({ time: now, open: last.close, high: last.close + 0.6, low: close - 0.6, close });
  window.__main.apply_options({ title: "AION", title_visible: true, countdown_visible: true });
});
await p.waitForTimeout(500);
await b.close();
