import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
const ctx = await b.newContext({ viewport: { width: 1280, height: 750 }, deviceScaleFactor: 1.5 });
const p = await ctx.newPage();
const q = new URLSearchParams({ runtimeTest: "presentedFrame", backend: "canvas2d", dpr: "1.5", spacing: "50", theme: "light" });
await p.goto("http://127.0.0.1:4174/?" + q);
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
const aion = await p.evaluate(() => {
  const c = window.__chart;
  return { pane_left: c.wasm.pane_left(), pane_w: c.wasm.time_scale_width(), axis_w: c.price_scale("right").width(), vw: innerWidth, dpr: devicePixelRatio };
});
await p.goto("http://127.0.0.1:4174/reference.html?" + q);
await p.waitForFunction(() => document.documentElement.dataset.ready === "true");
const ref = await p.evaluate(() => {
  const c = window.__reference.chart;
  const pane = c.panes()[0];
  return {
    pane_w: pane.getWidth ? pane.getWidth() : null,
    axis_w: c.priceScale("right").width(),
    vw: innerWidth, dpr: devicePixelRatio,
    time_w: c.timeScale().width(),
  };
});
console.log(JSON.stringify({ aion, ref }, null, 1));
await b.close();
