import { expect, test } from '@playwright/test'
import { openSample, waitForCoreOperation } from './fixtures';
import AxeBuilder from '@axe-core/playwright'
import { readFile } from 'node:fs/promises'

for (const viewport of [{ width: 1440, height: 900 }, { width: 1200, height: 768 }, { width: 960, height: 700 }, { width: 390, height: 844 }]) {
  test(`canvas fit follows page aspect, inspector and resize at ${viewport.width}px`, async ({ page }, testInfo) => {
    test.setTimeout(90_000)
    await page.setViewportSize(viewport)
    await page.goto('/')
    await page.getByRole('button', { name: 'Add rectangle', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
    await page.evaluate(() => document.fonts.ready)
    const toggle = page.getByRole('button', { name: 'Toggle inspector', exact: true })
    const zoom = page.getByRole('combobox', { name: 'Zoom', exact: true })
    for (const [preset, width, height] of [['16:9', 1280, 720], ['4:3', 960, 720], ['portrait', 720, 1280], ['custom', 720, 720]] as const) {
      await page.getByRole('button', { name: 'Document setup', exact: true }).click()
      const dialog = page.getByRole('dialog', { name: 'Document setup', exact: true })
      await dialog.getByRole('combobox', { name: 'Preset', exact: true }).selectOption(preset, { timeout: 5000 })
      if (preset === 'custom') {
        await dialog.getByLabel('Page width', { exact: true }).fill(String(width))
        await dialog.getByLabel('Page height', { exact: true }).fill(String(height))
      }
      const resized = waitForCoreOperation(page, 'resize_canvas')
      await dialog.getByRole('button', { name: 'Apply page size', exact: true }).click()
      await resized
      await dialog.getByRole('button', { name: 'Close dialog', exact: true }).click()
      for (const hidden of [false, true]) {
        await expect(toggle).toHaveAttribute('aria-pressed', String(!hidden))
        await page.locator('.canvas-workspace').scrollIntoViewIfNeeded()
        await expect(zoom).toHaveValue('fit')
        await expect.poll(() => page.locator('.slide-stage').evaluate((stage, aspect) => {
          const slide = stage.getBoundingClientRect()
          const viewport = stage.closest('.canvas-scroll')!.getBoundingClientRect()
          const heading = document.querySelector('.canvas-heading')!.getBoundingClientRect()
          const footer = document.querySelector('.canvas-footer')!.getBoundingClientRect()
          const notes = document.querySelector('.notes-preview')!.getBoundingClientRect()
          return slide.width > 0 && slide.height > 0 && Math.abs(slide.width / slide.height - aspect) < 0.01 && slide.left >= viewport.left && slide.right <= viewport.right + 1 && slide.top >= Math.max(viewport.top, heading.bottom, 0) && slide.bottom <= Math.min(viewport.bottom + 1, footer.top, innerHeight) && footer.bottom <= notes.top + 1 && notes.bottom <= innerHeight + 1 && document.documentElement.scrollWidth <= innerWidth
        }, width / height)).toBe(true)
        await expect(page.locator('.slide-stage .slide-page')).toHaveCSS('width', `${width}px`)
        await expect(page.locator('.slide-stage .slide-page')).toHaveCSS('height', `${height}px`)
        await page.screenshot({ path: testInfo.outputPath(`fit-${viewport.width}-${preset.replace(':', '-')}-${hidden ? 'hidden' : 'shown'}.png`) })
        await toggle.click()
      }
    }
    await page.setViewportSize({ width: viewport.width + 30, height: viewport.height + 80 })
    await expect(zoom).toHaveValue('fit')
    await expect.poll(() => page.locator('.canvas-scroll').evaluate((node) => node.scrollWidth <= node.clientWidth + 1 && node.scrollHeight <= node.clientHeight + 1)).toBe(true)
    const pending = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
    const buffer = await readFile((await (await pending).path())!)
    await expect(page.getByRole('button', { name: 'Open PPTX', exact: true })).toBeEnabled()
    await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'square-fit.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer })
    await expect(page.locator('.document-name')).toContainText('square-fit.pptx')
    await expect(page.locator('.slide-stage .slide-page')).toHaveCSS('width', '720px')
    await expect(page.locator('.slide-stage .slide-page')).toHaveCSS('height', '720px')
    await expect(zoom).toHaveValue('fit')
  })
}

test('canvas wheel zoom is bounded, cursor anchored and independent of document history', async ({ page }, testInfo) => {
  test.setTimeout(90_000)
  await page.setViewportSize({ width: 1200, height: 768 })
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await expect(page.locator('.status-bar')).toContainText('1280 x 720')
  await openSample(page)
  async function savedBytes() {
    const pending = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
    return readFile((await (await pending).path())!)
  }
  const original = await savedBytes()
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Select title', exact: true }).click()
  const title = page.locator('.slide-stage [data-element-id="title"]')
  const geometry = await title.getAttribute('style')
  const operations: string[] = []
  page.on('request', (request) => { if (request.url().endsWith('/api/core') && request.method() === 'POST') operations.push(request.postDataJSON().op) })
  const zoom = page.getByRole('combobox', { name: 'Zoom', exact: true })
  const canvas = page.locator('.canvas-scroll')
  await canvas.hover()
  await page.mouse.wheel(0, -120)
  await expect(zoom).not.toHaveValue('fit')
  const firstZoom = Number(await zoom.inputValue())
  await page.mouse.wheel(0, 120)
  await expect.poll(async () => Number(await zoom.inputValue())).toBeLessThan(firstZoom)
  await zoom.selectOption('1')
  await page.getByRole('button', { name: 'Toggle inspector', exact: true }).click()
  await expect(zoom).toHaveValue('1')
  const viewport = (await canvas.boundingBox())!
  const cursor = { x: viewport.x + viewport.width / 2, y: viewport.y + viewport.height / 2 }
  const anchor = () => page.locator('.slide-stage').evaluate((stage, cursor) => {
    const box = stage.getBoundingClientRect()
    return { x: (cursor.x - box.left) / box.width, y: (cursor.y - box.top) / box.height, aspect: box.width / box.height, browserScale: visualViewport?.scale, pixelRatio: devicePixelRatio, width: innerWidth }
  }, cursor)
  await page.mouse.move(cursor.x, cursor.y)
  const before = await anchor()
  await page.keyboard.down('Control')
  await page.mouse.wheel(0, -120)
  await page.keyboard.up('Control')
  await expect.poll(async () => Number(await zoom.inputValue())).toBeGreaterThan(1)
  const after = await anchor()
  expect(after.x).toBeCloseTo(before.x, 2)
  expect(after.y).toBeCloseTo(before.y, 2)
  expect(after.aspect).toBeCloseTo(16 / 9, 2)
  expect([after.browserScale, after.pixelRatio, after.width]).toEqual([before.browserScale, before.pixelRatio, before.width])
  for (const [delta, limit] of [[-10000, '4'], [10000, '0.1']] as const) {
    for (let count = 0; count < 5; count++) {
      const previous = await zoom.inputValue()
      await page.mouse.wheel(0, delta)
      if (previous !== limit) await expect(zoom).not.toHaveValue(previous)
    }
    await expect(zoom).toHaveValue(limit)
  }
  for (let sample = 0; sample < 5; sample++) {
    const previous = Number(await zoom.inputValue())
    await page.mouse.wheel(0, -10)
    await expect.poll(async () => Number(await zoom.inputValue())).toBeGreaterThan(previous)
  }
  expect(Number(await zoom.inputValue())).toBeGreaterThan(0.109)
  const preserved = await zoom.inputValue()
  await page.keyboard.down('Shift')
  await page.mouse.wheel(0, -120)
  await page.keyboard.up('Shift')
  await page.mouse.wheel(120, 0)
  await page.locator('.canvas-heading').hover()
  await page.mouse.wheel(0, -120)
  await expect(zoom).toHaveValue(preserved)
  await zoom.focus()
  await zoom.press('Home')
  await zoom.press('Enter')
  await expect(zoom).toHaveValue('fit')
  await expect.poll(() => canvas.evaluate((node) => [node.scrollLeft, node.scrollTop])).toEqual([0, 0])
  await page.getByRole('button', { name: 'Edit title', exact: true }).hover()
  await page.mouse.down()
  await page.mouse.wheel(0, -120)
  await page.mouse.up()
  await expect(zoom).toHaveValue('fit')
  await expect(title).toHaveClass(/selected/)
  expect(await title.getAttribute('style')).toBe(geometry)
  await page.getByRole('button', { name: 'Edit title', exact: true }).dblclick()
  const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true })
  await editor.hover()
  await page.mouse.wheel(0, -120)
  await expect(zoom).toHaveValue('fit')
  await editor.press('Escape')
  await expect(editor).toHaveCount(0)
  await expect(page.locator('.dirty-indicator')).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled()
  expect(operations).toEqual([])
  expect(await savedBytes()).toEqual(original)
  await page.screenshot({ path: testInfo.outputPath('wheel-fit-restored-1200x768.png') })
})

test('canvas fit keeps all slide corners visible with the inspector hidden', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1200, height: 768 })
  await openSample(page)
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await page.evaluate(() => document.fonts.ready)
  await page.getByRole('button', { name: 'Toggle inspector', exact: true }).click()
  await page.screenshot({ path: testInfo.outputPath('inspector-hidden-1200x768.png') })
  const bounds = () => page.locator('.slide-stage').evaluate((stage) => {
    const slide = stage.getBoundingClientRect()
    const viewport = stage.closest('.canvas-scroll')!.getBoundingClientRect()
    const footer = document.querySelector('.canvas-footer')!.getBoundingClientRect()
    const notes = document.querySelector('.notes-preview')!.getBoundingClientRect()
    return { width: slide.width, height: slide.height, left: slide.left, right: slide.right, top: slide.top, bottom: slide.bottom, viewport: { left: viewport.left, right: viewport.right, top: viewport.top, bottom: viewport.bottom }, footerTop: footer.top, notesBottom: notes.bottom, windowHeight: innerHeight }
  })
  console.log('CANVAS_FIT_BOUNDS', await bounds())
  await expect.poll(async () => {
    const box = await bounds()
    return box.left >= box.viewport.left && box.right <= box.viewport.right + 1 && box.top >= box.viewport.top && box.bottom <= box.viewport.bottom + 1 && box.bottom <= box.footerTop && box.notesBottom <= box.windowHeight
  }).toBe(true)
  const box = await bounds()
  expect(box.width / box.height).toBeCloseTo(16 / 9, 2)
})

test('toolbar tooltips escape the scrolling ribbon without shifting their controls', async ({ page }, testInfo) => {
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
      await page.screenshot({ path: testInfo.outputPath(`ui-tooltip-${width}-${name === 'Add chart' ? 'chart' : 'graph'}.png`) })
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
  test(`long presentation names keep header controls in place at ${width}px`, async ({ page }, testInfo) => {
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
    await page.screenshot({ path: testInfo.outputPath(`ui-header-${width}.png`), fullPage: true })
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
  test(`DADS application typography, targets and focus preserve slide styling at ${width}px`, async ({ page }, testInfo) => {
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
    await page.screenshot({ path: testInfo.outputPath(`dads-studio-${width}.png`), fullPage: true })
  })

  test(`authoring dialogs fit and have labeled controls at ${width}px`, async ({ page }, testInfo) => {
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
      await page.screenshot({ path: testInfo.outputPath(`authoring-${screenshot}-${width}.png`), fullPage: true })
      await page.getByRole('button', { name: 'Close dialog', exact: true }).click()
    }
    expect(errors).toEqual([])
  })
}

for (const kind of ['Column', 'Bar', 'Line', 'Pie', 'Doughnut', 'Area', 'Scatter', 'Stacked column', 'Stacked bar']) {
  test(`${kind} chart renders after insertion without runtime errors`, async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    await openSample(page)
    await expect(page.getByRole('button', { name: /^Slide 12:/ })).toBeVisible()
    await page.getByRole('button', { name: 'Insert objects', exact: true }).click()
    const inserted = waitForCoreOperation(page, 'transaction')
    await page.getByRole('button', { name: `Insert ${kind} chart`, exact: true }).click()
    await inserted
    const chart = page.locator('.canvas-workspace .recharts-wrapper')
    await expect(chart).toHaveCount(1)
    await expect(chart).toBeVisible()
    expect(await chart.locator('path').count()).toBeGreaterThan(0)
    const undone = waitForCoreOperation(page, 'undo_transaction')
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await undone
    await expect(chart).toHaveCount(0)
    expect(errors).toEqual([])
  })
}