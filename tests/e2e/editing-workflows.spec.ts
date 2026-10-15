import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { openSample, waitForCoreOperation } from './fixtures'

test.setTimeout(90_000)

async function openEditingFixture(page: Page, rich = true, booleanHole = false) {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled({ timeout: 30_000 })
  const base64 = await page.evaluate(async ({ root, rich, booleanHole }) => {
    const { AislideClient } = await import(`${root}/packages/client/index.mjs`)
    const apiUrl = '/src/api.ts'
    const { core } = await import(apiUrl)
    const client = new AislideClient(core)
    const session = await client.createPresentation('editing-fixture', 'Synthetic editing fixture')
    const deck = session.document.deck
    deck.slides[0].elements = [
      { type: 'rect', id: 'first', x: 100, y: 120, width: 160, height: 100, fill: '007F73' },
      { type: 'rect', id: 'second', x: booleanHole ? 140 : 380, y: booleanHole ? 140 : 240, width: booleanHole ? 60 : 140, height: booleanHole ? 40 : 100, fill: 'C64B40' },
      { type: 'text', id: 'rich', x: 100, y: 450, width: 700, height: 90, text: 'Rich sample', font_size: 28, bold: false, color: '202525', ...(rich ? { format: { paragraphs: [{ runs: [{ text: 'Rich ', style: { bold: true } }, { text: 'sample' }] }] } } : {}) },
    ]
    await session.replaceDeck(deck)
    return (await session.exportPresentation()).base64
  }, { root: `/@fs/${process.cwd().replaceAll('\\', '/')}`, rich, booleanHole })
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'editing-fixture.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(base64, 'base64') })
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(3)
}

for (const width of [1440, 390]) {
  test(`G10 combination menu applies all five operations and Undo at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 })
    await openEditingFixture(page, true, true)
    const combinations: string[] = []
    page.on('request', request => {
      if (request.url().endsWith('/api/core') && request.method() === 'POST') {
        const input = request.postDataJSON()
        if (input.op === 'combine_shapes') combinations.push(input.operation)
      }
    })
    for (const [operation, label, contours] of [['union', 'Union shapes', 1], ['intersect', 'Intersect shapes', 1], ['subtract', 'Subtract shapes', 2], ['xor', 'Exclude overlap (XOR)', 2], ['fragment', 'Fragment shapes', 0]] as const) {
      await page.getByRole('button', { name: 'Select first', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Combine shapes', exact: true })).toBeDisabled()
      await page.getByRole('button', { name: 'Select second', exact: true }).click({ modifiers: ['Shift'] })
      await page.getByRole('button', { name: 'Combine shapes', exact: true }).click()
      const menu = page.getByRole('menu', { name: 'Combine shapes', exact: true })
      await expect(menu.getByRole('menuitem')).toHaveCount(5)
      await menu.getByRole('menuitem', { name: label, exact: true }).click()
      const combined = page.locator('.slide-stage [data-element-id^="combined-"]')
      await expect(combined).toHaveCount(operation === 'fragment' ? 2 : 1)
      await expect(combined.first()).toHaveClass(/selected/)
      await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(operation === 'fragment' ? 3 : 2)
      if (contours === 1) await expect(combined.locator('svg polygon')).toHaveCount(1)
      else if (contours) {
        const paths = await combined.locator('svg path').evaluateAll(paths => paths.map(path => path.getAttribute('d') ?? ''))
        expect(paths.some(path => (path.match(/M/g) ?? []).length === contours)).toBe(true)
      }
      expect(combinations.at(-1)).toBe(operation)
      await expect(page.getByRole('alert')).toHaveCount(0)
      if (operation === 'subtract') {
        await combined.scrollIntoViewIfNeeded()
        await page.screenshot({ path: `.artifacts/g10-studio-${width}.png` })
      }
      await page.getByRole('button', { name: 'Undo', exact: true }).click()
      await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(3)
    }
    expect(combinations).toEqual(['union', 'intersect', 'subtract', 'xor', 'fragment'])
  })

  test(`phase2 object painter and slide eyedropper at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 })
    await openEditingFixture(page)
    const first = page.locator('.slide-stage [data-element-id="first"] svg rect')
    const second = page.locator('.slide-stage [data-element-id="second"] svg rect')
    await page.getByRole('button', { name: 'Select first', exact: true }).click()
    await page.getByRole('button', { name: 'Copy format', exact: true }).click()
    await expect(page.locator('.status-bar')).toContainText('Format copied')
    await page.getByRole('button', { name: 'Select second', exact: true }).click()
    await page.getByRole('button', { name: 'Apply format', exact: true }).click()
    await expect(second).toHaveAttribute('fill', '#007F73')
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect(second).toHaveAttribute('fill', '#C64B40')
    await page.getByRole('button', { name: 'Select first', exact: true }).click()
    await page.getByRole('button', { name: 'Advanced object settings', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: 'Advanced object settings', exact: true })
    await dialog.getByText('Visual', { exact: true }).click()
    await dialog.getByText('Slide eyedropper', { exact: true }).click()
    await dialog.getByRole('button', { name: 'Load slide color preview', exact: true }).click()
    const preview = dialog.getByRole('button', { name: 'Sample slide preview', exact: true })
    await expect(preview.locator('img')).toHaveJSProperty('complete', true)
    const previewSampled = waitForCoreOperation(page, 'sample_slide_pixel')
    await preview.click({ position: { x: 2, y: 2 } })
    await previewSampled
    await expect(dialog.getByLabel('Color HEX', { exact: true })).toHaveValue('FFFFFF')
    await dialog.getByLabel('Sample X', { exact: true }).fill('400')
    await dialog.getByLabel('Sample Y', { exact: true }).fill('250')
    await expect(dialog.getByRole('button', { name: 'Sample slide pixel', exact: true })).toBeEnabled()
    const pixelSampled = waitForCoreOperation(page, 'sample_slide_pixel')
    await dialog.getByRole('button', { name: 'Sample slide pixel', exact: true }).focus()
    await page.keyboard.press('Enter')
    await pixelSampled
    await expect(dialog.getByLabel('Color HEX', { exact: true })).toHaveValue('C64B40')
    await expect(first).toHaveAttribute('fill', '#007F73')
    await dialog.getByLabel('Sample X', { exact: true }).fill('1280')
    await expect(dialog.getByRole('button', { name: 'Sample slide pixel', exact: true })).toBeDisabled()
    await dialog.getByLabel('Sample X', { exact: true }).fill('400')
    await dialog.getByRole('button', { name: 'Use color 007F73', exact: true }).click()
    await expect(dialog.getByLabel('Color HEX', { exact: true })).toHaveValue('007F73')
    await dialog.getByLabel('Color HEX', { exact: true }).fill('C64B40')
    await page.screenshot({ path: `.artifacts/phase2-eyedropper-${width}.png` })
    await dialog.getByRole('button', { name: 'Apply sampled color', exact: true }).click()
    await expect(first).toHaveAttribute('fill', '#C64B40')
    await expect(dialog.getByRole('alert')).toHaveCount(0)
    await dialog.getByRole('button', { name: 'Close dialog', exact: true }).click()
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect(first).toHaveAttribute('fill', '#007F73')
  })
}

test('phase2 SVG literal text and EMF import are visible native assets with Undo', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  const emf = Buffer.alloc(144)
  for (const [offset,value] of [[0,1],[4,88],[16,100],[20,100],[40,0x464d4520],[44,0x10000],[48,144],[52,4],[56,1],[72,100],[76,100],[80,25],[84,25]]) emf.writeUInt32LE(value,offset)
  ;[37,12,0x80000004,43,24,10,10,90,90,14,20,0,0,20].forEach((value,index) => emf.writeUInt32LE(value,88+index*4))
  const svg = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40"><text x="1" y="25" font-size="20">Literal SVG</text></svg>')
  for (const [name,mimeType,buffer] of [['literal.svg','image/svg+xml',svg],['bounded.emf','image/emf',emf]] as const) {
    await page.getByRole('button', { name: 'Insert icons', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: 'Icons and assets', exact: true })
    await dialog.getByLabel('Import asset files', { exact: true }).setInputFiles({ name,mimeType,buffer })
    await expect(dialog).toHaveCount(0)
    const image = page.locator('.slide-stage img')
    await expect(image).toHaveCount(1)
    await expect(image).toHaveJSProperty('complete', true)
    expect(await image.evaluate(image => (image as HTMLImageElement).naturalWidth)).toBeGreaterThan(0)
    await expect(page.getByRole('alert')).toHaveCount(0)
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect(image).toHaveCount(0)
  }
})

test('multi-selection groups and ungroups in one undoable operation', async ({ page }) => {
  await openEditingFixture(page)
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await page.getByRole('button', { name: 'Select second', exact: true }).click({ modifiers: ['Shift'] })
  await expect(page.locator('.slide-stage .slide-element.selected')).toHaveCount(2)
  await page.getByRole('button', { name: 'Group selection', exact: true }).click()
  await expect(page.locator('.slide-stage > .slide-surface > .slide-page > [data-element-id]')).toHaveCount(2)
  await page.getByRole('button', { name: 'Ungroup selection', exact: true }).click()
  await expect(page.locator('.slide-stage .slide-element.selected')).toHaveCount(2)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.slide-stage > .slide-surface > .slide-page > [data-element-id]')).toHaveCount(2)
})

test('page dimensions update the canvas and thumbnails and undo', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Add rectangle', exact: true }).click()
  await page.getByRole('button', { name: 'Document setup', exact: true }).click()
  const panel = page.getByRole('dialog', { name: 'Document setup', exact: true })
  await panel.getByRole('combobox', { name: 'Preset', exact: true }).selectOption('4:3')
  await panel.getByRole('button', { name: 'Apply page size', exact: true }).click()
  await expect(panel.getByRole('button', { name: 'Apply page size', exact: true })).toBeEnabled()
  await expect(panel.getByRole('alert')).toHaveCount(0)
  await expect(page.locator('.slide-stage .slide-page')).toHaveCSS('width', '960px')
  await panel.getByRole('button', { name: 'Close dialog' }).click()
  await expect(page.locator('.slide-stage .slide-page')).toHaveCSS('width', '960px')
  await expect(page.locator('.thumbnail .slide-page').first()).toHaveCSS('width', '960px')
  await expect(page.locator('.status-bar')).toContainText('960')
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.slide-stage .slide-page')).toHaveCSS('width', '1280px')
})

test('review notes and advanced visuals mutate the real session', async ({ page }) => {
  await openEditingFixture(page)
  await page.getByRole('button', { name: 'Review document', exact: true }).click()
  const review = page.getByRole('dialog', { name: 'Review document', exact: true })
  await review.getByLabel('Speaker notes', { exact: true }).fill('Reviewed synthetic notes')
  await review.getByRole('button', { name: 'Apply speaker notes', exact: true }).click()
  await expect(review.getByRole('button', { name: 'Apply speaker notes', exact: true })).toBeEnabled()
  await review.getByRole('button', { name: 'Close dialog' }).click()
  await expect(page.locator('.notes-preview')).toContainText('Reviewed synthetic notes')
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await page.getByRole('button', { name: 'Advanced object settings', exact: true }).click()
  const object = page.getByRole('dialog', { name: 'Advanced object settings', exact: true })
  await object.getByLabel('Rotation', { exact: true }).fill('25')
  await object.getByRole('button', { name: 'Apply visual style', exact: true }).click()
  await expect(object.getByRole('button', { name: 'Apply visual style', exact: true })).toBeEnabled()
  await object.getByRole('button', { name: 'Close dialog' }).click()
  await expect(page.locator('.slide-stage [data-element-id="first"]')).not.toHaveCSS('transform', 'none')
  await expect(page.locator('.slide-stage [data-element-id="first"] .document-ink')).toHaveCSS('transform', 'none')
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('Studio search opens the session text tools with Ctrl+F', async ({ page }) => {
  await openSample(page)
  await page.keyboard.press('Control+f')
  await expect(page.getByRole('dialog', { name: 'Find and replace' })).toBeVisible()
})

for (const rich of [false, true]) {
test(`search replaces real ${rich ? 'rich' : 'plain'} text and navigates to the result`, async ({ page }) => {
  await openEditingFixture(page, rich)
  await page.keyboard.press('Control+h')
  const tools = page.getByRole('dialog', { name: 'Find and replace', exact: true })
  await tools.getByRole('textbox', { name: 'Find text', exact: true }).fill('sample')
  await tools.getByRole('button', { name: 'Find', exact: true }).click()
  await expect(tools.getByRole('button', { name: 'Go to match 1' })).toBeVisible()
  await tools.getByRole('textbox', { name: 'Replace with', exact: true }).fill('replaced')
  await tools.getByRole('button', { name: 'Replace all', exact: true }).click()
  await expect(tools.getByRole('status').first()).toContainText('Replacement applied')
  await tools.getByRole('textbox', { name: 'Find text', exact: true }).fill('replaced')
  await tools.getByRole('button', { name: 'Find', exact: true }).click()
  await tools.getByRole('button', { name: 'Go to match 1' }).click()
  await expect(tools).toHaveCount(0)
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toHaveClass(/selected/)
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toContainText('Rich replaced')
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toContainText('Rich sample')
})
}

for (const mode of ['move', 'resize'] as const) {
  test(`multi ${mode} retains preview until one transaction completes`, async ({ page }) => {
    await openEditingFixture(page)
    await page.getByRole('button', { name: 'Select first', exact: true }).click()
    await page.getByRole('button', { name: 'Select second', exact: true }).click({ modifiers: ['Control'] })
    const first = page.locator('.slide-stage [data-element-id="first"]')
    const second = page.locator('.slide-stage [data-element-id="second"]')
    const before = await Promise.all([first.boundingBox(), second.boundingBox()])
    const control = mode === 'move' ? first.getByRole('button', { name: 'Edit first' }) : page.getByRole('button', { name: 'Resize selection', exact: true })
    const start = (await control.boundingBox())!
    let release = () => {}
    let started = () => {}
    const waiting = new Promise<void>((resolve) => { release = resolve })
    const observed = new Promise<void>((resolve) => { started = resolve })
    let transactions = 0
    let selections = 0
    await page.route('**/api/core', async (route) => {
      const request = route.request().postDataJSON()
      if (request.op === 'edit_selection') { selections += 1; transactions += 1; started(); await waiting }
      await route.continue()
    })
    try {
      await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2)
      await page.mouse.down()
      await page.mouse.move(start.x + start.width / 2 + 35, start.y + start.height / 2 + 18, { steps: 8 })
      const preview = await Promise.all([first.boundingBox(), second.boundingBox()])
      await page.mouse.up()
      await observed
      for (const [index, item] of [first, second].entries()) {
        const samples = await item.evaluate(async (element) => {
          const samples = []
          for (let frame = 0; frame < 4; frame++) { await new Promise(requestAnimationFrame); const bounds = element.getBoundingClientRect(); samples.push({ x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }) }
          return samples
        })
        for (const sample of samples) for (const key of ['x', 'y', 'width', 'height'] as const) expect(Math.abs(sample[key] - preview[index]![key])).toBeLessThan(1)
      }
      expect(selections).toBe(1)
      expect(transactions).toBe(1)
      expect(preview[0]![mode === 'move' ? 'x' : 'width']).toBeGreaterThan(before[0]![mode === 'move' ? 'x' : 'width'] + 5)
    } finally { release() }
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect.poll(async () => Math.abs((await first.boundingBox())!.x - before[0]!.x)).toBeLessThan(1)
    await expect.poll(async () => Math.abs((await second.boundingBox())!.width - before[1]!.width)).toBeLessThan(1)
    await expect(page.getByRole('alert')).toHaveCount(0)
  })
}

test('marquee and Ctrl+A exclude hidden and locked objects', async ({ page }) => {
  await openEditingFixture(page)
  await page.getByRole('button', { name: 'Lock first', exact: true }).click()
  await page.getByRole('button', { name: 'Hide second', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id="second"]')).toHaveAttribute('aria-hidden', 'true')
  await expect(page.locator('.slide-stage [data-element-id="second"] > *')).toHaveCount(0)
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Delete element', exact: true })).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Advanced object settings', exact: true })).toBeDisabled()
  const first = page.locator('.slide-stage [data-element-id="first"]')
  const original = (await first.boundingBox())!
  await first.getByRole('button', { name: 'Edit first', exact: true }).focus()
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('Delete')
  expect((await first.boundingBox())!.x).toBe(original.x)
  const canvas = page.getByRole('main', { name: 'Active slide canvas' })
  await canvas.focus()
  await page.keyboard.press('Control+a')
  await expect(page.locator('.slide-stage .slide-element.selected')).toHaveCount(1)
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toHaveClass(/selected/)
  await page.getByRole('button', { name: 'Unlock first', exact: true }).click()
  await page.getByRole('button', { name: 'Show second', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  const surface = (await page.locator('.slide-stage .slide-surface').boundingBox())!
  const scale = surface.width / 1280
  await page.mouse.move(surface.x + 40 * scale, surface.y + 60 * scale)
  await page.mouse.down()
  await page.mouse.move(surface.x + 600 * scale, surface.y + 400 * scale, { steps: 8 })
  await page.mouse.up()
  await expect(page.locator('.slide-stage .slide-element.selected')).toHaveCount(2)
})

test('clipboard preserves a selection across slides and replacement documents', async ({ page }) => {
  await openEditingFixture(page)
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await page.getByRole('button', { name: 'Select second', exact: true }).click({ modifiers: ['Shift'] })
  await page.keyboard.press('Control+c')
  await expect(page.getByRole('button', { name: 'Paste selection', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'New slide', exact: true }).click()
  await page.getByRole('button', { name: 'Paste selection', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(2)
  await page.getByRole('button', { name: 'New presentation', exact: true }).click()
  await page.getByRole('button', { name: 'Discard changes', exact: true }).click()
  await expect(page.locator('.thumbnail')).toHaveCount(1)
  await page.getByRole('main', { name: 'Active slide canvas' }).evaluate((main) => main.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: new DataTransfer() })))
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(2)
  await page.getByRole('button', { name: 'Cut selection', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(0)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(2)
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('rich property drafts block save while Properties and inline preparation can commit', async ({ page }) => {
  await openEditingFixture(page)
  await page.getByRole('button', { name: 'Select rich', exact: true }).click()
  await page.getByRole('textbox', { name: 'Text content', exact: true }).fill('Rich pending')
  await expect(page.getByRole('button', { name: 'Apply changes', exact: true })).toBeEnabled()
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toContainText('Rich sample')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Apply or cancel')
  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click()
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toContainText('Rich pending')
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toContainText('Rich sample')
  await page.getByRole('button', { name: 'Edit rich', exact: true }).dblclick()
  const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true })
  await editor.fill('Inline prepared')
  let release = () => {}
  const waiting = new Promise<void>((resolve) => { release = resolve })
  let started = () => {}
  const observed = new Promise<void>((resolve) => { started = resolve })
  await page.route('**/api/core', async (route) => { if (route.request().postDataJSON().op === 'replace_element_text') { started(); await waiting } await route.continue() })
  try {
    await editor.press('Control+Enter')
    await observed
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeDisabled()
    await expect(page.getByRole('button', { name: 'New slide', exact: true })).toBeDisabled()
  } finally { release() }
  await expect(editor).toHaveCount(0)
  await expect(page.locator('.slide-stage [data-element-id="rich"]')).toContainText('Inline prepared')
  await expect(page.getByRole('alert')).toHaveCount(0)
})

for (const width of [1440, 390]) {
  test(`editing tools remain keyboard reachable within a ${width}px viewport`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 })
    await page.goto('/')
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled({ timeout: 30_000 })
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    expect(await page.locator('.header-actions').evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true)
    for (const selector of ['.ribbon', '.selection-tools']) {
      const toolbar = page.locator(selector)
      await expect(toolbar).toHaveCSS('overflow-x', 'auto')
      const controls = await toolbar.locator('button:enabled, select:enabled').all()
      expect(controls.length).toBeGreaterThan(0)
      await controls[0].focus()
      for (const control of controls) {
        await expect(control).toBeFocused()
        await expect(control).toBeInViewport({ ratio: 1 })
        await page.keyboard.press('Tab')
      }
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    await page.getByRole('button', { name: 'Document setup', exact: true }).click()
    await expect(page.getByRole('dialog', { name: 'Document setup', exact: true })).toBeVisible()
    expect(await page.getByRole('dialog', { name: 'Document setup', exact: true }).evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true)
  })
}

test('template replacement asks before discarding and keeps the active document on cancel', async ({ page }) => {
  await openEditingFixture(page, false)
  const template = await page.evaluate(async (root) => {
    const { AislideClient } = await import(`${root}/packages/client/index.mjs`)
    const apiUrl = '/src/api.ts'
    const { core } = await import(apiUrl)
    const session = await new AislideClient(core).createPresentation('template-fixture', 'Synthetic template')
    return (await session.exportTemplate('potx')).base64
  }, `/@fs/${process.cwd().replaceAll('\\', '/')}`)
  await page.getByRole('button', { name: 'Add rectangle', exact: true }).click()
  await page.getByRole('button', { name: 'Document setup', exact: true }).click()
  const setup = page.getByRole('dialog', { name: 'Document setup', exact: true })
  await setup.getByRole('checkbox', { name: 'Replace active document', exact: true }).check()
  const file = { name: 'synthetic.potx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.template', buffer: Buffer.from(template, 'base64') }
  await setup.locator('input[type="file"]').setInputFiles(file)
  const decision = page.getByRole('dialog', { name: 'Replace unsaved presentation', exact: true })
  await expect(decision).toBeVisible()
  await decision.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(4)
  await setup.locator('input[type="file"]').setInputFiles(file)
  await decision.getByRole('button', { name: 'Discard changes and open template', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(0)
  await expect(page.locator('.dirty-indicator')).toBeVisible()
  const downloaded = page.waitForEvent('download')
  await setup.getByRole('button', { name: 'Download template', exact: true }).click()
  expect((await downloaded).suggestedFilename()).toMatch(/^aislide-copy-.*\.potx$/)
  await expect(setup.getByRole('alert')).toHaveCount(0)
})

test('phase6 modern native comments reply resolve undo and nondeletable inspection candidates', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled({ timeout: 30_000 })
  const base64 = await page.evaluate(async root => {
    const { AislideClient } = await import(`${root}/packages/client/index.mjs`)
    const apiUrl = '/src/api.ts'
    const { core } = await import(apiUrl)
    const session = await new AislideClient(core).createPresentation('phase6-ui', 'Synthetic Phase 6')
    await session.transact([{ op: 'add', path: '/deck/slides/0/elements/-', value: { type: 'table', id: 'semantic-table', x: 40, y: 40, width: 700, height: 200, rows: [['', 'Column'], ['Row', 'Value']], font_size: 20 } }])
    await session.modernComment('slide-1', { type: 'create', anchor: { kind: 'unknown' }, draft: { author_name: 'Offline fixture', created: '2026-09-19T10:00:00Z', body: [{ runs: [{ text: 'synthetic@example.invalid +1 (202) 555-0147' }] }] } })
    return (await session.exportPresentation()).base64
  }, `/@fs/${process.cwd().replaceAll('\\', '/')}`)
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'phase6-modern.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(base64, 'base64') })
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Review document', exact: true }).click()
  const review = page.getByRole('dialog', { name: 'Review document', exact: true })
  await review.getByLabel('Review section').selectOption('comments')
  await expect(review.getByLabel('Comment format')).toHaveValue('modern')
  await review.getByRole('button', { name: 'Reply to modern thread 1', exact: true }).click()
  await review.getByLabel('Comment author', { exact: true }).fill('Offline fixture')
  await review.getByLabel('Reply text', { exact: true }).fill('Synthetic reply')
  await review.getByRole('button', { name: 'Add modern reply', exact: true }).click()
  await expect(review.getByText('Synthetic reply', { exact: true })).toBeVisible()
  await review.getByLabel('Modern thread 1 status', { exact: true }).selectOption('resolved')
  await expect(review.getByLabel('Modern thread 1 status', { exact: true })).toBeEnabled()
  await review.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await page.getByRole('button', { name: 'Review document', exact: true }).click()
  await review.getByLabel('Review section').selectOption('comments')
  await expect(review.getByLabel('Modern thread 1 status', { exact: true })).toHaveValue('active')
  await review.getByLabel('Review section').selectOption('inspection')
  await review.getByRole('button', { name: 'Inspect document', exact: true }).click()
  const candidates = review.getByRole('table', { name: 'Personal data candidates', exact: true })
  await expect(candidates).toContainText('email_candidate')
  await expect(candidates).toContainText('[REDACTED]')
  await expect(candidates.getByRole('checkbox')).toHaveCount(0)
  await expect(candidates).not.toContainText('example.invalid')
  await expect(review.getByRole('button', { name: 'Download clean PPTX copy', exact: true })).toBeDisabled()
  await page.screenshot({ path: '.artifacts/phase6-review-candidates.png' })
  await expect(review.getByRole('alert')).toHaveCount(0)
  await review.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Select semantic-table', exact: true }).click()
  await page.getByRole('button', { name: 'Review document', exact: true }).click()
  await review.getByLabel('Review section').selectOption('accessibility')
  await review.getByRole('combobox', { name: 'Header policy', exact: true }).selectOption('both')
  await review.getByRole('button', { name: 'Apply table headers', exact: true }).click()
  await expect(review.getByRole('button', { name: 'Apply table headers', exact: true })).toBeEnabled()
  await review.getByRole('button', { name: 'Check accessibility', exact: true }).click()
  await expect(review).toContainText('table_declared_header_empty')
  await review.getByRole('combobox', { name: 'Header policy', exact: true }).selectOption('none')
  await review.getByRole('button', { name: 'Apply table headers', exact: true }).click()
  await expect(review.getByRole('button', { name: 'Apply table headers', exact: true })).toBeEnabled()
  await review.getByRole('button', { name: 'Check accessibility', exact: true }).click()
  await expect(review).toContainText('table_headers_none_review')
  await expect(review).not.toContainText('table_declared_header_empty')
  await page.setViewportSize({ width: 390, height: 844 })
  await expect(review.getByRole('combobox', { name: 'Header policy', exact: true })).toBeVisible()
  const bounds = await review.boundingBox()
  expect(bounds!.x).toBeGreaterThanOrEqual(0)
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(390)
  await page.screenshot({ path: '.artifacts/phase6-review-mobile.png' })
  await expect(review.getByRole('alert')).toHaveCount(0)
})

test('comments accessibility and clean-copy export run from Studio review', async ({ page }) => {
  await openEditingFixture(page, false)
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await page.getByRole('button', { name: 'Review document', exact: true }).click()
  const review = page.getByRole('dialog', { name: 'Review document', exact: true })
  const section = review.getByRole('combobox', { name: 'Review section', exact: true })
  await section.selectOption('comments')
  await review.getByRole('textbox', { name: 'Comment author', exact: true }).fill('Synthetic reviewer')
  await review.getByRole('textbox', { name: 'Comment text', exact: true }).fill('Review fixture comment')
  await review.getByRole('button', { name: 'Add comment', exact: true }).click()
  await expect(review.locator('.object-tools-comment')).toContainText('Review fixture comment')
  await section.selectOption('accessibility')
  await review.getByRole('textbox', { name: 'Accessible title', exact: true }).fill('Synthetic rectangle')
  await review.getByRole('button', { name: 'Apply accessibility', exact: true }).click()
  await expect(review.getByRole('button', { name: 'Apply accessibility', exact: true })).toBeEnabled()
  await review.getByRole('button', { name: 'Check accessibility', exact: true }).click()
  await expect(review.getByRole('button', { name: 'Check accessibility', exact: true })).toBeEnabled()
  await section.selectOption('inspection')
  await review.getByRole('button', { name: 'Inspect document', exact: true }).click()
  await review.getByRole('checkbox', { name: /^comments / }).check()
  await review.getByRole('checkbox', { name: 'Remove selected categories from a new copy', exact: true }).check()
  const downloaded = page.waitForEvent('download')
  await review.getByRole('button', { name: 'Download clean PPTX copy', exact: true }).click()
  expect((await downloaded).suggestedFilename()).toMatch(/^aislide-copy-.*\.pptx$/)
  await expect(review.getByRole('alert')).toHaveCount(0)
})

test('alignment distribution duplicate and delete act on the complete selection', async ({ page }) => {
  await openEditingFixture(page, false)
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await page.getByRole('button', { name: 'Select second', exact: true }).click({ modifiers: ['Meta'] })
  await page.getByRole('button', { name: 'Align objects top', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id="second"]')).toHaveCSS('top', '120px')
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id="second"]')).toHaveCSS('top', '240px')
  await page.getByRole('main', { name: 'Active slide canvas' }).focus()
  await page.keyboard.press('Control+a')
  await expect(page.locator('.slide-stage .slide-element.selected')).toHaveCount(3)
  await page.getByRole('combobox', { name: 'Align relative to', exact: true }).selectOption('page')
  await page.getByRole('button', { name: 'Distribute vertically', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await page.getByRole('button', { name: 'Select second', exact: true }).click({ modifiers: ['Control'] })
  await page.keyboard.press('Control+d')
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(5)
  await page.getByRole('main', { name: 'Active slide canvas' }).focus()
  await page.keyboard.press('Delete')
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(3)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(5)
})

test('external image paste takes priority over Studio clipboard without duplicate handling', async ({ page }) => {
  await openEditingFixture(page, false)
  await page.getByRole('button', { name: 'Select first', exact: true }).click()
  await page.getByRole('button', { name: 'Copy selection', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Paste selection', exact: true })).toBeEnabled()
  await page.getByRole('main', { name: 'Active slide canvas' }).evaluate((main) => {
    const data = new DataTransfer()
    data.items.add(new File(['<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#008877"/></svg>'], 'clipboard.svg', { type: 'image/svg+xml' }))
    main.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: data }))
  })
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(4)
  await expect(page.locator('.slide-stage img')).toHaveCount(1)
  await page.getByRole('button', { name: 'Paste selection', exact: true }).click()
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(5)
  await expect(page.locator('.slide-stage img')).toHaveCount(1)
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('native presentations can add a layout and undo the design change', async ({ page }) => {
  await openEditingFixture(page, false)
  await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click()
  const design = page.getByRole('dialog', { name: 'Masters and layouts', exact: true })
  await design.getByRole('button', { name: 'New layout', exact: true }).click()
  await design.getByRole('button', { name: 'Save design', exact: true }).click()
  await expect(design).toHaveCount(0)
  await expect(page.getByRole('combobox', { name: 'Slide layout', exact: true })).toContainText('New layout')
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(page.getByRole('combobox', { name: 'Slide layout', exact: true })).not.toContainText('New layout')
  await expect(page.getByRole('alert')).toHaveCount(0)
})