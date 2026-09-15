import { expect, test } from '@playwright/test'
import AxeBuilder from '@axe-core/playwright'

for (const width of [1440, 390]) {
  test(`DADS application typography, targets and focus preserve slide styling at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 })
    await page.goto('/')
    const save = page.getByRole('button', { name: 'Save PPTX', exact: true })
    await expect(save).toBeEnabled()
    await page.evaluate(() => document.fonts.ready)
    await expect(page.locator('body')).toHaveCSS('font-family', /Noto Sans JP/)
    await expect(save).toHaveCSS('font-size', '16px')
    const undersized = await page.locator('.app-header button, .ribbon button').evaluateAll((buttons) => buttons.filter((button) => {
      const bounds = button.getBoundingClientRect()
      return bounds.width < 44 || bounds.height < 44
    }).map((button) => button.getAttribute('aria-label')))
    expect(undersized).toEqual([])
    await save.focus()
    await expect(save).toHaveCSS('outline-color', 'rgb(0, 0, 0)')
    await expect(save).toHaveCSS('outline-width', '2px')
    await expect(save).toHaveCSS('box-shadow', /rgb\(255, 212, 61\)/)
    await page.getByRole('button', { name: /^Slide 4:/ }).click()
    await expect(page.locator('.slide-stage .slide-table th').first()).toHaveCSS('background-color', 'rgb(8, 127, 115)')
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    await page.screenshot({ path: `.artifacts/dads-studio-${width}.png`, fullPage: true })
  })

  test(`authoring dialogs fit and have labeled controls at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 })
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    await page.goto('/')
    await expect(page.getByRole('button', { name: /^Slide 12:/ })).toBeVisible()
    for (const [name, screenshot] of [['Insert objects', 'insert'], ['Edit theme', 'theme'], ['Edit masters and layouts', 'masters']]) {
      await page.getByRole('button', { name, exact: true }).click()
      const dialog = page.getByRole('dialog')
      await expect(dialog).toBeVisible()
      if (name === 'Edit masters and layouts') {
        await page.getByRole('button', { name: 'Add common text', exact: true }).click()
        await page.getByLabel('Design element text', { exact: true }).fill('Master text / Synthetic fixture')
      }
      await page.evaluate(() => document.fonts.ready)
      expect(await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth + 1)).toBe(true)
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
      const result = await new AxeBuilder({ page }).include('dialog').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()
      expect(result.violations.filter((issue) => ['serious', 'critical'].includes(issue.impact ?? ''))).toEqual([])
      await page.getByRole('heading', { name: name === 'Insert objects' ? 'Insert objects' : name === 'Edit theme' ? 'Theme' : 'Masters and layouts', exact: true }).scrollIntoViewIfNeeded()
      await page.screenshot({ path: `.artifacts/authoring-${screenshot}-${width}.png`, fullPage: true })
      await page.getByRole('button', { name: 'Close dialog', exact: true }).click()
    }
    expect(errors).toEqual([])
  })
}

test('all nine chart kinds render after insertion without runtime errors', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  await page.goto('/')
  await expect(page.getByRole('button', { name: /^Slide 12:/ })).toBeVisible()
  for (const kind of ['Column', 'Bar', 'Line', 'Pie', 'Doughnut', 'Area', 'Scatter', 'Stacked column', 'Stacked bar']) {
    await page.getByRole('button', { name: 'Insert objects', exact: true }).click()
    await page.getByRole('button', { name: `Insert ${kind} chart`, exact: true }).click()
    const chart = page.locator('.canvas-workspace .recharts-wrapper')
    await expect(chart).toBeVisible()
    expect(await chart.locator('path').count()).toBeGreaterThan(0)
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect(chart).toHaveCount(0)
  }
  expect(errors).toEqual([])
})