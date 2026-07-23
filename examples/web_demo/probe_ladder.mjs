import { PNG } from "pngjs";
import { readFileSync } from "node:fs";
const png = PNG.sync.read(readFileSync("axis_aion.png"));
const x0 = 1684, x1 = 1800;
// ink rows in the price strip
const rows = [];
for (let y = 0; y < png.height; y++) {
  let n = 0;
  for (let x = x0; x < x1; x++) {
    const o = (y * png.width + x) * 4;
    if (png.data[o+3] > 100 && png.data[o] < 140) n++;
  }
  rows.push(n);
}
// cluster into labels
const labels = [];
let cur = null;
for (let y = 0; y < rows.length; y++) {
  if (rows[y] > 2) {
    if (!cur) cur = { top: y, bottom: y, ink: 0 };
    cur.bottom = y; cur.ink += rows[y];
  } else if (cur) { labels.push(cur); cur = null; }
}
if (cur) labels.push(cur);
// grid lines in the pane (light #f5f5f5 rows): find horizontal grid line ys from the pane area
const grid = [];
for (let y = 0; y < png.height; y++) {
  let n = 0;
  for (let x = 100; x < 1600; x += 4) {
    const o = (y * png.width + x) * 4;
    const r = png.data[o], g = png.data[o+1], b = png.data[o+2];
    if (r > 235 && r < 250 && g > 235 && g < 250 && b > 235 && b < 250) n++;
  }
  if (n > 250) grid.push(y);
}
const out = labels.map((l) => {
  const center = (l.top + l.bottom) / 2;
  // nearest grid line
  let best = null, bd = 1e9;
  for (const gy of grid) { const d = Math.abs(gy - center); if (d < bd) { bd = d; best = gy; } }
  return { top: l.top, bottom: l.bottom, center, ink: l.ink, nearest_grid: best, offset: best === null ? null : center - best };
});
console.log(JSON.stringify(out.slice(0, 18), null, 1));
