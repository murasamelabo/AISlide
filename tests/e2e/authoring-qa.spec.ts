import { expect, test } from '@playwright/test'
import { openSample } from './fixtures';
import AxeBuilder from '@axe-core/playwright'

test('toolbar tooltips escape the scrolling ribbon without shifting their controls', async ({ page }) => {
  await openSample(page)
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  for (const width of [1200, 390]) {
    await page.setViewportSize({ width, height: 900 })
    await page.evaluate(() => document.fonts.ready)
    for (const name of ['Add chart', 'Architecture diagram']) {
      const button = page.getByRole('button', { name, exact: true })
      await button.scrollIntoViewIfNeeded()
      const before = await button.boundingBox()
      await button.hover()
      const tooltip = page.locator('.tool-tooltip')
      await expect(tooltip).toBeVisible()
      await expect(page.getByRole('tooltip', { name, exact: true })).toHaveText(name)
      await expect(button).not.toHaveAttribute('title')
      const unclipped = await tooltip.evaluate((element) => {
        const bounds = element.getBoundingClientRect()
        if (bounds.left < 0 || bounds.right > innerWidth || bounds.top < 0 || bounds.bottom > innerHeight) return false
        for (let parent = element.parentElement; parent; parent = parent.parentElement) {
          const style = getComputedStyle(parent)
          const box = parent.getBoundingClientRect()
          if (/(auto|scroll|hidden|clip)/.test(style.overflowX) && (bounds.left < box.left || bounds.right > box.right)) return false
          if (/(auto|scroll|hidden|clip)/.test(style.overflowY) && (bounds.top < box.top || bounds.bottom > box.bottom)) return false
        }
        return true
      })
      expect(unclipped).toBe(true)
      expect(await button.boundingBox()).toEqual(before)
      await tooltip.hover()
      await expect(tooltip).toBeVisible()
      await page.screenshot({ path: `.artifacts/ui-tooltip-${width}-${name === 'Add chart' ? 'chart' : 'graph'}.png` })
      await page.keyboard.press('Escape')
      await expect(tooltip).toHaveCount(0)
    }
  }
  await page.setViewportSize({ width: 1200, height: 900 })
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true })
  const close = dialog.getByRole('button', { name: 'Close dialog', exact: true })
  await close.focus()
  await expect(dialog.locator('.tool-tooltip')).toBeVisible()
  await expect(close).toHaveAttribute('aria-describedby', /.+/)
  await page.keyboard.press('Escape')
  await expect(dialog.locator('.tool-tooltip')).toHaveCount(0)
  await expect(dialog).toBeVisible()
  await close.click()
  await expect(dialog).toHaveCount(0)
  await page.getByRole('button', { name: 'Select title', exact: true }).click()
  const bold = page.getByRole('button', { name: 'Bold', exact: true })
  await bold.scrollIntoViewIfNeeded()
  const before = await bold.boundingBox()
  const pressed = await bold.getAttribute('aria-pressed')
  await bold.click()
  await expect(bold).toHaveAttribute('aria-pressed', String(pressed !== 'true'))
  expect(await bold.boundingBox()).toEqual(before)
  await bold.press('Tab')
  await page.keyboard.press('Shift+Tab')
  await expect(bold).toBeFocused()
  await expect(bold).toHaveCSS('outline-width', '2px')
  await expect(bold).toHaveCSS('box-shadow', /rgb\(255, 212, 61\)/)
})

for (const width of [1440, 1200, 960, 390]) {
  test(`long presentation names keep header controls in place at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 })
    await openSample(page)
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
    await page.evaluate(() => document.fonts.ready)
    const before = await page.locator('.app-header').boundingBox()
    await page.getByRole('button', { name: 'Rename presentation', exact: true }).click()
    const title = 'QuarterlyPerformanceReviewWithoutSpaces'.repeat(3)
    await page.getByRole('textbox', { name: 'Presentation title', exact: true }).fill(title)
    await page.getByRole('dialog').getByRole('button', { name: 'Apply', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(page.locator('.document-name strong')).toHaveText(title)
    const after = await page.locator('.app-header').boundingBox()
    expect(after!.height).toBe(before!.height)
    const outside = await page.locator('.app-header button').evaluateAll((buttons) => buttons.filter((button) => {
      const box = button.getBoundingClientRect()
      return box.left < 0 || box.right > innerWidth || box.width < 44 || box.height < 44
    }).map((button) => button.getAttribute('aria-label')))
    expect(outside).toEqual([])
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    await page.screenshot({ path: `.artifacts/ui-header-${width}.png`, fullPage: true })
  })
}

test('tooltips do not reappear after a background edit disables their trigger', async ({ page }) => {
  await openSample(page)
  const button = page.getByRole('button', { name: 'Add chart', exact: true })
  await expect(button).toBeEnabled()
  await button.hover()
  await expect(page.locator('.tool-tooltip')).toBeVisible()
  let release = () => {}
  let started = () => {}
  const waiting = new Promise<void>((resolve) => { release = resolve })
  const observed = new Promise<void>((resolve) => { started = resolve })
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op === 'edit_slides') { started(); await waiting }
    await route.continue()
  })
  try {
    await page.keyboard.press('Control+m')
    await observed
    await expect(button).toBeDisabled()
    await expect(page.locator('.tool-tooltip')).toHaveCount(0)
    await page.mouse.move(500, 500)
  } finally { release() }
  await expect(page.locator('.thumbnail')).toHaveCount(13)
  await expect(button).toBeEnabled()
  await expect(page.locator('.tool-tooltip')).toHaveCount(0)
})

test('toolbar commands have distinct icons and inspector state stays visible', async ({ page }) => {
  await openSample(page)
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  const icons = await Promise.all(['Insert objects', 'Insert icons', 'Architecture diagram', 'Add process diagram'].map((name) => page.getByRole('button', { name, exact: true }).locator('svg').getAttribute('class')))
  expect(new Set(icons).size).toBe(4)
  const toggle = page.getByRole('button', { name: 'Toggle inspector', exact: true })
  await expect(toggle).toHaveAttribute('aria-pressed', 'true')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-pressed', 'false')
  await expect(page.getByRole('complementary', { name: 'Inspector', exact: true })).toHaveCount(0)
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('complementary', { name: 'Inspector', exact: true })).toBeVisible()
})

for (const width of [1440, 390]) {
  test(`DADS application typography, targets and focus preserve slide styling at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 })
    await openSample(page)
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
    await save.press('Tab')
    await page.keyboard.press('Shift+Tab')
    await expect(save).toBeFocused()
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
    await openSample(page)
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
      const inconsistentTools = await dialog.locator('button.tool').evaluateAll((buttons) => buttons.filter((button) => {
        const bounds = button.getBoundingClientRect()
        return Math.abs(bounds.width - 44) > 1 || Math.abs(bounds.height - 44) > 1
      }).map((button) => button.getAttribute('aria-label')))
      expect(inconsistentTools).toEqual([])
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
  await openSample(page)
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