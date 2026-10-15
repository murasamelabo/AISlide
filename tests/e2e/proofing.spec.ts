import { test, expect, type Page } from '@playwright/test'

test.setTimeout(90_000)

async function fixture(page: Page) {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  const base64 = await page.evaluate(async root => {
    const { AislideClient } = await import(`${root}/packages/client/index.mjs`)
    const apiUrl = '/src/api.ts'
    const { core } = await import(apiUrl)
    const client = new AislideClient(core)
    const session = await client.createPresentation('proofing-fixture', 'Synthetic proofing fixture')
    const deck = session.document.deck
    deck.slides[0].elements = [
      { type: 'text', id: 'source', x: 80, y: 100, width: 500, height: 120, text: 'hello report', font_size: 32, color: 'BB2211', bold: true, format: { italic: true, alignment: 'center' } },
      { type: 'text', id: 'target', x: 80, y: 300, width: 500, height: 180, text: 'hello wurld', font_size: 22, color: '202525', bold: false },
    ]
    await session.replaceDeck(deck)
    return (await session.exportPresentation()).base64
  }, `/@fs/${process.cwd().replaceAll('\\', '/')}`)
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'proofing-fixture.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(base64, 'base64') })
  await expect(page.locator('.slide-stage [data-element-id]')).toHaveCount(2)
}

for (const width of [1440, 390]) {
  test(`G04 local proofing and G11 painter work on native text at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 })
    await fixture(page)
    await page.getByRole('button', { name: 'Select target', exact: true }).click()
    await page.getByRole('button', { name: 'Find and replace', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: 'Find and replace', exact: true })
    await dialog.getByRole('tab', { name: 'Proofing', exact: true }).click()
    await dialog.getByRole('combobox', { name: 'Proofing object' }).selectOption('0:1')
    await dialog.getByLabel('Dictionary file', { exact: true }).setInputFiles({ name: 'terms.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ language: 'en-US', words: ['hello', 'world', 'report'], synonyms: { hello: ['greeting'] }, translations: { 'ja-JP': { hello: 'こんにちは' } } })) })
    await expect(dialog.getByRole('status')).toContainText('3 words')
    await dialog.getByRole('button', { name: 'Check spelling', exact: true }).click()
    await expect(dialog.getByText('wurld', { exact: true })).toBeVisible()
    await dialog.getByRole('button', { name: 'Replace wurld with world', exact: true }).click()
    await expect(dialog.getByLabel('Proofing text', { exact: true })).toHaveValue('hello world')
    await dialog.getByRole('button', { name: 'Apply proofing language', exact: true }).click()
    await expect(dialog.getByRole('status')).toContainText('Language applied')
    await dialog.getByLabel('Lookup term', { exact: true }).fill('hello')
    await dialog.getByRole('button', { name: 'Look up term', exact: true }).click()
    await expect(dialog.getByText('greeting', { exact: true })).toBeVisible()
    await expect(dialog.getByText('こんにちは', { exact: true })).toBeVisible()
    await expect(dialog.getByRole('alert')).toHaveCount(0)
    await page.screenshot({ path: `.artifacts/g04-proofing-${width}.png` })
    await dialog.getByRole('button', { name: 'Close dialog' }).click()
    await page.getByRole('button', { name: 'Select source', exact: true }).click()
    await page.getByRole('button', { name: 'Copy format', exact: true }).click()
    await expect(page.locator('.status-bar')).toContainText('Format copied')
    await page.getByRole('button', { name: 'Select target', exact: true }).click()
    await page.getByRole('button', { name: 'Apply format', exact: true }).click()
    await expect(page.locator('.status-bar')).toContainText('Format applied')
    await expect(page.getByRole('alert')).toHaveCount(0)
    const painted = page.locator('.slide-stage [data-element-id="target"]').getByText('hello world', { exact: true })
    await expect(painted).toHaveCSS('font-size', '32px')
    await expect(painted).toHaveCSS('font-weight', '700')
    await expect(painted).toHaveCSS('color', 'rgb(187, 34, 17)')
    await page.screenshot({ path: `.artifacts/g11-painter-${width}.png` })
    await page.getByRole('button', { name: 'Undo', exact: true }).click()
    await expect(page.locator('.slide-stage [data-element-id="target"]')).toContainText('hello world')
    await expect(page.locator('.slide-stage [data-element-id="target"]').getByText('hello world', { exact: true })).toHaveCSS('font-size', '22px')
  })
}

test('G04 cancellation and invalid UTF-8 keep dictionary and document unchanged', async ({ page }) => {
  await fixture(page)
  await expect(page.getByRole('button', { name: 'Apply format', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Find and replace', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Find and replace', exact: true })
  await dialog.getByRole('tab', { name: 'Proofing', exact: true }).click()
  const panel = dialog.getByRole('tabpanel', { name: 'Proofing', exact: true })
  await panel.getByLabel('Dictionary file', { exact: true }).setInputFiles({ name: 'bad.txt', mimeType: 'text/plain', buffer: Buffer.from([0xff, 0xfe, 0x61]) })
  await expect(panel.getByRole('alert')).toBeVisible()
  await expect(panel.getByText('Tiny en-US sample (not a complete dictionary)', { exact: true })).toBeVisible()
  await panel.getByLabel('Dictionary file', { exact: true }).setInputFiles({ name: 'good.txt', mimeType: 'text/plain', buffer: Buffer.from('hello\nreport\n') })
  await expect(panel.getByRole('status')).toContainText('2 words')
  const text = await panel.getByLabel('Proofing text', { exact: true }).inputValue()
  let entered = () => {}
  let release = () => {}
  const started = new Promise<void>(resolve => { entered = resolve })
  const held = new Promise<void>(resolve => { release = resolve })
  await page.route('**/api/core', async route => {
    if (route.request().postDataJSON().op === 'proof_text') { entered(); await held }
    await route.continue()
  })
  try {
    await panel.getByRole('button', { name: 'Check spelling', exact: true }).click()
    await started
    await expect(panel.getByRole('button', { name: 'Apply proofing language', exact: true })).toBeDisabled()
    await panel.getByRole('button', { name: 'Cancel proofing', exact: true }).click()
  } finally { release() }
  await expect(panel.getByRole('button', { name: 'Check spelling', exact: true })).toBeEnabled()
  await expect(panel.getByLabel('Proofing text', { exact: true })).toHaveValue(text)
  await expect(panel.getByText('good.txt', { exact: true })).toBeVisible()
  await expect(panel.getByRole('alert')).toHaveCount(0)
  await expect(panel.getByRole('status')).toHaveText('Cancelled')
})