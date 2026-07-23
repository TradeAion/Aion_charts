import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
for (const dpr of [1, 1.35, 2]) {
  const p = await (await b.newContext({ viewport: { width: 900, height: 600 }, deviceScaleFactor: dpr })).newPage();
  const out = await p.evaluate(() => new Promise((resolve) => {
    const el = document.createElement("div");
    el.style.cssText = "position:fixed;inset:0;";
    document.body.appendChild(el);
    const ro = new ResizeObserver((entries) => {
      const e = entries[0];
      resolve({
        dpr: devicePixelRatio,
        contentBox: e.contentBoxSize ? [e.contentBoxSize[0].inlineSize, e.contentBoxSize[0].blockSize] : null,
        devicePixelContentBox: e.devicePixelContentBoxSize ? [e.devicePixelContentBoxSize[0].inlineSize, e.devicePixelContentBoxSize[0].blockSize] : null,
        rect: [el.getBoundingClientRect().width, el.getBoundingClientRect().height],
      });
      ro.disconnect();
      el.remove();
    });
    try { ro.observe(el, { box: "device-pixel-content-box" }); } catch (err) { ro.observe(el); }
  }));
  console.log(`dpr ${dpr}:`, JSON.stringify(out));
  await p.close();
}
await b.close();
