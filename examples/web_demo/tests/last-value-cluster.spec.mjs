import { test, expect } from "@playwright/test";
import { PNG } from "pngjs";

// TradingView-style last-value cluster: title chip (darker shade) + price text + candle-close
// countdown row, in one connected box with axis-facing corner radius. These specs drive the
// live demo page (hourly bars ending at the current hour) through the public API only.

const LABEL = [239, 83, 80]; // #ef5350 — the deterministic final DOWN bar's label color
const CHIP = [172, 60, 58]; // LABEL darkened by 0.72 (the title chip shade)
const ROW = 17; // 12px font + 2*2.5 padding

const test_port = Number.parseInt(process.env.AION_TEST_PORT ?? "4174", 10);
const test_base_url = `http://127.0.0.1:${test_port}`;

async function wait_for_chart(page) {
  await page.waitForFunction(() => window.__chart?.backend?.() !== undefined);
  await page.evaluate(() => new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
  }));
}

// Pixel probes run at dpr 1 (2px radius = 2 bitmap px; CSS px = bitmap px).
async function open_cluster_page(browser, options) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
    colorScheme: "light",
  });
  const page = await context.newPage();
  await page.goto(`${test_base_url}/`);
  await wait_for_chart(page);
  await page.evaluate((opts) => {
    // A deterministic final DOWN bar at the current second: the label color is pinned to
    // #ef5350 and the countdown always has ~1h left (no hour-boundary flake). The bar is a
    // real `update` through the public series API.
    const now = Math.floor(Date.now() / 1000);
    const last = window.__data[window.__data.length - 1];
    const close = last.close - 2;
    window.__cluster_close = close;
    window.__main.update({ time: now, open: last.close, high: last.close + 0.6, low: close - 0.6, close });
    window.__main.apply_options(opts);
  }, options);
  await page.evaluate(() => new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
  }));
  return { context, page };
}

async function capture(page) {
  const data_url = await page.evaluate(() => window.__chart.take_screenshot().toDataURL("image/png"));
  return PNG.sync.read(Buffer.from(data_url.split(",")[1], "base64"));
}

async function cluster_anchor(page) {
  return page.evaluate(() => ({
    pane_w: window.__chart.time_scale().width(),
    y: window.__main.price_to_coordinate(window.__cluster_close),
  }));
}

function px(png, x, y) {
  const o = (y * png.width + x) * 4;
  return [png.data[o], png.data[o + 1], png.data[o + 2]];
}

function dist(a, b) {
  return Math.max(Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1]), Math.abs(a[2] - b[2]));
}

function near(a, b, tol = 12) {
  return dist(a, b) <= tol;
}

const is_box = (c) => near(c, LABEL) || near(c, CHIP);

// Locate the cluster's painted bounding box: column pane_w+2 (inside the chip, left of its
// centered text) brackets the vertical extent; the right edge is the widest box-colored run
// across the rows (rounded corners only shrink the outer 2 rows).
function find_cluster(png, pane_w) {
  let top = -1;
  let bottom = -1;
  for (let y = 0; y < png.height; y += 1) {
    if (is_box(px(png, pane_w + 2, y))) {
      if (top === -1) top = y;
      bottom = y;
    }
  }
  let right = -1;
  for (let y = top; y <= bottom; y += 1) {
    for (let x = png.width - 1; x >= pane_w; x -= 1) {
      if (is_box(px(png, x, y))) {
        right = Math.max(right, x + 1);
        break;
      }
    }
  }
  return { left: pane_w, top, right, bottom: bottom + 1 };
}

function count_where(png, box, predicate) {
  let n = 0;
  for (let y = box.top; y < box.bottom; y += 1) {
    for (let x = box.left; x < box.right; x += 1) {
      if (predicate(px(png, x, y))) n += 1;
    }
  }
  return n;
}

const count_color = (png, box, color) => count_where(png, box, (c) => near(c, color));
const is_white = (c) => c[0] > 240 && c[1] > 240 && c[2] > 240;

function region_diff(a, b, box) {
  let diff = 0;
  for (let y = box.top; y < box.bottom; y += 1) {
    for (let x = box.left; x < box.right; x += 1) {
      if (dist(px(a, x, y), px(b, x, y)) > 10) diff += 1;
    }
  }
  return diff;
}

test("countdown_timer_needed gates on visibility and data (pure timer logic)", async () => {
  const { countdown_timer_needed } = await import("../dist/aion_charts.js");
  expect(countdown_timer_needed([])).toBe(false);
  expect(countdown_timer_needed([{ countdown_visible: true, has_data: true }])).toBe(true);
  expect(countdown_timer_needed([{ countdown_visible: true, has_data: false }])).toBe(false);
  expect(countdown_timer_needed([{ countdown_visible: false, has_data: true }])).toBe(false);
  expect(countdown_timer_needed([{ has_data: true }, { countdown_visible: true, has_data: true }])).toBe(true);
});

test("last-value cluster paints chip, price, and countdown rows; chip is visibly darker", async ({ browser }) => {
  const { context, page } = await open_cluster_page(browser, {
    title: "AION",
    title_visible: true,
    countdown_visible: true,
  });
  const anchor = await cluster_anchor(page);
  const on = await capture(page);
  const box = find_cluster(on, anchor.pane_w);
  expect(box.top, "cluster box should be located").toBeGreaterThanOrEqual(0);
  // One connected two-row box (~34px at the default 12px font).
  expect(box.bottom - box.top).toBeGreaterThanOrEqual(ROW * 2 - 4);
  expect(box.bottom - box.top).toBeLessThanOrEqual(ROW * 2 + 4);
  // The title chip (left end of the top row) is the darker shade of the label color.
  const chip_pixel = px(on, box.left + 3, box.top + Math.floor(ROW / 2));
  const price_pixel = px(on, box.right - 4, box.top + Math.floor(ROW / 2));
  expect(near(chip_pixel, CHIP), `chip pixel ${chip_pixel}`).toBe(true);
  expect(near(price_pixel, LABEL), `price pixel ${price_pixel}`).toBe(true);
  const luminance = (c) => 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
  expect(luminance(chip_pixel)).toBeLessThan(luminance(price_pixel) - 30);
  // The countdown row sits below the top row, in the main label color, spanning the full width.
  expect(near(px(on, box.left + 3, box.bottom - 3), LABEL)).toBe(true);
  expect(near(px(on, box.right - 4, box.bottom - 3), LABEL)).toBe(true);

  // Disabled (title chip + countdown off, price label on): the cluster rows vanish (diff > 0
  // in the cluster region) and the remaining plain label is a single row.
  await page.evaluate(() => window.__main.apply_options({ title_visible: false, countdown_visible: false }));
  const off = await capture(page);
  expect(region_diff(on, off, box)).toBeGreaterThan(0);
  const plain = find_cluster(off, anchor.pane_w);
  expect(plain.top).toBeGreaterThanOrEqual(0);
  expect(plain.bottom - plain.top).toBeLessThanOrEqual(ROW + 3);
  await context.close();
});

test("cluster parts toggle independently", async ({ browser }) => {
  const { context, page } = await open_cluster_page(browser, {
    title: "AION",
    title_visible: true,
    countdown_visible: true,
  });
  const anchor = await cluster_anchor(page);

  // Title chip off: no chip-colored pixels anywhere, price + countdown rows remain.
  await page.evaluate(() => window.__main.apply_options({ title_visible: false }));
  let shot = await capture(page);
  let box = find_cluster(shot, anchor.pane_w);
  expect(box.bottom - box.top).toBeGreaterThanOrEqual(ROW * 2 - 4);
  expect(count_color(shot, box, CHIP)).toBe(0);
  expect(count_color(shot, box, LABEL)).toBeGreaterThan(100);
  // Price text (white glyphs on the box) is still painted in the top row.
  expect(count_where(shot, { ...box, bottom: box.top + ROW }, is_white)).toBeGreaterThan(5);

  // Price off (chip + countdown on): the chip returns, the top row's price area carries no
  // text, and the countdown row keeps its text.
  await page.evaluate(() => window.__main.apply_options({ title_visible: true, last_value_visible: false }));
  shot = await capture(page);
  box = find_cluster(shot, anchor.pane_w);
  expect(box.bottom - box.top).toBeGreaterThanOrEqual(ROW * 2 - 4);
  expect(count_color(shot, box, CHIP)).toBeGreaterThan(20);
  // Chip end = the chip→label color transition on a row above the text.
  let chip_end = -1;
  for (let x = box.left + 1; x < box.right; x += 1) {
    if (near(px(shot, x, box.top + 1), LABEL)) { chip_end = x; break; }
  }
  expect(chip_end, "chip/price-area boundary").toBeGreaterThan(box.left);
  expect(count_where(shot, { left: chip_end, top: box.top, right: box.right, bottom: box.top + ROW }, is_white)).toBe(0);
  expect(count_where(shot, { ...box, top: box.top + ROW }, is_white)).toBeGreaterThan(5);

  // Countdown off (chip + price on): one row, nothing painted below it, price text present.
  await page.evaluate(() => window.__main.apply_options({ last_value_visible: true, countdown_visible: false }));
  shot = await capture(page);
  box = find_cluster(shot, anchor.pane_w);
  expect(box.bottom - box.top).toBeLessThanOrEqual(ROW + 3);
  expect(count_color(shot, box, CHIP)).toBeGreaterThan(20);
  expect(count_where(shot, { ...box, bottom: box.top + ROW }, is_white)).toBeGreaterThan(5);

  // Everything off: no cluster at all.
  await page.evaluate(() => window.__main.apply_options({ last_value_visible: false, title_visible: false }));
  shot = await capture(page);
  expect(find_cluster(shot, anchor.pane_w).top).toBe(-1);
  await context.close();
});

test("countdown row ticks with the 1s interval timer", async ({ browser }) => {
  const { context, page } = await open_cluster_page(browser, {
    title: "AION",
    title_visible: true,
    countdown_visible: true,
  });
  const anchor = await cluster_anchor(page);
  // Let the first tick settle (the timer starts on apply; the first capture must be past the
  // initial pin so both captures read distinct remaining seconds).
  await page.waitForTimeout(1100);
  const first = await capture(page);
  const box = find_cluster(first, anchor.pane_w);
  expect(box.bottom - box.top).toBeGreaterThanOrEqual(ROW * 2 - 4);
  await page.waitForTimeout(1300);
  const second = await capture(page);
  const countdown_row = { left: box.left, right: box.right, top: box.bottom - ROW, bottom: box.bottom };
  expect(region_diff(first, second, countdown_row)).toBeGreaterThan(0);
  await context.close();
});

test("cluster rounds its axis-facing corners and keeps the chart-facing side sharp", async ({ browser }) => {
  const { context, page } = await open_cluster_page(browser, {
    title: "AION",
    title_visible: true,
    countdown_visible: true,
  });
  const anchor = await cluster_anchor(page);
  const shot = await capture(page);
  const box = find_cluster(shot, anchor.pane_w);
  expect(box.top).toBeGreaterThanOrEqual(0);
  const strip_bg = px(shot, box.right + 3, box.top + 3);

  // Axis-facing top-right corner (2px radius): the extreme corner pixel is clipped (blended
  // toward the strip background), while the same column a few px down is fully filled.
  const corner_tr = px(shot, box.right - 1, box.top);
  expect(dist(corner_tr, LABEL)).toBeGreaterThan(40);
  expect(dist(corner_tr, strip_bg)).toBeLessThan(dist(corner_tr, LABEL));
  expect(near(px(shot, box.right - 1, box.top + 3), LABEL)).toBe(true);
  // Chart-facing top-left corner (the chip's): sharp, fully filled.
  expect(near(px(shot, box.left, box.top), CHIP)).toBe(true);
  // Axis-facing bottom-right corner of the countdown row: clipped the same way.
  const corner_br = px(shot, box.right - 1, box.bottom - 1);
  expect(dist(corner_br, LABEL)).toBeGreaterThan(40);
  expect(near(px(shot, box.right - 1, box.bottom - 4), LABEL)).toBe(true);
  // Chart-facing bottom-left corner: sharp.
  expect(near(px(shot, box.left, box.bottom - 1), LABEL)).toBe(true);
  await context.close();
});
