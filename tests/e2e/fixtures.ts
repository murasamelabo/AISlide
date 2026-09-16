import { expect } from '@playwright/test'
import type { Page } from '@playwright/test'

export async function openSample(page: Page) {
  await page.goto('/')
  await page.getByRole('button', { name: 'New report', exact: true }).click()
  await expect(page.locator('.thumbnail')).toHaveCount(12)
  const pending = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  await pending
  await expect(page.locator('.dirty-indicator')).toHaveCount(0)
}