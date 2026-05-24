import { expect, test } from '@playwright/test';

test('demo initializes the chart without a host integration error', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') errors.push(message.text());
  });

  await page.goto('/demo/index.html?renderer=canvas2d');

  await expect(page.locator('#loading')).toBeHidden();
  await expect(page.locator('#error-banner')).toBeHidden();
  await expect(page.locator('#chart-container canvas').first()).toBeVisible();
  await expect(page.locator('#hud-status')).toContainText(/bars/);
  expect(errors).toEqual([]);
});

test('demo uses the canvas-native editor for drawing text insertion', async ({ page }) => {
  await page.route('**/demo/index.html?*', async route => {
    const response = await route.fetch();
    let body = await response.text();
    body = body.replace(
      'core = await Aion_charts.create_chart(hostId, {',
      'core = window.__demoCore = await Aion_charts.create_chart(hostId, {',
    );
    await route.fulfill({ response, body });
  });

  await page.goto('/demo/index.html?renderer=canvas2d');
  await expect(page.locator('#loading')).toBeHidden();

  await page.locator('#draw-text').click();
  const box = await page.locator('#chart-container').boundingBox();
  if (!box) throw new Error('chart container missing');
  await page.mouse.click(box.x + box.width * 0.45, box.y + box.height * 0.42);

  const editor = page.locator('#drawing-text-editor');
  await expect(editor).toBeHidden();

  await page.keyboard.type('Native label');
  await expect(editor).toBeHidden();
  await expect.poll(() =>
    page.evaluate(() => {
      const core = (window as any).__demoCore;
      return JSON.parse(core.get_selected_drawing_info_json());
    }),
  ).toMatchObject({ text: 'Native label', text_editing: true });

  await page.keyboard.press('Enter');
  await expect(editor).toBeHidden();
  await expect.poll(() =>
    page.evaluate(() => {
      const core = (window as any).__demoCore;
      return JSON.parse(core.get_selected_drawing_info_json());
    }),
  ).toMatchObject({ text: 'Native label', text_editing: false });
});
