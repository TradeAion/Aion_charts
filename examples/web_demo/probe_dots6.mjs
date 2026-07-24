import { chromium } from "playwright";
import { PNG } from "pngjs";

const b = await chromium.launch({ channel: "chromium" });

async function setup(page, url, is_ref) {
  await page.goto(url);
  await page.waitForFunction(() => window.__chart?.backend?.() !== undefined || document.documentElement.dataset.ready === "true");
  if (!is_ref) {
    await page.evaluate(() => {
      const c = window.__chart;
      c.apply_options({ grid: { vertLines: { style: 1, color: "#333333" }, horzLines: { style: 1, color: "#333333" } } });
      c.render();
    });
    await page.waitForTimeout(150);
    const y = await page.evaluate(() => {
      const c = window.__chart;
      return Math.round(c.price_to_coordinate(100));
    });
    return { shot: PNG.sync.read(await page.screenshot()), y };
  }
  await page.evaluate(() => {
    const c = window.__reference.chart;
    c.applyOptions({ grid: { vertLines: { style: 1, color: "#333333" }, horzLines: { style: 1, color: "#333333" } } });
  });
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  const y = await page.evaluate(() => Math.round(window.__reference.series.priceToCoordinate(98)));
  return { shot: PNG.sync.read(await page.screenshot()), y };
}

function measure(shot, y0) {
  for (let yy = y0 - 3; yy < y0 + 4; yy++) {
    const runs = [];
    let cur = null;
    for (let x = 60; x < 700; x++) {
      const o = (yy * shot.width + x) * 4;
      const dark = shot.data[o] < 120 && shot.data[o + 1] < 120 && shot.data[o + 2] < 120;
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
let r = await setup(p, "http://127.0.0.1:4174/?backend=webgpu", false);
console.log("aion webgpu:  ", JSON.stringify(measure(r.shot, r.y)));
r = await setup(p, "http://127.0.0.1:4174/?backend=canvas2d", false);
console.log("aion canvas2d:", JSON.stringify(measure(r.shot, r.y)));
r = await setup(p, "http://127.0.0.1:4174/reference.html?runtimeTest=presentedFrame&backend=canvas2d&dpr=1&spacing=6&theme=light", true);
console.log("reference:    ", JSON.stringify(measure(r.shot, r.y)));
await b.close();
