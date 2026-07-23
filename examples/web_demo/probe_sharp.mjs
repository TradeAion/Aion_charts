import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
for (const dpr of [1, 1.35, 2]) {
  const p = await (await b.newContext({ viewport: { width: 900, height: 600 }, deviceScaleFactor: dpr })).newPage();
  await p.goto("http://127.0.0.1:4174/");
  await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
  await p.waitForTimeout(300);
  const sharp = await p.evaluate(() => {
    const overlay = document.querySelectorAll("canvas")[3];
    const ctx = overlay.getContext("2d", { willReadFrequently: true });
    // find a label row with text; measure the alpha profile of one glyph stem
    const img = ctx.getImageData(0, 0, overlay.width, overlay.height);
    // scan the axis strip for the steepest alpha transition (text edge quality)
    let transitions = 0, samples = 0;
    for (let y = 0; y < overlay.height; y += 2) {
      for (let x = Math.floor(overlay.width * 0.96); x < overlay.width - 2; x++) {
        const a1 = img.data[(y * overlay.width + x) * 4 + 3];
        const a2 = img.data[(y * overlay.width + x + 2) * 4 + 3];
        if (a1 > 200 && a2 < 50) { transitions++; }
        if (a1 > 200) samples++;
      }
    }
    return { transitions, samples, bitmap_w: overlay.width, bitmap_h: overlay.height, expected_w: Math.round(900 * devicePixelRatio), expected_h: Math.round(600 * devicePixelRatio) };
  });
  console.log(`dpr ${dpr}:`, JSON.stringify(sharp));
  await p.close();
}
await b.close();
