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
