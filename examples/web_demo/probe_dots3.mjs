import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
async function measure(page, url) {
  await page.goto(url);
  await page.waitForFunction(() => window.__chart?.backend?.() !== undefined || document.documentElement.dataset.ready === "true");
  await page.waitForTimeout(300);
  const shot = PNG.sync.read(await page.screenshot());
  const y = Math.round(shot.height / 2);
  // find a horizontal grid row: scan several rows, pick the one with periodic dark runs
  for (let yy = 60; yy < shot.height - 60; yy++) {
    const runs = [];
    let cur = null;
    for (let x = 60; x < 700; x++) {
      const o = (yy * shot.width + x) * 4;
      const dark = shot.data[o] < 210 && shot.data[o+1] < 210 && shot.data[o+2] < 210;
      if (dark && !cur) cur = { s: x, e: x };
      else if (dark) cur.e = x;
      else if (cur) { runs.push(cur); cur = null; }
    }
    if (runs.length > 10) {
      const widths = runs.slice(0, 12).map((r) => r.e - r.s + 1);
      const gaps = runs.slice(0, 11).map((r, i) => runs[i + 1].s - r.e - 1);
      return { yy, widths: widths.join(","), gaps: gaps.join(",") };
    }
  }
  return null;
}
const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })).newPage();
console.log("aion webgpu:", JSON.stringify(await measure(p, "http://127.0.0.1:4174/?backend=webgpu&gridstyle=dotted")));
console.log("aion canvas2d:", JSON.stringify(await measure(p, "http://127.0.0.1:4174/?backend=canvas2d")));
console.log("reference:", JSON.stringify(await measure(p, "http://127.0.0.1:4174/reference.html?runtimeTest=presentedFrame&backend=canvas2d&dpr=1&spacing=6&theme=light")));
await b.close();
