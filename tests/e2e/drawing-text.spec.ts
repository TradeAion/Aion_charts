import { expect, test } from '@playwright/test';
import {
  canvasLocator,
  commitCanvasFrame,
  importTextDrawing,
  installHarness,
  readSelectedDrawingInfo,
  startTextEdit,
  textPixelBounds,
} from './axius-harness';

test.beforeEach(async ({ page }) => {
  await installHarness(page);
});

test('text drawing edits through the native canvas key path and commits with Enter', async ({ page }) => {
  await importTextDrawing(page);
  await startTextEdit(page);

  await page.keyboard.type('Native text');

  await expect.poll(() => readSelectedDrawingInfo(page)).toMatchObject({
    supports_text: true,
    text_editing: true,
    text: 'Native text',
  });

  await expect(page.locator('#drawing-text-editor')).toBeHidden();
  await expect(page.locator('textarea:visible, input:visible')).toHaveCount(0);

  await page.keyboard.press('Enter');

  await expect.poll(() => readSelectedDrawingInfo(page)).toMatchObject({
    text_editing: false,
    text: 'Native text',
  });
});

test('committed and active edit text use the same canvas text placement', async ({ page }, testInfo) => {
  await importTextDrawing(page, 'Stable label');
  await startTextEdit(page);

  await page.evaluate(() => {
    const { chart } = (window as any).__axiusHarness;
    chart.tick_drawing_caret_blink(0);
    chart.tick_drawing_caret_blink(600);
    chart.render();
  });
  const activeScreenshot = await (await canvasLocator(page)).screenshot();
  const activeBounds = await textPixelBounds(page);
  await testInfo.attach('drawing-text-active.png', { body: activeScreenshot, contentType: 'image/png' });

  await page.keyboard.press('Enter');
  await page.keyboard.press('Escape');
  await commitCanvasFrame(page);
  const committedScreenshot = await (await canvasLocator(page)).screenshot();
  const committedBounds = await textPixelBounds(page);
  await testInfo.attach('drawing-text-committed.png', { body: committedScreenshot, contentType: 'image/png' });

  expect(activeBounds).not.toBeNull();
  expect(committedBounds).not.toBeNull();
  expect(Math.abs(activeBounds!.x - committedBounds!.x)).toBeLessThanOrEqual(1);
  expect(Math.abs(activeBounds!.y - committedBounds!.y)).toBeLessThanOrEqual(1);
  expect(Math.abs(activeBounds!.width - committedBounds!.width)).toBeLessThanOrEqual(1);
  expect(Math.abs(activeBounds!.height - committedBounds!.height)).toBeLessThanOrEqual(1);
});

test('Escape cancels active text edits without changing committed text', async ({ page }) => {
  await importTextDrawing(page, 'Keep me');
  await startTextEdit(page);
  await page.keyboard.type(' changed');

  await expect.poll(() => readSelectedDrawingInfo(page)).toMatchObject({
    text_editing: true,
    text: 'Keep me changed',
  });

  await page.keyboard.press('Escape');

  await expect.poll(() => readSelectedDrawingInfo(page)).toMatchObject({
    text_editing: false,
    text: 'Keep me',
  });
});
