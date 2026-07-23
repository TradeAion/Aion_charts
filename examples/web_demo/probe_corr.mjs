import { chromium } from "playwright";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext()).newPage();
const out = await p.evaluate(() => {
  const ctx = document.createElement("canvas").getContext("2d");
  const fam = "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif";
  const corr = (font, text) => {
    ctx.font = font;
    const m = ctx.measureText(text);
    return Math.round(((m.actualBoundingBoxAscent - m.actualBoundingBoxDescent) / 2) * 100) / 100;
  };
  const texts = ["100.00", "120.00", "94.74", "98.00", "Apr0", "Mar", "28,998.10", "1d 10h"];
  return {
    normal: texts.map((t) => [t, corr(`12px ${fam}`, t)]),
    bold: texts.map((t) => [t, corr(`bold 12px ${fam}`, t)]),
  };
});
console.log(JSON.stringify(out, null, 1));
await b.close();
