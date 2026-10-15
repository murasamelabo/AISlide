import { expect, test } from '@playwright/test'
import sharp from 'sharp'
import { readFile } from 'node:fs/promises'
import { openSample } from './fixtures'
import type { Page } from '@playwright/test'
import type { AislideDocument, Design, MasterImportPreview, MasterSourceInspection } from '../../packages/client/index.mjs'

async function masterImportSource(page: Page, kind: 'pptx' | 'potx', width = 1280) {
  const base64 = await page.evaluate(async ({ root, kind, width }) => {
    const { AislideClient } = await import(`${root}/packages/client/index.mjs`)
    const apiUrl = '/src/api.ts'
    const { core } = await import(apiUrl)
    const client = new AislideClient(core)
    const source = await client.createPresentation(`master-import-${crypto.randomUUID()}`, 'Synthetic import source')
    const design: Design = await client.designDefaults()
    design.masters[0].name = 'Synthetic source master'
    design.masters[0].elements = [{ type: 'text', id: 'source-footer', x: 64, y: 640, width: 600, height: 48, text: 'Synthetic source footer', font_size: 24, color: '@dk1', bold: false }]
    design.layouts = design.layouts.filter(layout => ['blank', 'title-content'].includes(layout.id))
    design.layouts.find(layout => layout.id === 'blank')!.name = 'Source blank'
    design.layouts.find(layout => layout.id === 'title-content')!.name = 'Source title'
    design.layouts.find(layout => layout.id === 'title-content')!.elements.find(element => element.id === 'body')!.height = 300
    await source.replaceDeck({ ...source.document.deck, design })
    await source.assignLayout(source.document.deck.slides[0].id, 'title-content')
    const deck = source.document.deck
    const sample = deck.slides[0]
    sample.title = 'Sample one'
    sample.elements.push({ type: 'text', id: 'fixed-sample-text', x: 64, y: 584, width: 600, height: 44, text: 'Fixed sample text', font_size: 24, color: '@dk1', bold: false })
    deck.slides.push({ ...structuredClone(sample), id: 'sample-two', title: 'Sample two' })
    await source.replaceDeck(deck)
    if (width !== deck.width) await source.resizeCanvas({ width, height: deck.height, mode: 'scale' })
    return (kind === 'potx' ? await source.exportTemplate(kind) : await source.exportPresentation()).base64
  }, { root: `/@fs/${process.cwd().replaceAll('\\', '/')}`, kind, width })
  return { name: `synthetic-master-source.${kind}`, mimeType: kind === 'potx' ? 'application/vnd.openxmlformats-officedocument.presentationml.template' : 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(base64, 'base64') }
}

async function savedPresentation(page: Page) {
  const pending = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  const download = await pending
  const bytes = await readFile((await download.path())!)
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  return bytes
}

async function previewMasters(page: Page) {
  const pending = page.waitForResponse(response => response.url().endsWith('/api/core') && response.request().postDataJSON()?.op === 'preview_master_import')
  await page.getByRole('button', { name: 'Preview masters', exact: true }).click()
  const response = await pending
  expect(response.ok()).toBe(true)
  const result = await response.json() as MasterImportPreview
  await expect(page.getByRole('region', { name: 'Master import', exact: true })).toHaveAttribute('aria-busy', 'false')
  await expect(page.getByRole('region', { name: 'Master import preview', exact: true })).toBeVisible()
  return { result, request: response.request().postDataJSON() as { input: unknown; document: AislideDocument; expected_revision: number; expected_hash: string } }
}

test.beforeEach(async ({ page }) => {
  await openSample(page)
  await expect(page.getByRole('button', { name: 'Slide 12:', exact: false })).toBeVisible()
})

for (const { width, kind, mode } of [{ width: 1440, kind: 'potx', mode: 'masters' }, { width: 390, kind: 'pptx', mode: 'slides' }] as const) {
  test(`master import appends ${kind} ${mode} with preview, Undo and reopen at ${width}px`, async ({ page }) => {
    test.setTimeout(120_000)
    await page.setViewportSize({ width, height: 960 })
    const source = await masterImportSource(page, kind)
    const initial = await savedPresentation(page)
    await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'native-import-target.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: initial })
    await expect(page.getByLabel('Slide layout', { exact: true })).toBeEnabled()
    const original = await savedPresentation(page)
    const originalLayouts = await page.getByLabel('Slide layout', { exact: true }).locator('option').evaluateAll(options => options.map(option => ({ value: (option as HTMLOptionElement).value, text: option.textContent })))
    const originalLayout = await page.getByLabel('Slide layout', { exact: true }).inputValue()
    const chooseSource = async () => {
      await page.getByRole('button', { name: 'Import masters', exact: true }).click()
      await page.getByLabel('Master source file', { exact: true }).setInputFiles(source)
      await expect(page.getByRole('region', { name: 'Master import', exact: true })).toHaveAttribute('aria-busy', 'false')
      await page.getByLabel('Import mode', { exact: true }).selectOption(mode)
      await page.getByLabel('Imported master name', { exact: true }).fill('Reusable import')
      if (mode === 'masters') await page.getByRole('checkbox', { name: 'Select Synthetic source master', exact: true }).check()
      else {
        await page.getByRole('checkbox', { name: 'Select Sample one', exact: true }).check()
        await page.getByRole('checkbox', { name: 'Select Sample two', exact: true }).check()
      }
      await expect(page.getByLabel('Master source file', { exact: true })).toHaveValue('')
    }
    await chooseSource()
    const cancelled = await previewMasters(page)
    expect(cancelled.result.base_revision).toBe(cancelled.request.document.revision)
    expect(cancelled.result.base_hash).toBe(cancelled.request.document.hash)
    expect(cancelled.result.office_visual_parity).toBe(false)
    await expect(page.locator('.dirty-indicator')).toHaveCount(0)
    const previewRegion = page.getByRole('region', { name: 'Master import preview', exact: true })
    await expect(page.getByLabel('Preview layout', { exact: true }).locator('option')).toHaveCount(2)
    await page.getByLabel('Preview layout', { exact: true }).selectOption('1')
    await expect(previewRegion.getByText(mode === 'slides' ? 'Fixed sample text' : 'Synthetic source footer', { exact: true })).toBeVisible()
    await page.evaluate(() => document.fonts.ready)
    await expect.poll(() => page.getByRole('dialog', { name: 'Master import', exact: true }).evaluate(dialog => dialog.scrollWidth <= dialog.clientWidth + 1)).toBe(true)
    await expect.poll(() => previewRegion.locator('.slide-page').evaluate(slide => {
      const bounds = slide.getBoundingClientRect()
      const frame = slide.closest('.master-import-preview')!.getBoundingClientRect()
      return bounds.width > 200 && bounds.left >= frame.left && bounds.right <= frame.right + 1 && bounds.bottom <= frame.bottom + 1
    })).toBe(true)
    await previewRegion.screenshot({ path: `.artifacts/master-import-preview-${width}.png` })
    await page.getByRole('button', { name: 'Cancel', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    expect(await savedPresentation(page)).toEqual(original)
    await chooseSource()
    await expect(page.getByRole('button', { name: 'Add masters', exact: true })).toBeDisabled()
    await previewMasters(page)
    await page.getByLabel('Imported master name', { exact: true }).fill('Reusable import revised')
    await expect(previewRegion).toHaveCount(0)
    await expect(page.getByRole('button', { name: 'Add masters', exact: true })).toBeDisabled()
    const prepared = await previewMasters(page)
    const applied = page.waitForResponse(response => response.url().endsWith('/api/core') && response.request().postDataJSON()?.op === 'import_masters')
    await page.getByRole('button', { name: 'Add masters', exact: true }).click()
    const response = await applied
    expect(response.ok()).toBe(true)
    expect(response.request().postDataJSON()).toMatchObject({ input: prepared.request.input, expected_revision: prepared.result.base_revision, expected_hash: prepared.result.base_hash, expected_candidate_hash: prepared.result.candidate_hash })
    const imported = (await response.json()).document as AislideDocument
    expect(imported.deck.slides).toEqual(prepared.request.document.deck.slides)
    expect(imported.deck.design!.theme).toEqual(prepared.request.document.deck.design!.theme)
    expect(imported.deck.design!.masters.slice(0, prepared.request.document.deck.design!.masters.length)).toEqual(prepared.request.document.deck.design!.masters)
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(page.getByText('Masters added / Existing slides unchanged', { exact: true })).toBeVisible()
    await expect(page.getByLabel('Slide layout', { exact: true })).toHaveValue(originalLayout)
    await expect(page.locator('.thumbnail')).toHaveCount(12)
    const importedBytes = await savedPresentation(page)
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect(page.getByLabel('Slide layout', { exact: true }).locator('option')).toHaveCount(originalLayouts.length)
    expect(await savedPresentation(page)).toEqual(original)
    await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'imported-masters.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: importedBytes })
    await expect(page.getByLabel('Slide layout', { exact: true }).locator('option')).toHaveCount(originalLayouts.length + prepared.result.layout_ids.length)
    const master = prepared.result.design.masters.find(entry => prepared.result.master_ids.includes(entry.id))!
    const layout = prepared.result.design.layouts.find(entry => prepared.result.layout_ids.includes(entry.id) && entry.elements.some(element => element.type === 'text' && element.format?.placeholder?.kind === 'title'))!
    expect(layout).toBeTruthy()
    if (mode === 'slides') {
      const fixed = layout.elements.find(element => element.type === 'text' && element.text === 'Fixed sample text')!
      expect(fixed).toBeTruthy()
      expect(fixed.type === 'text' && fixed.format?.placeholder).toBeFalsy()
    }
    await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click()
    await page.getByRole('button', { name: `Master ${master.name}`, exact: true }).click()
    await page.getByRole('button', { name: 'Add common text', exact: true }).click()
    await page.getByLabel('Design element text', { exact: true }).fill('Imported master edited')
    await page.getByRole('button', { name: 'Save design', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await page.getByLabel('Slide layout', { exact: true }).selectOption(layout.id)
    await expect(page.locator('.canvas-workspace .master-graphics').getByText('Imported master edited', { exact: true })).toBeVisible()
    await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Quarterly performance' })).toBeVisible()
    if (mode === 'slides') await expect(page.locator('.canvas-workspace .master-graphics').getByText('Fixed sample text', { exact: true })).toBeVisible()
    await expect(page.getByRole('alert')).toHaveCount(0)
  })
}

test('master import invalidates drafts and exposes core rejections while guarding pending work', async ({ page }) => {
  test.setTimeout(90_000)
  const source = await masterImportSource(page, 'pptx')
  const mismatch = await masterImportSource(page, 'potx', 960)
  const original = await savedPresentation(page)
  await page.getByRole('button', { name: 'Import masters', exact: true }).click()
  await page.route('**/api/core', async route => {
    if (route.request().postDataJSON()?.op !== 'inspect_master_source') return route.continue()
    const response = await route.fetch()
    expect(response.ok()).toBe(true)
    const inspection = await response.json() as MasterSourceInspection
    inspection.masters.push({ id: 'unavailable-fixture', name: 'Unsupported source entry', layout_count: 1, importable: false, reason: 'Synthetic unsupported relationship' })
    await route.fulfill({ response, json: inspection })
  })
  await page.getByLabel('Master source file', { exact: true }).setInputFiles(source)
  await expect(page.getByRole('checkbox', { name: 'Select Unsupported source entry', exact: true })).toBeDisabled()
  await expect(page.getByText('Synthetic unsupported relationship', { exact: true })).toBeVisible()
  await page.unroute('**/api/core')
  await page.getByRole('checkbox', { name: 'Select Synthetic source master', exact: true }).check()
  await previewMasters(page)
  await page.getByRole('checkbox', { name: 'Select Synthetic source master', exact: true }).uncheck()
  await expect(page.getByRole('region', { name: 'Master import preview', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Add masters', exact: true })).toBeDisabled()
  await page.getByRole('checkbox', { name: 'Select Synthetic source master', exact: true }).check()
  await previewMasters(page)
  await page.getByLabel('Import mode', { exact: true }).selectOption('slides')
  await expect(page.getByRole('button', { name: 'Add masters', exact: true })).toBeDisabled()
  await expect(page.getByRole('region', { name: 'Master import', exact: true }).getByRole('checkbox', { checked: true })).toHaveCount(0)
  await page.getByRole('checkbox', { name: 'Select Sample one', exact: true }).check()
  await previewMasters(page)
  await page.getByLabel('Master source file', { exact: true }).setInputFiles(mismatch)
  await expect(page.getByRole('region', { name: 'Master import preview', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Add masters', exact: true })).toBeDisabled()
  await page.getByRole('checkbox', { name: 'Select Sample one', exact: true }).check()
  const rejected = page.waitForResponse(response => response.url().endsWith('/api/core') && response.request().postDataJSON()?.op === 'preview_master_import')
  await page.getByRole('button', { name: 'Preview masters', exact: true }).click()
  const rejection = await rejected
  expect(rejection.ok()).toBe(false)
  const reason = (await rejection.json()).error as string
  expect(reason).toMatch(/dimension|size/i)
  await expect(page.getByRole('alert')).toHaveText(reason)
  await page.getByLabel('Master source file', { exact: true }).setInputFiles(source)
  await page.getByRole('checkbox', { name: 'Select Sample one', exact: true }).check()
  let release: () => void = () => {}
  let observed: () => void = () => {}
  const held = new Promise<void>(resolve => { observed = resolve })
  const gate = new Promise<void>(resolve => { release = resolve })
  await page.route('**/api/core', async route => {
    if (route.request().postDataJSON()?.op !== 'preview_master_import') return route.continue()
    const response = await route.fetch()
    observed()
    await gate
    await route.fulfill({ response })
  })
  const pendingPreview = previewMasters(page)
  try {
    await held
    await expect(page.getByRole('button', { name: 'Close dialog', exact: true })).toBeDisabled()
    await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled()
    await expect(page.getByLabel('Master source file', { exact: true })).toBeDisabled()
    await expect(page.getByLabel('Import mode', { exact: true })).toBeDisabled()
    await expect(page.getByLabel('Imported master name', { exact: true })).toBeDisabled()
    await expect(page.getByRole('checkbox', { name: 'Select Sample one', exact: true })).toBeDisabled()
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog', { name: 'Master import', exact: true })).toBeVisible()
  } finally { release(); await pendingPreview; await page.unroute('**/api/core') }
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Import masters', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Master import preview', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Add masters', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  expect(await savedPresentation(page)).toEqual(original)
})

for (const width of [1440, 390]) {
  test(`G25 G27 auxiliary masters and rich notes are editable at ${width}px`, async ({ page }) => {
    test.setTimeout(90_000)
    await page.setViewportSize({ width, height: 960 })
    await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click()
    await page.getByRole('button', { name: 'Notes master', exact: true }).click()
    await page.getByRole('button', { name: 'Add common text', exact: true }).click()
    await page.getByLabel('Design element text', { exact: true }).fill('Notes master footer')
    await page.getByLabel('Design element x', { exact: true }).fill('70')
    await page.getByText('Dynamic fields', { exact: true }).click()
    await page.getByRole('button', { name: 'Add slide number', exact: true }).click()
    await page.getByLabel('Design element y', { exact: true }).fill('820')
    await page.getByLabel('Auxiliary preview page', { exact: true }).fill('3')
    await expect(page.locator('.design-canvas .slide-text').filter({ hasText: /^3$/ })).toBeVisible()
    await page.getByRole('button', { name: 'Handout master', exact: true }).click()
    await page.getByRole('button', { name: 'Add common text', exact: true }).click()
    await page.getByLabel('Design element text', { exact: true }).fill('Handout master footer')
    await expect(page.locator('.design-canvas .slide-page')).toHaveJSProperty('clientHeight', 960)
    await page.locator('.design-canvas .slide-page').scrollIntoViewIfNeeded()
    await expect(page.locator('.design-canvas .slide-text').filter({ hasText: 'Handout master footer' })).toBeInViewport()
    await page.locator('.design-canvas').screenshot({ path: `.artifacts/g25-g27-masters-${width}.png` })
    await page.getByRole('button', { name: 'Save design', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click()
    await page.getByRole('button', { name: 'Notes master', exact: true }).click()
    await expect(page.locator('.design-canvas').getByText('Notes master footer', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Handout master', exact: true }).click()
    await expect(page.locator('.design-canvas').getByText('Handout master footer', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Save design', exact: true }).click()
    await page.getByRole('tab', { name: 'Notes', exact: true }).click()
    await page.getByRole('button', { name: 'Edit speaker notes', exact: true }).click()
    await page.getByLabel('Speaker notes', { exact: true }).fill('Rich notes from Studio')
    await page.getByRole('button', { name: 'Format notes', exact: true }).click()
    await page.getByRole('button', { name: 'Notes bold', exact: true }).click()
    const notesUpdated = page.waitForResponse((response) => response.url().endsWith('/api/core') && response.request().postDataJSON()?.op === 'update_rich_notes')
    await page.getByRole('button', { name: 'Apply speaker notes', exact: true }).click()
    expect((await notesUpdated).ok()).toBe(true)
    await expect(page.getByRole('region', { name: 'Review', exact: true })).toHaveAttribute('aria-busy', 'false')
    await expect(page.getByRole('alert')).toHaveCount(0)
    await page.screenshot({ path: `.artifacts/g25-g27-notes-${width}.png`, fullPage: true })
    await page.getByRole('button', { name: 'Close dialog', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(page.locator('.notes-panel').getByText('Rich notes from Studio', { exact: true })).toHaveCSS('font-weight', '700')
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect(page.locator('.notes-panel').getByText('Rich notes from Studio', { exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Redo', exact: true }).click()
    const pendingDownload = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
    const download = await pendingDownload
    const bytes = await readFile((await download.path())!)
    await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'native-rich-notes.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: bytes })
    await expect(page.locator('.dirty-indicator')).toHaveCount(0)
    await page.getByRole('tab', { name: 'Notes', exact: true }).click()
    await page.getByRole('button', { name: 'Edit speaker notes', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Notes bold', exact: true })).toHaveAttribute('aria-pressed', 'true')
    await page.getByLabel('Notes run text', { exact: true }).fill('Native notes changed')
    const nativeUpdated = page.waitForResponse((response) => response.url().endsWith('/api/core') && response.request().postDataJSON()?.op === 'update_rich_notes')
    await page.getByRole('button', { name: 'Apply speaker notes', exact: true }).click()
    expect((await nativeUpdated).ok()).toBe(true)
    await expect(page.getByRole('region', { name: 'Review', exact: true })).toHaveAttribute('aria-busy', 'false')
    await page.getByRole('button', { name: 'Close dialog', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await page.getByRole('button', { name: 'Slide 2:', exact: false }).click()
    await page.getByText('Notes page', { exact: true }).click()
    await expect(page.getByLabel('Notes page 2', { exact: true }).locator('.slide-text').filter({ hasText: /^2$/ })).toBeVisible()
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await page.getByRole('button', { name: 'Slide 1:', exact: false }).click()
    await expect(page.locator('.notes-panel').getByText('Rich notes from Studio', { exact: true })).toBeVisible()
    await expect(page.getByRole('alert')).toHaveCount(0)
  })
}

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
  const slide = page.locator('.canvas-workspace .slide-page')
  const themedPixels = async () => {
    const { data, info } = await sharp(await slide.screenshot()).removeAlpha().raw().toBuffer({ resolveWithObject: true })
    let count = 0
    for (let offset = 0; offset < data.length; offset += info.channels) {
      if (data[offset] === 181 && data[offset + 1] === 48 && data[offset + 2] === 85) count += 1
    }
    return count
  }
  await expect.poll(themedPixels).toBe(0)
  await page.getByRole('button', { name: 'Edit theme', exact: true }).click()
  await page.getByLabel('Theme name', { exact: true }).fill('Editorial theme')
  await page.getByLabel('Theme accent1', { exact: true }).fill('#b53055')
  await page.getByLabel('Heading font', { exact: true }).fill('Arial')
  await page.getByRole('button', { name: 'Apply theme', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect.poll(() => slide.locator('svg rect').evaluateAll((nodes) => nodes.some((node) => getComputedStyle(node).fill === 'rgb(181, 48, 85)'))).toBe(true)
  await expect.poll(themedPixels).toBeGreaterThan(100)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect.poll(() => slide.locator('svg rect').evaluateAll((nodes) => nodes.some((node) => getComputedStyle(node).fill === 'rgb(181, 48, 85)'))).toBe(false)
  await expect.poll(themedPixels).toBe(0)
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

for (const width of [1440, 390]) {
  test(`G23 G25 distinct master fonts and linked slide fields at ${width}px`, async ({ page }) => {
    test.setTimeout(90_000)
    await page.setViewportSize({ width, height: 960 })
    const base64 = await page.evaluate(async (root) => {
      const { AislideClient } = await import(`${root}/packages/client/index.mjs`)
      const apiUrl = '/src/api.ts'
      const { core } = await import(apiUrl)
      const client = new AislideClient(core)
      const session = await client.createPresentation('master-field-browser', 'Synthetic master fields')
      const design = await client.designDefaults()
      const theme = structuredClone(design.theme)
      theme.fonts.minor = 'Courier New'
      design.masters.push({ id: 'second-master', name: 'Second', background: '@lt1', elements: [], theme })
      design.layouts.push({ id: 'second-layout', name: 'Second layout', master_id: 'second-master', background: null, elements: [] })
      const deck = session.document.deck
      deck.design = design
      deck.slides[0].layout_id = 'blank'
      deck.slides[0].elements = [{ type: 'text', id: 'sample', x: 80, y: 90, width: 1000, height: 80, text: 'Master font sample', font_size: 28, color: '@dk1', bold: false, format: { font_family: '@minor' } }]
      deck.slides.push({ ...structuredClone(deck.slides[0]), id: 'slide-2', title: 'Second', layout_id: 'second-layout' })
      await session.replaceDeck(deck)
      return (await session.exportPresentation()).base64
    }, `/@fs/${process.cwd().replaceAll('\\', '/')}`)
    await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'master-fields.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(base64, 'base64') })
    const discard = page.getByRole('button', { name: 'Discard changes', exact: true })
    if (await discard.isVisible()) await discard.click()
    await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Master font sample' })).toBeVisible()
    await page.getByRole('button', { name: 'Slide 2:', exact: false }).click()
    await expect(page.locator('.canvas-workspace .slide-text').first()).toHaveCSS('font-family', /Courier New/)
    await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click()
    await page.getByRole('button', { name: 'Master Second', exact: true }).click()
    await page.getByText('Master theme', { exact: true }).click()
    await expect(page.getByLabel('Master minor font', { exact: true })).toHaveValue('Courier New')
    await page.getByLabel('Master minor font', { exact: true }).fill('Arial')
    await page.getByText('Dynamic fields', { exact: true }).click()
    await page.getByLabel('Design field reference date', { exact: true }).fill('2026-09-18')
    await page.getByRole('button', { name: 'Add slide number', exact: true }).click()
    await page.getByLabel('Design footer text', { exact: true }).fill('Synthetic footer')
    await page.getByRole('button', { name: 'Add footer', exact: true }).click()
    await page.getByLabel('Design field reference date', { exact: true }).scrollIntoViewIfNeeded()
    await page.screenshot({ path: `.artifacts/g23-g25-controls-${width}.png`, fullPage: true })
    await page.getByRole('button', { name: 'Save design', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(page.locator('.canvas-workspace .slide-text').first()).toHaveCSS('font-family', /Arial/)
    await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: /^2$/ })).toBeVisible()
    await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Synthetic footer' })).toBeVisible()
    await page.locator('.canvas-workspace .slide-page').scrollIntoViewIfNeeded()
    await page.screenshot({ path: `.artifacts/g23-g25-${width}.png`, fullPage: true })
    await page.getByRole('button', { name: 'Slide 1:', exact: false }).click()
    await expect(page.locator('.canvas-workspace .slide-text').first()).not.toHaveCSS('font-family', /Courier New/)
    await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: /^2$/ })).toHaveCount(0)
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await page.getByRole('button', { name: 'Slide 2:', exact: false }).click()
    await expect(page.locator('.canvas-workspace .slide-text').first()).toHaveCSS('font-family', /Courier New/)
    await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: /^2$/ })).toHaveCount(0)
    await expect(page.getByRole('alert')).toHaveCount(0)
  })
}