import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
for (const dpr of [1, 2]) {
  const p = await (await b.newContext({ viewport: { width: 900, height: 600 }, deviceScaleFactor: dpr })).newPage();
  await p.goto("http://127.0.0.1:4174/");
  await p.waitForFunction(() => window.__chart?.backend?.() !== undefined);
  await p.waitForTimeout(300);
  const sizes = await p.evaluate(() => [...document.querySelectorAll("canvas")].map((c) => ({
    w: c.width, h: c.height, css_w: Math.round(c.getBoundingClientRect().width), css_h: Math.round(c.getBoundingClientRect().height), dpr: devicePixelRatio,
  })));
  console.log(`dpr ${dpr}:`, JSON.stringify(sizes));
  await p.close();
}
await b.close();
