import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
async function setup(page, url, is_ref) {
  await page.goto(url);
  await page.waitForFunction(() => window.__chart?.backend?.() !== undefined || document.documentElement.dataset.ready === "true");
  if (!is_ref) {
    await page.evaluate(() => {
      const c = window.__chart;
      c.apply_options({ grid: { vertLines: { style: 1, color: "#666666" }, horzLines: { style: 1, color: "#666666" } });
      c.render();
    });
  } else {
    await page.evaluate(() => {
      const c = window.__reference.chart;
      c.applyOptions({ grid: { vertLines: { style: 1, color: "#666666" }, horzLines: { style: 1, color: "#666666" } } });
    });
  }
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  return PNG.sync.read(await page.screenshot());
}
function measure(shot, y0, x0, x1) {
  for (let yy = y0 - 3; yy < y0 + 4; yy++) {
    const runs = [];
    let cur = null;
    for (let x = x0; x < x1; x++) {
      const o = (yy * shot.width + x) * 4;
      const dark = shot.data[o] < 150 && shot.data[o+1] < 150 && shot.data[o+2] < 150;
      if (dark && !cur) cur = { s: x, e: x };
      else if (dark) cur.e = x;
      else if (cur) { runs.push(cur); cur = null; }
    }
    if (runs.length > 8) {
      const widths = runs.slice(0, 14).map((r) => r.e - r.s + 1);
      const gaps = runs.slice(0, 13).map((r, i) => runs[i + 1].s - r.e - 1);
      return { yy, widths: widths.join(","), gaps: gaps.join(",") };
    }
  }
  return { yy: y0, widths: "?", gaps: "?" };
}
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
const y100 = 303, x0 = 60, x1 = 700;
console.log("aion webgpu:  ", JSON.stringify(measure(await setup(p, "http://127.0.0.1:4174/?backend=webgpu", false), y100, x0, x1)));
console.log("aion canvas2d:", JSON.stringify(measure(await setup(p, "http://127.0.0.1:4174/?backend=canvas2d", false), y100, x0, x1)));
const ref_y = await p.evaluate(() => 0);
console.log("reference:    ", JSON.stringify(measure(await setup(p, "http://127.0.0.1:4174/reference.html?runtimeTest=presentedFrame&backend=canvas2d&dpr=1&spacing=6&theme=light", true), 200, x0, x1)));
await b.close();
