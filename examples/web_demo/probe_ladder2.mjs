import { PNG } from "pngjs";
import { readFileSync } from "node:fs";
for (const name of ["axis_aion.png", "axis_ref.png"]) {
  const png = PNG.sync.read(readFileSync(name));
  // find the strip: rightmost ~116px
  const x1 = png.width, x0 = x1 - 116;
  const rows = [];
  for (let y = 0; y < png.height; y++) {
    let n = 0;
    for (let x = x0; x < x1; x++) {
      const o = (y * png.width + x) * 4;
      if (png.data[o+3] > 100 && png.data[o] < 140) n++;
    }
    rows.push(n);
  }
  const labels = [];
  let cur = null;
  for (let y = 0; y < rows.length; y++) {
    if (rows[y] > 2) {
      if (!cur) cur = { top: y, bottom: y, ink: 0 };
      cur.bottom = y; cur.ink += rows[y];
    } else if (cur) { labels.push(cur); cur = null; }
  }
  if (cur) labels.push(cur);
  const centers = labels.map((l) => (l.top + l.bottom) / 2);
  const gaps = centers.slice(1).map((c, i) => Math.round((c - centers[i]) * 10) / 10);
  console.log(name, "labels:", labels.length, "centers:", centers.slice(0, 12).join(","), "gaps:", gaps.slice(0, 11).join(","));
}
