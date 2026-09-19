import { expect } from '@playwright/test'
import type { Page } from '@playwright/test'

export async function waitForCoreOperation(page: Page, operation: string) {
  const response = await page.waitForResponse(response => response.url().endsWith('/api/core')
    && response.request().method() === 'POST' && response.request().postDataJSON().op === operation)
  expect(response.ok()).toBe(true)
  expect(await response.finished()).toBeNull()
}

export async function openSample(page: Page) {
  await page.goto('/')
  const created = waitForCoreOperation(page, 'new_document')
  await page.getByRole('button', { name: 'New report', exact: true }).click()
  await created
  await expect(page.locator('.thumbnail')).toHaveCount(12)
  const pending = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  await pending
  await expect(page.locator('.dirty-indicator')).toHaveCount(0)
}