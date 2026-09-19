import { expect, test } from '@playwright/test'

test('phase5 Studio restores both Undo and Redo through a reload with profile intact', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Document setup', exact: true }).click()
  const setup = page.getByRole('dialog', { name: 'Document setup', exact: true })
  await expect(setup.getByLabel('Capacity profile')).toHaveValue('large')
  await setup.getByLabel('Capacity profile').selectOption('standard')
  await setup.getByRole('button', { name: 'Apply capacity profile', exact: true }).click()
  await expect(setup).toContainText('Active: standard')
  await setup.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'Add text', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Add rectangle', exact: true }).click()
  const canvas = page.getByRole('main', { name: 'Active slide canvas' })
  await expect(canvas.locator('[data-element-id]')).toHaveCount(2)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(canvas.locator('[data-element-id]')).toHaveCount(1)
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  const recovery = page.getByRole('dialog', { name: 'Local recovery', exact: true })
  await recovery.getByRole('checkbox').click()
  await expect(recovery.getByRole('checkbox')).toBeChecked()
  await expect(page.getByRole('status').filter({ hasText: 'Recovery copy saved' })).toBeVisible()
  await recovery.getByRole('button', { name: 'Refresh recovery', exact: true }).click()
  await expect(recovery.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true })).toBeVisible()
  await recovery.getByRole('checkbox').click()
  await expect(recovery.getByRole('checkbox')).not.toBeChecked()
  await page.setViewportSize({ width: 390, height: 844 })
  await expect.poll(() => recovery.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.screenshot({ path: '.artifacts/phase5-recovery-mobile.png' })
  await page.setViewportSize({ width: 1440, height: 960 })
  await page.reload()
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Local recovery', exact: true }).click()
  await expect(recovery.getByRole('checkbox')).not.toBeChecked()
  await recovery.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true }).click()
  await expect(canvas.locator('[data-element-id]')).toHaveCount(1)
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled()
  await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Redo', exact: true }).click()
  await expect(canvas.locator('[data-element-id]')).toHaveCount(2)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(canvas.locator('[data-element-id]')).toHaveCount(1)
  await page.getByRole('button', { name: 'Undo', exact: true }).click()
  await expect(canvas.locator('[data-element-id]')).toHaveCount(0)
  await page.getByRole('button', { name: 'Document setup', exact: true }).click()
  await expect(setup.getByLabel('Capacity profile')).toHaveValue('standard')
  await page.screenshot({ path: '.artifacts/phase5-capacity-desktop.png' })
  await setup.getByRole('button', { name: 'Close dialog', exact: true }).click()
  await page.getByRole('button', { name: 'New presentation', exact: true }).click()
  await page.getByRole('button', { name: 'Discard changes', exact: true }).click()
  await page.getByRole('button', { name: 'Document setup', exact: true }).click()
  await expect(setup.getByLabel('Capacity profile')).toHaveValue('standard')
})

test('phase5 recovery v2 persists verified histories and rejects stale cross-window saves', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  const result = await page.evaluate(async () => {
    const recoveryModule = '/src/recovery-v2.ts'
    const apiModule = '/src/api.ts'
    const { createSessionRecoveryStore } = await import(recoveryModule)
    const { core } = await import(apiModule)
    const document = await core({ op: 'create_presentation', id: 'browser-recovery', title: 'Initial' })
    const changed = await core({ op: 'transaction', document, transaction: { expected_revision: 0, expected_hash: document.hash,
      operations: [{ op: 'replace', path: '/deck/title', value: 'Changed' }] } })
    const envelope = { format: 'aislide.session', version: 1, capacity_profile: 'large', document: changed.document,
      past: [changed.receipt], future: [], history_boundary: null }
    const first = createSessionRecoveryStore()
    const second = createSessionRecoveryStore()
    const defaultOff = await first.config()
    await first.config(true)
    await second.list()
    await first.save(envelope, 'C:\\private\\work.pptx')
    let conflict = ''
    try { await second.save(envelope, 'stale.pptx') } catch (error) { conflict = String(error) }
    const restarted = createSessionRecoveryStore()
    const entries = await restarted.list()
    const loaded = await restarted.load(entries[0].id, entries[0])
    const undone = await core({ op: 'undo_transaction', document: loaded.envelope.document,
      expected_revision: loaded.envelope.document.revision, receipt: loaded.envelope.past[0] })
    await restarted.remove(entries[0].id, entries[0])
    let resurrect = ''
    try { await first.save(envelope, 'resurrect.pptx') } catch (error) { resurrect = String(error) }
    return { defaultOff, conflict, resurrect, title: undone.document.deck.title, filename: loaded.filename, count: (await restarted.list()).length }
  })
  expect(result.defaultOff).toBe(false)
  expect(result.conflict).toMatch(/changed|refresh/i)
  expect(result.resurrect).toMatch(/changed|refresh/i)
  expect(result.title).toBe('Initial')
  expect(result.filename).toBe('work.pptx')
  expect(result.count).toBe(0)
})

test('phase5 cancelled and disposed in-flight saves cannot write or notify after disable', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  const results = await page.evaluate(async () => {
    const recoveryModule = '/src/recovery-v2.ts'
    const apiModule = '/src/api.ts'
    const { createSessionRecoveryStore, createSessionRecoveryScheduler } = await import(recoveryModule)
    const { core } = await import(apiModule)
    const document = await core({ op: 'create_presentation', id: 'cancel-recovery', title: 'Good checkpoint' })
    const envelope = { format: 'aislide.session', version: 1, capacity_profile: 'large', document, past: [], future: [], history_boundary: null }
    const output = []
    for (const mode of ['cancel', 'dispose']) {
      let release!: () => void
      let started!: () => void
      let delay = false
      const barrier = new Promise<void>(resolve => { release = resolve })
      const entered = new Promise<void>(resolve => { started = resolve })
      const store = createSessionRecoveryStore({ request: async (request: { action?: { op: string } }, options: unknown) => {
        if (delay && request.action?.op === 'save') { started(); await barrier }
        return core(request, options)
      } })
      await store.config(); await store.config(true)
      await store.save(envelope, 'good.pptx')
      const before = await store.list()
      let callbacks = 0
      const scheduler = createSessionRecoveryScheduler({ store, onSaved: () => callbacks++, onError: () => callbacks++ })
      delay = true
      scheduler.schedule(envelope, 'late.pptx')
      const pending = scheduler.flush()
      await entered
      scheduler[mode]()
      release(); await pending
      await store.config(false)
      const after = await store.list()
      output.push({ same: JSON.stringify(before) === JSON.stringify(after), callbacks, enabled: await store.config() })
      scheduler.dispose()
    }
    return output
  })
  expect(results).toEqual([{ same: true, callbacks: 0, enabled: false }, { same: true, callbacks: 0, enabled: false }])
})

test('phase5 v2 consent and clearing never migrate or delete legacy v1 copies', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled()
  const result = await page.evaluate(async () => {
    const legacyModule = '/src/recovery.ts'
    const nextModule = '/src/recovery-v2.ts'
    const apiModule = '/src/api.ts'
    const { createRecoveryStore } = await import(legacyModule)
    const { createSessionRecoveryStore } = await import(nextModule)
    const { core } = await import(apiModule)
    const document = await core({ op: 'create_presentation', id: 'legacy-retained', title: 'Legacy copy' })
    const old = createRecoveryStore(); await old.config(true); await old.save(document, 'legacy.pptx')
    const before = await old.list()
    const current = createSessionRecoveryStore(); const defaultOff = await current.config()
    await current.config(true); await current.list(); await current.clear(); await current.config(false)
    return { defaultOff, retained: JSON.stringify(before) === JSON.stringify(await old.list()) }
  })
  expect(result).toEqual({ defaultOff: false, retained: true })
})