import { expect, test } from '@playwright/test'
import type { Download, Page } from '@playwright/test'
import { build } from 'vite'
test.setTimeout(60_000)

async function bytes(download: Download) {
  const stream = await download.createReadStream()
  if (!stream) throw new Error('Download has no bytes')
  const chunks: Buffer[] = []
  for await (const chunk of stream) chunks.push(Buffer.from(chunk))
  return Buffer.concat(chunks)
}

async function open(page: Page) {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
}

test('recovery discard refuses a newer snapshot and export errors leave controls usable', async ({ page }) => {
  await open(page)
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  const recovery = page.getByRole('dialog', { name: 'Local recovery', exact: true })
  await recovery.getByRole('checkbox').click()
  await expect(recovery.getByRole('checkbox')).toBeChecked()
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Add text', exact: true }).click()
  await expect(page.getByRole('status').filter({ hasText: 'Recovery copy saved' })).toBeVisible()
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await expect(recovery.getByRole('button', { name: 'Discard Untitled presentation.pptx', exact: true })).toBeEnabled()
  const newer = await page.evaluate(async () => {
    const modulePath = '/src/recovery-v2.ts'
    const { createSessionRecoveryStore } = await import(modulePath)
    const recoveryStore = createSessionRecoveryStore()
    const entries = await recoveryStore.list()
    const validate = async (document: unknown) => {
      const response = await fetch('/api/core', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ op: 'verify_recovery', document }) })
      if (!response.ok) throw new Error('Fixture verification failed')
      return response.json()
    }
    const recovered = await recoveryStore.load(entries[0].id, entries[0])
    const document = await validate(recovered.envelope.document)
    const response = await fetch('/api/core', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ op: 'transaction', document,
      transaction: { expected_revision: document.revision, expected_hash: document.hash, operations: [{ op: 'replace', path: '/deck/title', value: 'Newer snapshot' }] } }) })
    if (!response.ok) throw new Error('Fixture transaction failed')
    const changed = await response.json()
    return recoveryStore.save({ ...recovered.envelope, document: changed.document, past: [...recovered.envelope.past, changed.receipt], future: [] }, 'Untitled presentation.pptx')
  })
  await recovery.getByRole('button', { name: 'Discard Untitled presentation.pptx', exact: true }).click()
  await expect(recovery.getByRole('alert')).toContainText('changed')
  await recovery.getByRole('button', { name: 'Refresh recovery', exact: true }).click()
  await expect(recovery.locator('code').first()).toHaveText(newer.hash)
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  const saved = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  await bytes(await saved)
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await expect(recovery.locator('code').first()).toHaveText(newer.hash)
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Export PDF and images', exact: true }).click()
  const output = page.getByRole('dialog', { name: 'Export PDF and images', exact: true })
  await output.getByLabel('Export scale').fill('16')
  await output.getByRole('button', { name: 'Prepare export', exact: true }).click()
  await expect(output.getByRole('alert')).toContainText('8192')
  await expect(output.getByRole('button', { name: 'Prepare export', exact: true })).toBeEnabled()
});

test('actual Studio exports PDF and every PNG page without changing source or printing automatically', async ({ page, context }) => {
  await open(page)
  await page.getByRole('button', { name: 'Add text', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled()
  await page.keyboard.press('Control+m')
  await expect(page.locator('.status-bar')).toContainText('2 slides')
  const save = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  const source = await bytes(await save)
  await page.getByRole('button', { name: 'Export PDF and images', exact: true }).click()
  const panel = page.getByRole('dialog', { name: 'Export PDF and images', exact: true })
  await expect(panel.getByLabel('Export format')).toHaveValue('pdf')
  await expect(panel).toContainText('outlined')
  const pagesBefore = context.pages().length
  await panel.getByRole('button', { name: 'Prepare export', exact: true }).click()
  await expect(panel.getByRole('button', { name: 'Download report.pdf', exact: true })).toBeEnabled()
  expect(context.pages()).toHaveLength(pagesBefore)
  await page.screenshot({ path: '.artifacts/output-recovery-pdf-desktop.png' })
  await page.setViewportSize({ width: 390, height: 844 })
  await expect.poll(() => panel.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.screenshot({ path: '.artifacts/output-recovery-pdf-mobile.png' })
  await page.setViewportSize({ width: 1440, height: 960 })
  const pdfDownload = page.waitForEvent('download')
  await panel.getByRole('button', { name: 'Download report.pdf', exact: true }).click()
  expect((await bytes(await pdfDownload)).subarray(0, 5).toString()).toBe('%PDF-')
  await expect(panel.getByRole('button', { name: 'Print PDF', exact: true })).toBeEnabled()
  await panel.getByLabel('Export format').selectOption('png')
  await panel.getByLabel('Export pages').selectOption('all')
  await panel.getByRole('button', { name: 'Prepare export', exact: true }).click()
  for (const name of ['report-page-001.png', 'report-page-002.png']) {
    const download = page.waitForEvent('download')
    await panel.getByRole('button', { name: `Download ${name}`, exact: true }).click()
    expect((await bytes(await download)).subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))
  }
  const allImages: Download[] = []
  const collect = (download: Download) => { allImages.push(download) }
  page.on('download', collect)
  await panel.getByRole('button', { name: 'Download all images', exact: true }).click()
  await expect.poll(() => allImages.length).toBe(2)
  page.off('download', collect)
  expect(allImages.map(download => download.suggestedFilename())).toEqual(['report-page-001.png', 'report-page-002.png'])
  for (const download of allImages) expect((await bytes(download)).subarray(1, 4).toString()).toBe('PNG')
  await panel.getByLabel('Export format').selectOption('jpeg')
  await panel.getByLabel('Export pages').selectOption('selected')
  await panel.getByRole('button', { name: 'Prepare export', exact: true }).click()
  const jpegDownload = page.waitForEvent('download')
  await panel.getByRole('button', { name: 'Download report-page-002.jpg', exact: true }).click()
  expect((await bytes(await jpegDownload)).subarray(0, 3)).toEqual(Buffer.from([255, 216, 255]))
  await expect(panel.locator('.static-export-files li')).toHaveCount(1)
  await panel.getByRole('button', { name: 'Close dialog', exact: true }).click()
  const savedAgain = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  expect(await bytes(await savedAgain)).toEqual(source)
});

test('actual Studio recovery is opt-in, restores verified history and guards replacement', async ({ page }) => {
  await open(page)
  expect(await page.evaluate(async () => (await indexedDB.databases()).some(database => database.name === 'aislide.recovery.v1'))).toBe(false)
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  const recovery = page.getByRole('dialog', { name: 'Local recovery', exact: true })
  await expect(recovery.getByRole('checkbox')).not.toBeChecked()
  await recovery.getByRole('checkbox').click()
  await expect(recovery.getByRole('checkbox')).toBeChecked()
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Add text', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled()
  await expect(page.getByRole('status').filter({ hasText: 'Recovery copy saved' })).toBeVisible()
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await expect(recovery.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true })).toBeVisible()
  const hash = await recovery.locator('code').first().textContent()
  await page.screenshot({ path: '.artifacts/output-recovery-history-desktop.png' })
  await page.setViewportSize({ width: 390, height: 844 })
  await expect.poll(() => recovery.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.screenshot({ path: '.artifacts/output-recovery-history-mobile.png' })
  await page.setViewportSize({ width: 1440, height: 960 })
  await recovery.getByRole('checkbox').click()
  await expect(recovery.getByRole('checkbox')).not.toBeChecked()
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await expect(page.getByRole('main', { name: 'Active slide canvas' }).locator('[data-element-id]')).toHaveCount(0)
  await page.getByRole('button', { name: 'Add rectangle', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await recovery.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true }).click()
  const guard = page.getByRole('dialog', { name: 'Unsaved changes', exact: true })
  await expect(guard).toBeVisible()
  await guard.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.getByRole('main', { name: 'Active slide canvas' })).not.toContainText('New text')
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await recovery.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true }).click()
  await guard.getByRole('button', { name: 'Discard changes', exact: true }).click()
  await expect(page.getByRole('main', { name: 'Active slide canvas' })).toContainText('New text')
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled()
  await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await expect(recovery.locator('code').first()).toHaveText(hash!)
  await recovery.getByRole('button', { name: 'Discard Untitled presentation.pptx', exact: true }).click()
  await expect(recovery.getByRole('status')).toHaveText('0 recovery copies')
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  const saved = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click()
  const source = await bytes(await saved)
  await page.getByRole('button', { name: 'Add rectangle', exact: true }).click()
  await expect(page.getByRole('main', { name: 'Active slide canvas' }).locator('[data-element-id]')).toHaveCount(2)
  await page.getByRole('button', { name: 'New presentation', exact: true }).click()
  await expect(guard).toBeVisible()
  await guard.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'reopen.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: source })
  await expect(guard).toBeVisible()
  await guard.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.getByRole('main', { name: 'Active slide canvas' }).locator('[data-element-id]')).toHaveCount(2)
  await page.getByRole('button', { name: 'New presentation', exact: true }).click()
  await guard.getByRole('button', { name: 'Discard changes', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeDisabled()
});

test('actual recovery does not trust a forged storage hash and reports snapshot permission errors', async ({ page }) => {
  await open(page)
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  const recovery = page.getByRole('dialog', { name: 'Local recovery', exact: true })
  await recovery.getByRole('checkbox').click()
  await expect(recovery.getByRole('checkbox')).toBeChecked()
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Add text', exact: true }).click()
  await expect(page.getByRole('status').filter({ hasText: 'Recovery copy saved' })).toBeVisible()
  await page.evaluate(async () => {
    const modulePath = '/src/recovery-v2.ts'
    const { createSessionRecoveryStore, SESSION_RECOVERY_DB } = await import(modulePath)
    const recoveryStore = createSessionRecoveryStore()
    const entry = (await recoveryStore.list())[0]
    const recovered = await recoveryStore.load(entry.id, entry)
    const tampered = structuredClone(recovered.envelope)
    tampered.document.deck.title = 'Forged recovery title'
    await new Promise<void>((resolve, reject) => {
      const request = indexedDB.open(SESSION_RECOVERY_DB, 1)
      request.onerror = () => reject(request.error)
      request.onsuccess = () => {
        const database = request.result
        const transaction = database.transaction('slots', 'readwrite')
        transaction.objectStore('slots').put(JSON.stringify(tampered), entry.integrity)
        transaction.oncomplete = () => { database.close(); resolve() }
        transaction.onabort = () => { database.close(); reject(transaction.error) }
      }
    })
  })
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await recovery.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true }).click()
  await expect(recovery.getByRole('alert')).toContainText('integrity mismatch')
  await expect(page.getByRole('dialog', { name: 'Unsaved changes', exact: true })).toHaveCount(0)
  await recovery.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Rename presentation', exact: true })).toHaveText('Untitled presentation')
  await expect(page.getByRole('main', { name: 'Active slide canvas' })).toContainText('New text')
  await page.evaluate(() => { IDBFactory.prototype.open = () => { throw new DOMException('Injected test denial', 'SecurityError') } })
  await page.getByRole('button', { name: 'Add rectangle', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Injected test denial')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
});

test('print content is owned, prepared separately, and only requests a dialog on user confirmation', async ({ page, context }) => {
  await open(page)
  await page.evaluate(() => {
    document.body.dataset.dialogRequests = '0'
    window.print = () => {
      document.body.dataset.dialogRequests = String(Number(document.body.dataset.dialogRequests) + 1)
      window.dispatchEvent(new Event('beforeprint'))
      window.dispatchEvent(new Event('afterprint'))
    }
  })
  await page.getByRole('button', { name: 'Add text', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled()
  await page.keyboard.press('Control+m')
  await expect(page.locator('.status-bar')).toContainText('2 slides')
  await page.getByRole('button', { name: 'Export PDF and images', exact: true }).click()
  const panel = page.getByRole('dialog', { name: 'Export PDF and images', exact: true })
  await panel.getByLabel('Export scale').fill('0.5')
  await panel.getByRole('button', { name: 'Prepare export', exact: true }).click()
  await expect(panel.getByRole('button', { name: 'Print PDF', exact: true })).toBeEnabled()
  const pagesBefore = context.pages().length
  await panel.getByRole('button', { name: 'Print PDF', exact: true }).click()
  const printRoot = page.locator('body > [data-aislide-print]')
  await expect(printRoot).toHaveCount(1)
  await expect(printRoot.locator('img')).toHaveCount(2)
  await expect(printRoot.locator('img').last()).toHaveAttribute('alt', 'Slide 2')
  await expect(page.locator('iframe')).toHaveCount(0)
  await expect(panel.getByRole('button', { name: 'Open print dialog', exact: true })).toBeEnabled()
  expect(context.pages()).toHaveLength(pagesBefore)
  const imageUrl = await printRoot.evaluate(element => {
    const image = element.querySelector('img')!
    if (!image.complete || !image.naturalWidth || !image.src.startsWith('blob:')) throw new Error('Print image was not prepared')
    if (image.naturalWidth !== 1280 || image.naturalHeight !== 720) throw new Error('Print must use 96 dpi independent of the PDF scale')
    if (element.querySelector(':scope > :not(img)')) throw new Error('Print content must contain only images')
    return image.src
  })
  await expect(page.locator('body')).toHaveAttribute('data-dialog-requests', '0')
  await expect(printRoot).toBeHidden()
  await expect(panel).toBeVisible()
  await page.emulateMedia({ media: 'print' })
  await expect(printRoot).toBeVisible()
  expect(await page.locator('style[data-aislide-print-style]').textContent()).toContain('@page { size: 13.333333333333334in 7.5in; margin: 0; }')
  expect(await page.locator('body > *').evaluateAll(elements => elements.filter(element => getComputedStyle(element).display !== 'none').map(element => element.getAttribute('data-aislide-print')))).toEqual([await printRoot.getAttribute('data-aislide-print')])
  await page.emulateMedia({ media: 'screen' })
  await expect(panel).toBeVisible()
  await panel.getByRole('button', { name: 'Open print dialog', exact: true }).click()
  await expect(page.locator('body')).toHaveAttribute('data-dialog-requests', '1')
  await expect(panel.getByRole('status')).toContainText('closed')
  await expect(panel.getByRole('status')).toContainText('cannot determine')
  await expect(printRoot).toHaveCount(0)
  await expect(page.locator('style[data-aislide-print-style]')).toHaveCount(0)
  expect(await page.evaluate(async url => { try { await fetch(url); return false } catch { return true } }, imageUrl)).toBe(true)
  await expect(panel.getByRole('button', { name: 'Download report.pdf', exact: true })).toBeEnabled()
  await panel.getByRole('button', { name: 'Print PDF', exact: true }).click()
  await expect(panel.getByRole('button', { name: 'Open print dialog', exact: true })).toBeEnabled()
  await panel.getByLabel('Export pages').selectOption('selected')
  await expect(printRoot).toHaveCount(0)
  await expect(panel.getByRole('button', { name: 'Open print dialog', exact: true })).toHaveCount(0)
  await panel.getByRole('button', { name: 'Prepare export', exact: true }).click()
  await panel.getByRole('button', { name: 'Print PDF', exact: true }).click()
  await expect(panel.getByRole('button', { name: 'Open print dialog', exact: true })).toBeEnabled()
  await panel.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await expect(printRoot).toHaveCount(0)
  await expect(page.locator('style[data-aislide-print-style]')).toHaveCount(0)
});

test('print rejects invalid bounded PNG responses and explicitly reports an ignored print API', async ({ page }) => {
  await open(page)
  await page.getByRole('button', { name: 'Export PDF and images', exact: true }).click()
  const panel = page.getByRole('dialog', { name: 'Export PDF and images', exact: true })
  await panel.getByRole('button', { name: 'Prepare export', exact: true }).click()
  await expect(panel.getByRole('button', { name: 'Print PDF', exact: true })).toBeEnabled()
  let mode = 'mime'
  await page.route('**/api/core', async route => {
    const request = route.request().postDataJSON()
    if (request.op !== 'export_static' || request.options.format !== 'png') return route.continue()
    expect(request.options.scale).toBe(1)
    expect(request.options.transparent).toBe(false)
    const response = await route.fetch()
    const result = await response.json()
    if (mode === 'mime') result.files[0].mime_type = 'image/svg+xml'
    if (mode === 'count') result.files = Array(33).fill(result.files[0])
    if (mode === 'page') result.files[0].page_indices = [99]
    if (mode === 'size') result.files[0].width = 8193
    if (mode === 'budget') result.files[0].base64 = 'A'.repeat(3 * 1024 * 1024)
    if (mode === 'signature') result.files[0].base64 = 'AAAAAAAA' + result.files[0].base64.slice(8)
    if (mode === 'decode') {
      result.files[0].base64 = Buffer.from(result.files[0].base64, 'base64').subarray(0, 16).toString('base64')
      result.files[0].byte_length = 16
    }
    await route.fulfill({ json: result })
  })
  for (mode of ['mime', 'count', 'page', 'size', 'budget', 'signature', 'decode']) {
    await panel.getByRole('button', { name: 'Print PDF', exact: true }).click()
    await expect(panel.getByRole('alert')).toContainText(/invalid|Invalid|budget|could not be loaded/)
    await expect(page.locator('[data-aislide-print]')).toHaveCount(0)
    await expect(page.locator('style[data-aislide-print-style]')).toHaveCount(0)
    await expect(panel.getByRole('button', { name: 'Prepare export', exact: true })).toBeEnabled()
  }
  mode = 'valid'
  await panel.getByRole('button', { name: 'Print PDF', exact: true }).click()
  await expect(panel.getByRole('button', { name: 'Open print dialog', exact: true })).toBeEnabled()
  const imageUrl = await page.locator('[data-aislide-print] img').getAttribute('src')
  await page.evaluate(() => { window.print = () => {} })
  await panel.getByRole('button', { name: 'Open print dialog', exact: true }).click()
  await expect(panel.getByRole('alert')).toContainText('did not confirm a print dialog')
  await expect(panel.getByRole('status')).toBeEmpty()
  await expect(page.locator('[data-aislide-print]')).toHaveCount(0)
  expect(await page.evaluate(async url => { try { await fetch(url!); return false } catch { return true } }, imageUrl)).toBe(true)
  await expect(panel.getByRole('button', { name: 'Close dialog', exact: true })).toBeEnabled()
});

test('print drops late preparation after session replacement, revision change and unmount', async ({ page }) => {
  const entry = '\0print-lifetime-harness.tsx'
  const component = `${process.cwd().replaceAll('\\', '/')}/apps/studio/src/ExportPanel.tsx`
  const result = await build({ configFile: false, logLevel: 'silent', plugins: [{ name: 'print-lifetime-harness',
    resolveId: id => id === 'virtual:print-lifetime-harness' ? entry : undefined,
    load: id => id === entry ? `
      import React from 'react';
      import { createRoot } from 'react-dom/client';
      import { ExportPanel } from ${JSON.stringify(component)};
      const root = createRoot(document.getElementById('test-root'));
      let release;
      let session;
      let busy = false;
      let allocations = 0;
      const create = URL.createObjectURL;
      URL.createObjectURL = blob => { allocations++; return create(blob); };
      const onBusy = value => { busy = value; };
      function replace() {
        session = { revision: 0, exportStatic: async options => options.format === 'pdf'
          ? { files: [{ mime_type: 'application/pdf', filename: 'report.pdf', page_indices: [0], width: 1, height: 1, byte_length: 1, base64: 'AA==' }], warnings: [] }
          : new Promise(resolve => { release = () => resolve({ files: [], warnings: [] }); }) };
        root.render(<React.StrictMode><ExportPanel session={session} pageIndex={0} onBusy={onBusy} /></React.StrictMode>);
      }
      window.printLifetime = { replace, release: () => release(), revise: () => session.revision++, unmount: () => root.unmount(), status: () => ({ busy, allocations }) };
      replace();
    ` : undefined }], build: { write: false, minify: false, rolldownOptions: { input: 'virtual:print-lifetime-harness' } } })
  const output = Array.isArray(result) ? result[0] : result
  if (!('output' in output) || output.output[0].type !== 'chunk') throw new Error('Print harness did not bundle')
  await page.route('**/__print_lifetime__', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="frame-src &apos;none&apos;"></head><body><main id="test-root"></main><script type="module" src="/__print_lifetime__.js"></script></body></html>' }))
  await page.route('**/__print_lifetime__.js', route => route.fulfill({ contentType: 'text/javascript', body: output.output[0].type === 'chunk' ? output.output[0].code : '' }))
  await page.goto('/__print_lifetime__')
  for (const action of ['replace', 'revise', 'unmount'] as const) {
    await page.getByRole('button', { name: 'Prepare export', exact: true }).click()
    await page.getByRole('button', { name: 'Print PDF', exact: true }).click()
    await expect(page.getByRole('status')).toHaveText('Export operation pending')
    await page.evaluate(actionName => {
      const fixture = (window as unknown as { printLifetime: Record<string, () => void> }).printLifetime
      fixture[actionName]()
    }, action)
    await page.evaluate(() => (window as unknown as { printLifetime: { release: () => void } }).printLifetime.release())
    await expect.poll(() => page.evaluate(() => (window as unknown as { printLifetime: { status: () => { busy: boolean; allocations: number } } }).printLifetime.status())).toEqual({ busy: false, allocations: 0 })
    await expect(page.locator('[data-aislide-print], style[data-aislide-print-style], iframe')).toHaveCount(0)
    await expect(page.getByRole('alert')).toHaveCount(0)
  }
});