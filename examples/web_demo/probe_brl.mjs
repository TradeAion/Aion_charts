import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1.5 })).newPage();
await p.goto("http://127.0.0.1:4174/?runtimeTest=presentedFrame&backend=canvas2d&dpr=1.5&spacing=50&theme=light");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
const out = await p.evaluate(() => {
  const o = window.__chart.price_scale("right").options();
  return { bold_round_labels: o.bold_round_labels, mode: o.mode };
});
console.log(JSON.stringify(out));
await b.close();
