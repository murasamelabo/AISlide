import { test, expect } from '@playwright/test';
import { openSample } from './fixtures';

test('sample setup completes without waiting for unrelated load assets', async ({ page }) => {
  const asset = Promise.withResolvers<void>();
  await page.route('**/__sample_fixture_pending.svg', async route => {
    await asset.promise;
    if (!page.isClosed()) {
      await route.fulfill({ contentType: 'image/svg+xml', body: '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>' });
    }
  });
  await page.route('**/', async route => {
    const response = await route.fetch();
    const html = await response.text();
    expect(html).toContain('</body>');
    await route.fulfill({
      response,
      body: html.replace('</body>', '<img src="/__sample_fixture_pending.svg" alt="" hidden></body>'),
    });
  });
  try {
    await openSample(page);
    expect(await page.evaluate(() => document.readyState)).toBe('interactive');
  } finally {
    asset.resolve();
  }
  await page.waitForLoadState('load');
  await expect(page.locator('.thumbnail')).toHaveCount(12);
  await expect(page.locator('.dirty-indicator')).toHaveCount(0);
});
