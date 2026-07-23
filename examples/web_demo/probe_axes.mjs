import { chromium } from "playwright";
import { PNG } from "pngjs";
import { writeFileSync } from "node:fs";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 900, height: 720 }, deviceScaleFactor: 2 })).newPage();
await p.goto("http://127.0.0.1:4174/");
await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
await p.waitForTimeout(400);
const shot = PNG.sync.read(await p.screenshot());
writeFileSync("axis_aion.png", PNG.sync.write(shot));
// crop price axis strip + time axis strip for inspection
const info = await p.evaluate(() => {
  const c = window.__chart;
  return { pane_left: c.wasm.pane_left(), pane_w: c.wasm.time_scale_width(), pane_h: c.wasm.pane_height(0), dpr: devicePixelRatio, w: innerWidth, h: innerHeight };
});
console.log(JSON.stringify(info));
// also reference for A/B
await p.goto("http://127.0.0.1:4174/reference.html?runtimeTest=presentedFrame&backend=canvas2d&dpr=2&spacing=6&theme=light");
await p.waitForFunction(() => document.documentElement.dataset.ready === "true");
const shot2 = PNG.sync.read(await p.screenshot());
writeFileSync("axis_ref.png", PNG.sync.write(shot2));
await b.close();
