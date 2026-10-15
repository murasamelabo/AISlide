import { expect, test } from '@playwright/test'
import { build } from 'vite'
import type { AislideDocument } from '../../packages/client/types'
import type { RecoveryStore, createRecoveryScheduler, createRecoveryStore } from '../../apps/studio/src/recovery'

declare global {
  interface Window {
    recoveryTest: {
      store: RecoveryStore
      snapshot: (id?: string, revision?: number) => AislideDocument
      restored: AislideDocument[]
      validations: number
      rejectValidation: boolean
      denyRestore: boolean
      createScheduler: typeof createRecoveryScheduler
      createStore: typeof createRecoveryStore
      mutate: (id: string, patch: Record<string, unknown>) => Promise<void>
    }
  }
}

let harness: string
test.beforeAll(async () => {
  const entry = '\0recovery-harness.tsx'
  const root = process.cwd().replaceAll('\\', '/')
  const result = await build({
    configFile: false, logLevel: 'silent',
    plugins: [{ name: 'recovery-harness', resolveId: id => id === 'virtual:recovery-harness' ? entry : undefined,
      load: id => id === entry ? `
        import React, { useState } from 'react';
        import { createRoot } from 'react-dom/client';
        import { RecoveryPanel } from ${JSON.stringify(`${root}/apps/studio/src/RecoveryPanel.tsx`)};
        import { createRecoveryStore, createRecoveryScheduler, RECOVERY_DB_NAME } from ${JSON.stringify(`${root}/apps/studio/src/recovery.ts`)};
        const snapshot = (id = 'local-document', revision = 0) => ({
          version: 1, id, revision, hash: (revision + 1).toString(16).padStart(64, '0'),
          deck: { version: 1, title: 'Synthetic recovery fixture', width: 1280, height: 720, slides: [] },
          sources: [], bindings: []
        });
        const store = createRecoveryStore();
        window.recoveryTest = { store, snapshot, restored: [], validations: 0, rejectValidation: false, denyRestore: false,
          createScheduler: createRecoveryScheduler, createStore: createRecoveryStore,
          mutate: (id, patch) => new Promise((resolve, reject) => {
            const opening = indexedDB.open(RECOVERY_DB_NAME, 1);
            opening.onerror = () => reject(opening.error);
            opening.onsuccess = () => {
              const database = opening.result;
              const transaction = database.transaction('snapshots', 'readwrite');
              const records = transaction.objectStore('snapshots');
              const reading = records.get(id);
              reading.onsuccess = () => records.put({ ...reading.result, ...patch });
              transaction.oncomplete = () => { database.close(); resolve(); };
              transaction.onabort = () => { database.close(); reject(transaction.error); };
            };
          })
        };
        function Harness() {
          const [enabled, setEnabled] = useState(false);
          return <RecoveryPanel enabled={enabled} onEnabledChange={setEnabled} store={store}
            validate={async document => {
              window.recoveryTest.validations++;
              if (window.recoveryTest.rejectValidation) throw new Error('Synthetic core rejection');
              return structuredClone(document);
            }}
            onRestore={async document => {
              if (window.recoveryTest.denyRestore) throw new Error('Synthetic dirty guard');
              window.recoveryTest.restored.push(document);
            }} />;
        }
        createRoot(document.getElementById('recovery-root')).render(<Harness />);
      ` : undefined,
    }],
    build: { write: false, minify: false, rolldownOptions: { input: 'virtual:recovery-harness' } },
  })
  const output = Array.isArray(result) ? result[0] : result
  if (!('output' in output) || output.output[0].type !== 'chunk') throw new Error('Recovery harness did not bundle')
  harness = output.output[0].code
})

test.beforeEach(async ({ page }) => {
  await page.route('**/__recovery_harness__', route => route.fulfill({
    contentType: 'text/html', body: '<!doctype html><html lang="en"><head><meta name="viewport" content="width=device-width,initial-scale=1"><title>Recovery test</title><link rel="stylesheet" href="/src/studio.css"></head><body><main id="recovery-root"></main><script type="module" src="/__recovery_bundle__.js"></script></body></html>',
  }))
  await page.route('**/__recovery_bundle__.js', route => route.fulfill({ contentType: 'text/javascript', body: harness }))
  await page.goto('/__recovery_harness__')
  await expect(page.getByRole('checkbox')).toBeVisible()
})

test('recovery is opt-in, stores only a basename, and survives a committed save and reload', async ({ page }) => {
  await expect(page.getByRole('checkbox')).not.toBeChecked()
  expect(await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    return { enabled: await store.config(), saved: await store.save(snapshot(), 'private.pptx'), entries: await store.list() }
  })).toEqual({ enabled: false, saved: null, entries: [] })
  await page.getByRole('checkbox').click()
  await expect(page.getByRole('checkbox')).toBeChecked()
  const saved = await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    const document = snapshot('exact-document', 7)
    const saving = store.save(document, 'C:\\sensitive\\source\\exact.pptx')
    document.deck.title = 'Changed after scheduling'
    return saving
  })
  expect(saved).toMatchObject({ id: 'exact-document', revision: 7, filename: 'exact.pptx', hash: '8'.padStart(64, '0') })
  await page.reload()
  expect(await page.evaluate(() => window.recoveryTest.store.config())).toBe(true)
  const restored = await page.evaluate(async () => {
    const { store } = window.recoveryTest
    return store.load('exact-document', async document => structuredClone(document))
  })
  expect(restored.document.deck.title).toBe('Synthetic recovery fixture')
  expect(restored.document.hash).toBe(saved!.hash)
  expect(restored.document.revision).toBe(7)
  expect(Object.keys(restored.document)).not.toContain('history')
})

test('latest five documents are bounded, deduplicated, and oversized writes preserve existing entries', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    const first = await store.save(snapshot('document-0'), 'zero.pptx')
    const duplicate = await store.save(snapshot('document-0'), 'zero.pptx')
    for (let index = 1; index < 7; index++) await store.save(snapshot(`document-${index}`), `${index}.pptx`)
    const before = await store.list()
    const oversized = snapshot('too-large')
    oversized.deck.title = 'x'.repeat(2 * 1024 * 1024)
    let rejected = false
    try { await store.save(oversized, 'large.pptx') } catch { rejected = true }
    return { first, duplicate, before, after: await store.list(), rejected }
  })
  expect(result.duplicate).toEqual(result.first)
  expect(result.before.map(entry => entry.id).sort()).toEqual(['document-2', 'document-3', 'document-4', 'document-5', 'document-6'])
  expect(result.before.reduce((bytes, entry) => bytes + entry.byteLength, 0)).toBeLessThanOrEqual(10 * 1024 * 1024)
  expect(result.rejected).toBe(true)
  expect(result.after).toEqual(result.before)
})

test('restore calls the validator and a core rejection or corruption keeps the current session', async ({ page }) => {
  await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    await store.save(snapshot(), 'candidate.pptx')
    window.recoveryTest.rejectValidation = true
  })
  await page.getByRole('button', { name: 'Refresh recovery' }).click()
  await page.getByRole('button', { name: 'Restore candidate.pptx', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('validation')
  expect(await page.evaluate(() => ({ restored: window.recoveryTest.restored.length, validations: window.recoveryTest.validations }))).toEqual({ restored: 0, validations: 1 })
  await page.evaluate(async () => {
    window.recoveryTest.rejectValidation = false
    await window.recoveryTest.mutate('local-document', { hash: 'f'.repeat(64) })
  })
  await page.getByRole('button', { name: 'Restore candidate.pptx', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('invalid')
  expect(await page.evaluate(() => window.recoveryTest.restored.length)).toBe(0)
})

test('debounce keeps the latest immutable snapshot, flush waits for commit, and cancel drops pending work', async ({ page }) => {
  const frozenTime = new Date('2026-09-17T00:00:00Z')
  await page.clock.install({ time: frozenTime })
  await page.clock.pauseAt(frozenTime)
  expect(await page.evaluate(async () => {
    const { store, snapshot, createScheduler } = window.recoveryTest
    await store.config(true)
    const scheduler = createScheduler({ store, onError: () => { throw new Error('Unexpected recovery failure') } })
    Object.assign(window.recoveryTest, { scheduler })
    for (let revision = 0; revision < 80; revision++) scheduler.schedule(snapshot('coalesced', revision), 'latest.pptx')
    const latest = snapshot('coalesced', 80)
    scheduler.schedule(latest, 'latest.pptx')
    latest.deck.title = 'Mutation after scheduling'
    return store.list()
  })).toEqual([])
  await page.clock.fastForward(249)
  expect(await page.evaluate(() => window.recoveryTest.store.list())).toEqual([])
  await page.clock.fastForward(1)
  await expect.poll(() => page.evaluate(async () => (await window.recoveryTest.store.list())[0]?.revision)).toBe(80)
  const result = await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    const scheduler = (window.recoveryTest as typeof window.recoveryTest & { scheduler: ReturnType<typeof createRecoveryScheduler> }).scheduler
    const loaded = await store.load('coalesced', async document => document)
    scheduler.schedule(snapshot('cancelled'), 'cancelled.pptx')
    scheduler.cancel()
    await scheduler.flush()
    scheduler.schedule(snapshot('flushed'), 'flushed.pptx')
    await scheduler.flush()
    scheduler.dispose()
    return { title: loaded.document.deck.title, pending: scheduler.pending, ids: (await store.list()).map(entry => entry.id).sort() }
  })
  expect(result).toEqual({ title: 'Synthetic recovery fixture', pending: false, ids: ['coalesced', 'flushed'] })
})

test('turning off persists consent and blocks a save already preparing; copies remain until explicit discard', async ({ page }) => {
  await page.getByRole('checkbox').click()
  await expect(page.getByRole('checkbox')).toBeChecked()
  await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.save(snapshot('retained'), 'retained.pptx')
  })
  await page.getByRole('button', { name: 'Refresh recovery' }).click()
  await page.getByRole('checkbox').click()
  await expect(page.getByRole('checkbox')).not.toBeChecked()
  expect(await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    return { enabled: await store.config(), saved: await store.save(snapshot('blocked'), 'blocked.pptx'), count: (await store.list()).length }
  })).toEqual({ enabled: false, saved: null, count: 1 })
  await expect(page.getByRole('button', { name: 'Restore retained.pptx' })).toBeEnabled()
  await page.getByRole('button', { name: 'Discard retained.pptx' }).click()
  await expect(page.getByRole('status')).toHaveText('0 recovery copies')
  await page.reload()
  expect(await page.evaluate(() => window.recoveryTest.store.config())).toBe(false)
})

test('quota failure rolls back eviction and reports failure without leaking document contents', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    for (let index = 0; index < 5; index++) await store.save(snapshot(`retained-${index}`), 'retained.pptx')
    const before = await store.list()
    const original = IDBObjectStore.prototype.put
    let error = ''
    IDBObjectStore.prototype.put = function (...args) {
      if (this.name === 'snapshots') throw new DOMException('Do not expose synthetic secret', 'QuotaExceededError')
      return original.apply(this, args)
    }
    try { await store.save(snapshot('sixth'), 'sixth.pptx') } catch (reason) { error = (reason as Error).message }
    finally { IDBObjectStore.prototype.put = original }
    return { before, after: await store.list(), error }
  })
  expect(result.after).toEqual(result.before)
  expect(result.error).toContain('quota')
  expect(result.error).not.toContain('synthetic secret')
})

test('storage permission failures are visible and delete all touches only the owned snapshot store', async ({ page }) => {
  const permission = await page.evaluate(async () => {
    const { createStore } = window.recoveryTest
    try { await createStore(() => { throw new DOMException('hidden', 'SecurityError') }).config() }
    catch (reason) { return (reason as Error).message }
  })
  expect(permission).toContain('permission')
  await page.getByRole('checkbox').click()
  await expect(page.getByRole('checkbox')).toBeChecked()
  await page.evaluate(async () => {
    localStorage.setItem('unrelated-app', 'keep')
    const { store, snapshot } = window.recoveryTest
    await store.save(snapshot(), 'delete-me.pptx')
  })
  await page.getByRole('button', { name: 'Refresh recovery' }).click()
  await page.getByRole('button', { name: 'Delete all recovery copies' }).click()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  expect(await page.evaluate(async () => (await window.recoveryTest.store.list()).length)).toBe(1)
  await page.getByRole('button', { name: 'Delete all recovery copies' }).click()
  await page.getByRole('button', { name: 'Confirm delete all' }).click()
  await expect(page.getByRole('status')).toHaveText('0 recovery copies')
  expect(await page.evaluate(async () => ({ enabled: await window.recoveryTest.store.config(), unrelated: localStorage.getItem('unrelated-app') }))).toEqual({ enabled: true, unrelated: 'keep' })
})

test('origin and source bytes stay inside the snapshot; no source, download, or network write occurs', async ({ page }) => {
  const requests: string[] = []
  const downloads: string[] = []
  page.on('request', request => requests.push(request.url()))
  page.on('download', download => downloads.push(download.suggestedFilename()))
  const result = await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    const document = snapshot()
    document.origin = { base64: 'UEsDBAoAAAAAAA==', sha256: 'a'.repeat(64), native: true }
    document.sources = [{ version: 1, id: 'source', name: 'Synthetic source', format: 'text', sha256: 'b'.repeat(64), byte_length: 6, content_sha256: 'c'.repeat(64), tables: [], text: 'SECRET', warnings: [], pages: [] }]
    const entry = await store.save(document, 'https://example.invalid/private/fixture.pptx')
    const loaded = await store.load(document.id, async candidate => candidate)
    return { equal: JSON.stringify(loaded.document) === JSON.stringify(document), originalHash: entry?.originalHash, filename: loaded.filename, frozen: Object.isFrozen(loaded.document.sources[0]) }
  })
  expect(result).toEqual({ equal: true, originalHash: 'a'.repeat(64), filename: 'fixture.pptx', frozen: true })
  expect(requests).toEqual([])
  expect(downloads).toEqual([])
})

test('stale selection, changed validator output, unsupported schema, and protected origins fail closed', async ({ page }) => {
  const results = await page.evaluate(async () => {
    const { store, snapshot, mutate } = window.recoveryTest
    await store.config(true)
    const first = (await store.save(snapshot(), 'first.pptx'))!
    await store.save(snapshot('local-document', 1), 'second.pptx')
    const failures: string[] = []
    try { await store.load(first.id, async document => document, first) } catch (reason) { failures.push((reason as Error).message) }
    try { await store.load(first.id, async document => ({ ...document, revision: 0 })) } catch (reason) { failures.push((reason as Error).message) }
    await mutate(first.id, { schemaVersion: 99 })
    try { await store.list() } catch (reason) { failures.push((reason as Error).message) }
    await store.clear()
    const protectedDocument = snapshot()
    protectedDocument.origin = { base64: '0M8R4KGxGuEAAAAA', sha256: 'a'.repeat(64) }
    try { await store.save(protectedDocument, 'protected.pptx') } catch (reason) { failures.push((reason as Error).message) }
    return { failures, entries: await store.list() }
  })
  expect(results.failures).toHaveLength(4)
  expect(results.failures[0]).toContain('selection changed')
  expect(results.failures[1]).toContain('validation')
  expect(results.entries).toEqual([])
})

test('UTF-8 envelope budgets hold near 10 MiB and on concurrent saves', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    for (let index = 0; index < 6; index++) {
      const document = snapshot(`large-${index}`)
      document.deck.title = 'x'.repeat(2 * 1024 * 1024 - 2048)
      await store.save(document, 'large.pptx')
    }
    const nearLimit = await store.list()
    await Promise.all(Array.from({ length: 10 }, (_, index) => store.save(snapshot(`concurrent-${index}`), 'small.pptx')))
    const multibyte = snapshot('multibyte')
    multibyte.deck.title = '\u3042'.repeat(800_000)
    let rejected = false
    try { await store.save(multibyte, 'too-large.pptx') } catch { rejected = true }
    return { nearLimit: nearLimit.map(entry => entry.byteLength), final: await store.list(), rejected }
  })
  expect(result.nearLimit).toHaveLength(5)
  expect(result.nearLimit.reduce((total, bytes) => total + bytes, 0)).toBeGreaterThan(9 * 1024 * 1024)
  expect(result.nearLimit.reduce((total, bytes) => total + bytes, 0)).toBeLessThanOrEqual(10 * 1024 * 1024)
  expect(result.final).toHaveLength(5)
  expect(result.final.every(entry => entry.byteLength <= 2 * 1024 * 1024)).toBe(true)
  expect(result.rejected).toBe(true)
})

test('deduplication repairs a corrupted stored document instead of reporting false success', async ({ page }) => {
  expect(await page.evaluate(async () => {
    const { store, snapshot, mutate } = window.recoveryTest
    await store.config(true)
    const document = snapshot()
    await store.save(document, 'original.pptx')
    const corrupted = structuredClone(document)
    corrupted.deck.title = 'Corrupted without changing the claimed hash'
    await mutate(document.id, { document: corrupted })
    await store.save(document, 'original.pptx')
    return (await store.load(document.id, async candidate => candidate)).document.deck.title
  })).toBe('Synthetic recovery fixture')
})

test('core-validated object key reordering preserves identity, revision, and exact values', async ({ page }) => {
  expect(await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    const document = snapshot()
    await store.save(document, 'original.pptx')
    const result = await store.load(document.id, async candidate => ({
      bindings: candidate.bindings, sources: candidate.sources, deck: candidate.deck,
      hash: candidate.hash, revision: candidate.revision, id: candidate.id, version: candidate.version,
    }))
    return { id: result.document.id, revision: result.document.revision, hash: result.document.hash }
  })).toEqual({ id: 'local-document', revision: 0, hash: '1'.padStart(64, '0') })
})

test('panel shows storage errors, preserves disabled consent, and respects a rejecting parent dirty guard', async ({ page }) => {
  await page.evaluate(() => {
    const { store } = window.recoveryTest
    const config = store.config
    store.config = async enabled => {
      store.config = config
      if (enabled) throw new DOMException('Do not display payloads', 'SecurityError')
      return config(enabled)
    }
  })
  await page.getByRole('checkbox').click()
  await expect(page.getByRole('alert')).toContainText('permission')
  await expect(page.getByRole('checkbox')).not.toBeChecked()
  await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    await store.save(snapshot(), 'dirty-guard.pptx')
    window.recoveryTest.denyRestore = true
  })
  await page.getByRole('button', { name: 'Refresh recovery' }).click()
  await page.getByRole('button', { name: 'Restore dirty-guard.pptx' }).click()
  await expect(page.getByRole('alert')).toBeVisible()
  expect(await page.evaluate(() => window.recoveryTest.restored)).toEqual([])
  await page.evaluate(() => { window.recoveryTest.denyRestore = false })
  await page.getByRole('button', { name: 'Restore dirty-guard.pptx' }).click()
  await expect.poll(() => page.evaluate(() => window.recoveryTest.restored.length)).toBe(1)
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('panel fits long basenames and hashes on desktop and mobile', async ({ page }, testInfo) => {
  await page.evaluate(async () => {
    const { store, snapshot } = window.recoveryTest
    await store.config(true)
    await store.save(snapshot(), `${'LongName'.repeat(30)}.pptx`)
  })
  await page.getByRole('button', { name: 'Refresh recovery' }).click()
  await expect(page.getByRole('status')).toHaveText('1 recovery copy')
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 960 })
    await page.evaluate(() => document.fonts.ready)
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    await expect(page.getByRole('button', { name: /^Restore / })).toBeVisible()
    await page.screenshot({ path: testInfo.outputPath(`recovery-${width}.png`), fullPage: true })
  }
})

test('one active save plus one latest pending snapshot stays bounded and reports scheduler failures', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { store, snapshot, createScheduler } = window.recoveryTest
    await store.config(true)
    let release!: () => void
    const hold = new Promise<void>(resolve => { release = resolve })
    const revisions: number[] = []
    const failures: string[] = []
    const scheduler = createScheduler({
      store: { ...store, save: async (document, filename) => {
        revisions.push(document.revision)
        if (revisions.length === 1) await hold
        return store.save(document, filename)
      } },
      onError: error => failures.push(error.message),
    })
    scheduler.schedule(snapshot('pending', 0), 'pending.pptx')
    const flushing = scheduler.flush()
    for (let revision = 1; revision <= 1000; revision++) scheduler.schedule(snapshot('pending', revision), 'pending.pptx')
    const pending = scheduler.pending
    release()
    await flushing
    scheduler.dispose()
    const broken = createScheduler({ store: { ...store, save: async () => { throw new DOMException('hidden payload', 'QuotaExceededError') } }, onError: error => failures.push(error.message) })
    broken.schedule(snapshot('failed'), 'failed.pptx')
    let flushRejected = false
    try { await broken.flush() } catch { flushRejected = true }
    broken.dispose()
    return { revisions, pending, after: scheduler.pending, failures, flushRejected }
  })
  expect(result.revisions).toEqual([0, 1000])
  expect(result.pending).toBe(true)
  expect(result.after).toBe(false)
  expect(result.flushRejected).toBe(true)
  expect(result.failures).toHaveLength(1)
  expect(result.failures[0]).toContain('quota')
  expect(result.failures[0]).not.toContain('hidden payload')
})

test('even matching rewritten database hashes cannot bypass the required core validator', async ({ page }) => {
  await page.evaluate(async () => {
    const { store, snapshot, mutate } = window.recoveryTest
    await store.config(true)
    const document = snapshot()
    await store.save(document, 'forged.pptx')
    document.deck.title = 'Tampered data'
    document.hash = 'f'.repeat(64)
    const serialized = JSON.stringify(document, (_key, value) => value && typeof value === 'object' && !Array.isArray(value)
      ? Object.fromEntries(Object.entries(value).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)) : value)
    const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(serialized))
    const integrity = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')
    await mutate(document.id, { document, hash: document.hash, integrity })
    window.recoveryTest.rejectValidation = true
  })
  await page.getByRole('button', { name: 'Refresh recovery' }).click()
  await expect(page.getByRole('status')).toHaveText('1 recovery copy')
  await page.getByRole('button', { name: 'Restore forged.pptx' }).click()
  await expect(page.getByRole('alert')).toContainText('validation')
  expect(await page.evaluate(() => ({ restored: window.recoveryTest.restored.length, validations: window.recoveryTest.validations }))).toEqual({ restored: 0, validations: 1 })
})