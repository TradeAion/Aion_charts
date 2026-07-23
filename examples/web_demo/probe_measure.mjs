import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
const ctx = await b.newContext({ viewport: { width: 1280, height: 750 }, deviceScaleFactor: 1.5 });
const p = await ctx.newPage();
const q = new URLSearchParams({ runtimeTest: "presentedFrame", backend: "canvas2d", dpr: "1.5", spacing: "50", theme: "light" });
await p.goto("http://127.0.0.1:4174/reference.html?" + q);
await p.waitForFunction(() => document.documentElement.dataset.ready === "true");
const out = await p.evaluate(() => {
  const c = window.__reference.chart;
  const before = c.priceScale("right").width();
  // force an LWC fullUpdate (any applyOptions triggers _adjustSizeImpl with fresh optimalWidth)
  c.applyOptions({ leftPriceScale: { borderColor: "#2B2B43" } });
  const after = c.priceScale("right").width();
  // measure the label text at LWC's font
  const ctx2 = document.createElement("canvas").getContext("2d");
  ctx2.font = `12px -apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif`;
  const widest = ["98.00", "96.00", "94.00", "92.00", "90.00"].map((t) => [t, ctx2.measureText(t).width]);
  return { before, after, widest };
});
console.log(JSON.stringify(out, null, 1));
await b.close();
