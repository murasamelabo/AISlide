import { invoke, isTauri } from '@tauri-apps/api/core'
import { core } from './api'
import { encodeCoreRequest } from '../../../packages/client/index.mjs'
import type { CoreTransport, SessionRecovery } from '../../../packages/client/index.mjs'

export const SESSION_RECOVERY_DB = 'aislide.recovery.v2'
export type SessionRecoveryEntry = { id: string; filename: string; hash: string; revision: number; saved_at: number; byte_length: number; integrity: string }
type State = { version: 2; enabled: boolean; generation: number; consent_epoch: number; entries: SessionRecoveryEntry[] }
type Action = { op: 'open' } | { op: 'configure'; enabled: boolean } | { op: 'save'; envelope: SessionRecovery; filename: string } | { op: 'remove'; id: string } | { op: 'clear' }
type Transition = { state: State; write: { entry: SessionRecoveryEntry; payload: string } | null; deleted: string[] }
const initial = (): State => ({ version: 2, enabled: false, generation: 0, consent_epoch: 0, entries: [] })
const conflict = () => new Error('Recovery changed in another window. Refresh before saving or deleting.')
function checkSignal(signal?: AbortSignal) { if (signal?.aborted) throw new DOMException('Recovery cancelled', 'AbortError') }

function database(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(SESSION_RECOVERY_DB, 1)
    let blocked = false
    request.onupgradeneeded = () => { request.result.createObjectStore('state'); request.result.createObjectStore('slots') }
    request.onerror = () => reject(request.error)
    request.onblocked = () => { blocked = true; reject(new Error('Recovery database blocked by another window.')) }
    request.onsuccess = () => {
      if (blocked) { request.result.close(); return }
      request.result.onversionchange = () => request.result.close()
      resolve(request.result)
    }
  })
}

async function transaction<Value>(mode: IDBTransactionMode, action: (transaction: IDBTransaction, result: (value: Value) => void, fail: (error: unknown) => void) => void, signal?: AbortSignal): Promise<Value> {
  checkSignal(signal)
  const connection = await database()
  try {
    checkSignal(signal)
    return await new Promise<Value>((resolve, reject) => {
      const current = connection.transaction(['state', 'slots'], mode)
      let value: Value
      let error: unknown
      const abort = () => { error = new DOMException('Recovery cancelled', 'AbortError'); current.abort() }
      signal?.addEventListener('abort', abort, { once: true })
      const cleanup = () => signal?.removeEventListener('abort', abort)
      current.oncomplete = () => { cleanup(); resolve(value) }
      current.onabort = () => { cleanup(); reject(error ?? current.error ?? new Error('Recovery write failed; last checkpoint retained.')) }
      const fail = (reason: unknown) => { error = reason; current.abort() }
      try { action(current, next => { value = next }, fail) } catch (reason) { fail(reason) }
    })
  } finally { connection.close() }
}

async function nativeRequest<Value>(request: unknown, signal?: AbortSignal): Promise<Value> {
  checkSignal(signal)
  const operationId = crypto.randomUUID()
  const pending = invoke<Value>('recovery_request', { request, operationId })
  let cancellation: Promise<unknown> | undefined
  const cancel = () => { cancellation = invoke('cancel_recovery_request', { operationId }).catch(() => false) }
  signal?.addEventListener('abort', cancel, { once: true })
  try { const value = await pending; checkSignal(signal); return value }
  finally { signal?.removeEventListener('abort', cancel); await cancellation }
}

export interface SessionRecoveryStore {
  config(enabled?: boolean): Promise<boolean>
  list(): Promise<SessionRecoveryEntry[]>
  save(envelope: SessionRecovery, filename: string, signal?: AbortSignal): Promise<SessionRecoveryEntry | null>
  load(id: string, expected: SessionRecoveryEntry): Promise<{ envelope: SessionRecovery; filename: string }>
  remove(id: string, expected: SessionRecoveryEntry): Promise<void>
  clear(): Promise<void>
}

export function createSessionRecoveryStore({ request = core, native = isTauri() }: { request?: CoreTransport; native?: boolean } = {}): SessionRecoveryStore {
  let known: State | null = null
  const read = () => native ? nativeRequest<State>({ op: 'state' }) : transaction<State>('readonly', (current, result) => {
    const reading = current.objectStore('state').get('current')
    reading.onsuccess = () => result(reading.result ?? initial())
  })
  async function transition(action: Action, signal?: AbortSignal): Promise<State> {
    checkSignal(signal)
    const before = structuredClone(known ?? await read())
    if (native) {
      const state = await nativeRequest<State>({ op: 'transition', expected_generation: before.generation, action }, signal)
      checkSignal(signal); known = state; return state
    }
    const prepared = await request<Transition>({ op: 'prepare_recovery', state: before, expected_generation: before.generation, action, now_ms: Date.now() }, { signal })
    checkSignal(signal)
    const state = await transaction<State>('readwrite', (current, result, fail) => {
      const reading = current.objectStore('state').get('current')
      reading.onsuccess = () => {
        try {
          checkSignal(signal)
          const actual: State = reading.result ?? initial()
          if (actual.generation !== before.generation || actual.consent_epoch !== before.consent_epoch) { fail(conflict()); return }
          if (prepared.state.generation !== before.generation) {
            const slots = current.objectStore('slots')
            if (prepared.write) slots.put(prepared.write.payload, prepared.write.entry.integrity)
            for (const integrity of prepared.deleted) slots.delete(integrity)
            current.objectStore('state').put(prepared.state, 'current')
          }
          result(prepared.state)
        } catch (reason) { fail(reason) }
      }
    }, signal)
    known = state
    return state
  }
  function selected(id: string, expected: SessionRecoveryEntry) {
    const entry = known?.entries.find(entry => entry.id === id)
    if (!entry || JSON.stringify(entry) !== JSON.stringify(expected)) throw conflict()
    return entry
  }
  const operations: SessionRecoveryStore = {
    async config(enabled) {
      if (enabled === undefined) { known = await read(); return known.enabled }
      return (await transition({ op: 'configure', enabled })).enabled
    },
    async list() {
      known = await read()
      const state = await transition({ op: 'open' })
      return structuredClone(state.entries).sort((left, right) => right.saved_at - left.saved_at)
    },
    async save(envelope, filename, signal) {
      if (!known) known = await read()
      if (!known.enabled) return null
      encodeCoreRequest({ op: 'verify_session_recovery', envelope })
      const id = envelope.document.id
      const state = await transition({ op: 'save', envelope: structuredClone(envelope), filename }, signal)
      return structuredClone(state.entries.find(entry => entry.id === id) ?? null)
    },
    async load(id, expected) {
      const entry = selected(id, expected)
      const generation = known!.generation
      if (native) return { envelope: await nativeRequest<SessionRecovery>({ op: 'load', id, expected_generation: generation }), filename: entry.filename }
      const payload = await transaction<string>('readonly', (current, result, fail) => {
        const reading = current.objectStore('state').get('current')
        reading.onsuccess = () => {
          if ((reading.result ?? initial()).generation !== generation) { fail(conflict()); return }
          const slot = current.objectStore('slots').get(entry.integrity)
          slot.onsuccess = () => typeof slot.result === 'string' ? result(slot.result) : fail(new Error('Recovery payload missing.'))
        }
      })
      const envelope = await request<SessionRecovery>({ op: 'verify_recovery_record', entry, payload })
      return { envelope, filename: entry.filename }
    },
    async remove(id, expected) { selected(id, expected); await transition({ op: 'remove', id }) },
    async clear() { await transition({ op: 'clear' }) },
  }
  let pending: Promise<unknown> = Promise.resolve()
  function enqueue<Value>(action: () => Promise<Value>): Promise<Value> {
    const next = pending.then(action, action)
    pending = next.catch(() => undefined)
    return next
  }
  return {
    config: enabled => enqueue(() => operations.config(enabled)),
    list: () => enqueue(() => operations.list()),
    save(envelope, filename, signal) {
      encodeCoreRequest({ envelope })
      const snapshot = structuredClone(envelope)
      return enqueue(() => operations.save(snapshot, filename, signal))
    },
    load: (id, expected) => enqueue(() => operations.load(id, expected)),
    remove: (id, expected) => enqueue(() => operations.remove(id, expected)),
    clear: () => enqueue(() => operations.clear()),
  }
}

export const sessionRecoveryStore = createSessionRecoveryStore()

export function createSessionRecoveryScheduler({ store = sessionRecoveryStore, onError, onSaved }: {
  store?: SessionRecoveryStore; onError: (error: Error) => void; onSaved?: (entry: SessionRecoveryEntry | null) => void
}) {
  let queued: { envelope: SessionRecovery; filename: string } | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  let running: Promise<void> | null = null
  let controller: AbortController | null = null
  let generation = 0
  let disposed = false
  function cancel() { generation++; clearTimeout(timer); timer = undefined; queued = null; controller?.abort() }
  function flush(): Promise<void> {
    clearTimeout(timer); timer = undefined
    if (running) return running
    if (!queued || disposed) return Promise.resolve()
    running = (async () => {
      while (queued && !disposed) {
        const next = queued; queued = null
        const operation = generation
        controller = new AbortController()
        try {
          const entry = await store.save(next.envelope, next.filename, controller.signal)
          if (!disposed && operation === generation) onSaved?.(entry)
        } catch (reason) {
          if (!disposed && operation === generation) onError(reason instanceof Error ? reason : new Error(String(reason)))
        } finally { controller = null }
      }
    })().finally(() => { running = null })
    return running
  }
  return {
    get pending() { return Boolean(queued || running) },
    schedule(envelope: SessionRecovery, filename: string) {
      if (disposed) throw new Error('Recovery scheduler disposed.')
      try { encodeCoreRequest({ envelope }); queued = { envelope: structuredClone(envelope), filename } }
      catch (reason) { cancel(); onError(reason instanceof Error ? reason : new Error(String(reason))); return }
      clearTimeout(timer); timer = setTimeout(() => { void flush() }, 250)
    },
    cancel, flush,
    dispose() { disposed = true; cancel() },
  }
}

export type SessionRecoveryScheduler = ReturnType<typeof createSessionRecoveryScheduler>