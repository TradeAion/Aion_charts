import { chromium } from "playwright";
import { PNG } from "pngjs";
const b = await chromium.launch({ channel: "chromium" });
const p = await (await b.newContext({ viewport: { width: 100, height: 60 }, deviceScaleFactor: 1 })).newPage();
const data_url = await p.evaluate(() => {
  const c = document.createElement("canvas");
  c.width = 100; c.height = 60;
  const ctx = c.getContext("2d");
  const x = 10, y = 10, w = 35, h = 17, r = 2;
  const tl = r, tr = r, br = r, bl = r;
  ctx.fillStyle = "rgb(172,60,58)";
  ctx.beginPath();
  ctx.moveTo(x + tl, y);
  ctx.lineTo(x + w - tr, y);
  if (tr > 0) ctx.quadraticCurveTo(x + w, y, x + w, y + tr);
  ctx.lineTo(x + w, y + h - br);
  if (br > 0) ctx.quadraticCurveTo(x + w, y + h, x + w - br, y + h);
  ctx.lineTo(x + bl, y + h);
  if (bl > 0) ctx.quadraticCurveTo(x, y + h, x, y + h - bl);
  ctx.lineTo(x, y + tl);
  if (tl > 0) ctx.quadraticCurveTo(x, y, x + tl, y);
  ctx.closePath();
  ctx.fill();
  return c.toDataURL("image/png");
});
const shot = PNG.sync.read(Buffer.from(data_url.split(",")[1], "base64"));
for (let dy = -1; dy < 5; dy++) {
  const row = [];
  for (let dx = -1; dx < 5; dx++) {
    const o = ((10 + dy) * shot.width + (10 + dx)) * 4;
    const a = shot.data[o+3];
    row.push(a === 255 ? "#" : a === 0 ? "." : `~(${a})`);
  }
  console.log(`y${10 + dy}: ${row.join(" ")}`);
}
await b.close();
