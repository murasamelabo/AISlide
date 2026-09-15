import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Slide 12:', exact: false })).toBeVisible()
})

test('inserts a native preset with directly editable text and undo', async ({ page }) => {
  await page.getByRole('button', { name: 'Insert objects', exact: true }).click()
  await page.getByRole('button', { name: 'Insert Ellipse', exact: true }).click()
  const shape = page.locator('.canvas-workspace .element-hitbox[aria-label^="Edit shape-"]')
  await expect(shape).toHaveCount(1)
  await shape.dblclick()
  await page.getByRole('textbox', { name: 'Slide text editor', exact: true }).fill('Editable native ellipse')
  await page.getByRole('button', { name: 'Apply on-slide edit', exact: true }).click()
  await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Editable native ellipse' })).toBeVisible()
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Editable native ellipse' })).toHaveCount(0)
})

test('theme colors are live references and theme edits can be undone', async ({ page }) => {
  await page.getByRole('button', { name: 'Edit theme', exact: true }).click()
  await page.getByLabel('Theme name', { exact: true }).fill('Editorial theme')
  await page.getByLabel('Theme accent1', { exact: true }).fill('#b53055')
  await page.getByLabel('Heading font', { exact: true }).fill('Arial')
  await page.getByRole('button', { name: 'Apply theme', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect.poll(() => page.locator('.canvas-workspace .slide-page').evaluate((node) => Array.from(node.querySelectorAll('*')).some((child) => getComputedStyle(child).backgroundColor === 'rgb(181, 48, 85)'))).toBe(true)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect.poll(() => page.locator('.canvas-workspace .slide-page').evaluate((node) => Array.from(node.querySelectorAll('*')).some((child) => getComputedStyle(child).backgroundColor === 'rgb(181, 48, 85)'))).toBe(false)
})

test('master text propagates and a layout preserves the slide title', async ({ page }) => {
  await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click()
  await page.getByRole('button', { name: 'Add common text', exact: true }).click()
  await page.getByLabel('Design element text', { exact: true }).fill('Company master footer')
  await page.getByRole('button', { name: 'Save design', exact: true }).click()
  await expect(page.locator('.canvas-workspace .master-graphics').getByText('Company master footer', { exact: true })).toBeVisible()
  await page.getByLabel('Slide layout', { exact: true }).selectOption('title-content')
  await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Quarterly performance' })).toBeVisible()
  await expect(page.locator('.canvas-workspace .element-hitbox[aria-label="Edit body"]')).toHaveCount(1)
  await page.getByRole('button', { name: 'Slide 2:', exact: false }).click()
  await expect(page.locator('.canvas-workspace .master-graphics').getByText('Company master footer', { exact: true })).toBeVisible()
})