import type { AislideDocument } from './types'

export const RECOVERY_DB_NAME = 'aislide.recovery.v1'
export const RECOVERY_SETTINGS_KEY = 'aislide.recovery.enabled.v1'
export const MAX_RECOVERY_DOCUMENTS = 5
export const MAX_RECOVERY_SNAPSHOT_BYTES = 2 * 1024 * 1024
export const MAX_RECOVERY_TOTAL_BYTES = 10 * 1024 * 1024
export const RECOVERY_DEBOUNCE_MS = 250

export type Immutable<Value> = Value extends object ? { readonly [Key in keyof Value]: Immutable<Value[Key]> } : Value
export type RecoveryValidator = (document: AislideDocument) => Promise<AislideDocument>
export type RecoveryEntry = Readonly<{
  schemaVersion: 1
  id: string
  hash: string
  originalHash: string | null
  revision: number
  savedAt: number
  filename: string
  byteLength: number
}>
export type RecoveredWork = Readonly<{ document: Immutable<AislideDocument>; filename: string }>
export type RecoverySelection = Pick<RecoveryEntry, 'hash' | 'revision' | 'savedAt'>
export interface RecoveryStore {
  config(enabled?: boolean): Promise<boolean>
  list(): Promise<readonly RecoveryEntry[]>
  save(document: AislideDocument, filename: string): Promise<RecoveryEntry | null>
  load(id: string, validate: RecoveryValidator, expected?: RecoverySelection): Promise<RecoveredWork>
  remove(id: string, expected?: RecoverySelection): Promise<void>
  clear(): Promise<void>
}

type StoredEntry = { -readonly [Key in keyof Omit<RecoveryEntry, 'byteLength'>]: RecoveryEntry[Key] } & { document: AislideDocument; integrity: string }
type Guard = (action: () => void) => () => void
const documentKeys = new Set(['version', 'id', 'revision', 'hash', 'deck', 'sources', 'bindings', 'parts', 'report', 'origin'])
const entryKeys = new Set(['schemaVersion', 'id', 'hash', 'originalHash', 'revision', 'savedAt', 'filename', 'document', 'integrity'])
const encoder = new TextEncoder()
const sha256 = /^[a-f0-9]{64}$/

export class RecoveryError extends Error {}

export function recoveryError(reason: unknown): RecoveryError {
  if (reason instanceof RecoveryError) return reason
  const name = reason instanceof Error ? reason.name : ''
  if (name === 'QuotaExceededError') return new RecoveryError('Local recovery quota exceeded. No snapshot was committed.')
  if (name === 'SecurityError' || name === 'NotAllowedError') return new RecoveryError('Local recovery storage permission was denied.')
  return new RecoveryError('Local recovery storage is unavailable or the operation failed. No success was confirmed.')
}

function invalid(): never { throw new RecoveryError('Local recovery snapshot is invalid or exceeds its storage limits.') }
function controlCharacter(character: string): boolean {
  const code = character.charCodeAt(0)
  return code < 32 || (code >= 127 && code <= 159)
}
function identity(id: unknown): asserts id is string {
  if (typeof id !== 'string' || !id.length || id.length > 80 || Array.from(id).some(controlCharacter)) invalid()
}

function plainCopy<Value>(value: Value): Value {
  const copy = structuredClone(value)
  const pending: { value: unknown; depth: number }[] = [{ value: copy, depth: 0 }]
  let nodes = 0
  let characters = 0
  while (pending.length) {
    const current = pending.pop()!
    if (++nodes > 200_000 || current.depth > 96) invalid()
    if (typeof current.value === 'string') characters += current.value.length
    else if (typeof current.value === 'number') { if (!Number.isFinite(current.value)) invalid() }
    else if (current.value !== null && typeof current.value === 'object') {
      if (!Array.isArray(current.value) && Object.getPrototypeOf(current.value) !== Object.prototype) invalid()
      const entries = Object.entries(current.value)
      if (entries.length + pending.length > 200_000) invalid()
      for (const [key, child] of entries) {
        characters += key.length
        pending.push({ value: child, depth: current.depth + 1 })
      }
      Object.freeze(current.value)
    } else if (current.value !== null && typeof current.value !== 'boolean') invalid()
    if (characters > MAX_RECOVERY_SNAPSHOT_BYTES) invalid()
  }
  return copy
}

function documentCopy(value: AislideDocument): AislideDocument {
  const document = plainCopy(value)
  if (!document || typeof document !== 'object' || Array.isArray(document)) invalid()
  if (Object.keys(document).some(key => !documentKeys.has(key))) invalid()
  identity(document.id)
  if (document.version !== 1 || !Number.isSafeInteger(document.revision) || document.revision < 0 || !sha256.test(document.hash)) invalid()
  if (!document.deck || typeof document.deck !== 'object' || !Array.isArray(document.sources) || !Array.isArray(document.bindings)) invalid()
  if (document.origin) {
    if (typeof document.origin.base64 !== 'string' || !document.origin.base64.startsWith('UEsDB') || !sha256.test(document.origin.sha256)) invalid()
  }
  if (encoder.encode(JSON.stringify(document)).byteLength > MAX_RECOVERY_SNAPSHOT_BYTES) invalid()
  return document
}

function basename(filename: string): string {
  const name = Array.from(filename.split(/[\\/]/).pop()!).filter(character => !controlCharacter(character)).join('')
    .replace(/[<>:"|?*\u202a-\u202e\u2066-\u2069]/g, '').trim().slice(0, 255)
  return !name || name === '.' || name === '..' ? 'Untitled.pptx' : name
}

function size(entry: StoredEntry): number { return encoder.encode(JSON.stringify(entry)).byteLength }
function summary(entry: StoredEntry): RecoveryEntry {
  const { document: _document, integrity: _integrity, ...metadata } = entry
  return Object.freeze({ ...metadata, byteLength: size(entry) })
}

function checkEntry(value: unknown): StoredEntry {
  if (!value || typeof value !== 'object' || Array.isArray(value)) invalid()
  if (Object.keys(value).length !== entryKeys.size || Object.keys(value).some(key => !entryKeys.has(key))) invalid()
  const entry = value as StoredEntry
  const document = documentCopy(entry.document)
  if (entry.schemaVersion !== 1 || entry.id !== document.id || entry.hash !== document.hash || entry.revision !== document.revision) invalid()
  if (entry.originalHash !== (document.origin?.sha256 ?? null) || !sha256.test(entry.integrity)) invalid()
  if (!Number.isSafeInteger(entry.savedAt) || entry.savedAt < 0 || entry.savedAt > 8_640_000_000_000_000) invalid()
  if (typeof entry.filename !== 'string' || entry.filename !== basename(entry.filename) || size(entry) > MAX_RECOVERY_SNAPSHOT_BYTES) invalid()
  return { ...entry, document }
}

function orderedDocument(document: AislideDocument): string {
  return JSON.stringify(document, (_key, value: unknown) => value && typeof value === 'object' && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)) : value)
}

async function digest(document: AislideDocument): Promise<string> {
  const bytes = encoder.encode(orderedDocument(document))
  const hash = await globalThis.crypto.subtle.digest('SHA-256', bytes)
  return Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('')
}

function openDatabase(factory: () => IDBFactory): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    let request: IDBOpenDBRequest
    let blocked = false
    try { request = factory().open(RECOVERY_DB_NAME, 1) } catch (reason) { reject(recoveryError(reason)); return }
    request.onblocked = () => {
      blocked = true
      reject(new RecoveryError('Local recovery database is blocked by another window. Close that recovery view and retry.'))
    }
    request.onerror = () => reject(recoveryError(request.error))
    request.onupgradeneeded = event => {
      if (blocked || event.oldVersion !== 0) { request.transaction?.abort(); return }
      request.result.createObjectStore('snapshots', { keyPath: 'id' })
      request.result.createObjectStore('settings')
    }
    request.onsuccess = () => {
      const database = request.result
      if (blocked) { database.close(); return }
      database.onversionchange = () => database.close()
      resolve(database)
    }
  })
}

async function transact<Result>(factory: () => IDBFactory, stores: string[], mode: IDBTransactionMode,
  action: (transaction: IDBTransaction, result: (value: Result) => void, guard: Guard) => void): Promise<Result> {
  const database = await openDatabase(factory)
  try {
    return await new Promise<Result>((resolve, reject) => {
      const transaction = database.transaction(stores, mode)
      let value: Result
      let failure: unknown
      const guard: Guard = callback => () => {
        try { callback() } catch (reason) { failure = reason; transaction.abort() }
      }
      transaction.oncomplete = () => resolve(value)
      transaction.onabort = () => reject(recoveryError(failure ?? transaction.error))
      guard(() => action(transaction, next => { value = next }, guard))()
    })
  } catch (reason) { throw recoveryError(reason) }
  finally { database.close() }
}

function scan(store: IDBObjectStore, guard: Guard, done: (entries: StoredEntry[]) => void) {
  const entries: StoredEntry[] = []
  let bytes = 0
  const request = store.openCursor()
  request.onsuccess = guard(() => {
    const cursor = request.result
    if (!cursor) { done(entries); return }
    if (entries.length >= MAX_RECOVERY_DOCUMENTS) invalid()
    const entry = checkEntry(cursor.value)
    if (cursor.primaryKey !== entry.id) invalid()
    bytes += size(entry)
    if (bytes > MAX_RECOVERY_TOTAL_BYTES) invalid()
    entries.push(entry)
    cursor.continue()
  })
}

export function createRecoveryStore(factory: () => IDBFactory = () => globalThis.indexedDB): RecoveryStore {
  return {
    async config(enabled) {
      return transact<boolean>(factory, ['settings'], enabled === undefined ? 'readonly' : 'readwrite', (transaction, result, guard) => {
        const settings = transaction.objectStore('settings')
        if (enabled !== undefined) {
          if (typeof enabled !== 'boolean') invalid()
          settings.put({ version: 1, enabled }, RECOVERY_SETTINGS_KEY)
          result(enabled)
        } else {
          const request = settings.get(RECOVERY_SETTINGS_KEY)
          request.onsuccess = guard(() => { result(request.result?.version === 1 && request.result.enabled === true) })
        }
      })
    },
    async list() {
      const entries = await transact<StoredEntry[]>(factory, ['snapshots'], 'readonly', (transaction, result, guard) => {
        scan(transaction.objectStore('snapshots'), guard, result)
      })
      for (const entry of entries) { if (await digest(entry.document) !== entry.integrity) invalid() }
      return Object.freeze(entries.sort((left, right) => right.savedAt - left.savedAt).map(summary))
    },
    async save(document, filename) {
      const copy = documentCopy(document)
      const entry: StoredEntry = {
        schemaVersion: 1, id: copy.id, hash: copy.hash, originalHash: copy.origin?.sha256 ?? null,
        revision: copy.revision, savedAt: Date.now(), filename: basename(filename), document: copy, integrity: await digest(copy),
      }
      if (size(entry) > MAX_RECOVERY_SNAPSHOT_BYTES) invalid()
      return transact<RecoveryEntry | null>(factory, ['settings', 'snapshots'], 'readwrite', (transaction, result, guard) => {
        const consent = transaction.objectStore('settings').get(RECOVERY_SETTINGS_KEY)
        consent.onsuccess = guard(() => {
          if (consent.result?.version !== 1 || consent.result.enabled !== true) { result(null); return }
          const records = transaction.objectStore('snapshots')
          scan(records, guard, entries => {
            const existing = entries.find(candidate => candidate.id === entry.id)
            if (existing?.hash === entry.hash && existing.revision === entry.revision && existing.integrity === entry.integrity && existing.filename === entry.filename
              && orderedDocument(existing.document) === orderedDocument(entry.document)) {
              result(summary(existing)); return
            }
            entry.savedAt = Math.max(entry.savedAt, ...entries.map(candidate => candidate.savedAt + 1))
            if (!Number.isSafeInteger(entry.savedAt) || entry.savedAt > 8_640_000_000_000_000 || size(entry) > MAX_RECOVERY_SNAPSHOT_BYTES) invalid()
            const retained = entries.filter(candidate => candidate.id !== entry.id).sort((left, right) => left.savedAt - right.savedAt)
            let total = size(entry) + retained.reduce((bytes, candidate) => bytes + size(candidate), 0)
            while (retained.length + 1 > MAX_RECOVERY_DOCUMENTS || total > MAX_RECOVERY_TOTAL_BYTES) {
              const oldest = retained.shift()
              if (!oldest) invalid()
              records.delete(oldest.id)
              total -= size(oldest)
            }
            records.put(entry)
            result(summary(entry))
          })
        })
      })
    },
    async load(id, validate, expected) {
      identity(id)
      const entry = await transact<StoredEntry>(factory, ['snapshots'], 'readonly', (transaction, result, guard) => {
        const request = transaction.objectStore('snapshots').get(id)
        request.onsuccess = guard(() => {
          if (!request.result) throw new RecoveryError('Local recovery snapshot no longer exists. Refresh the list.')
          const candidate = checkEntry(request.result)
          if (candidate.id !== id) invalid()
          result(candidate)
        })
      })
      if (expected && (entry.hash !== expected.hash || entry.revision !== expected.revision || entry.savedAt !== expected.savedAt)) {
        throw new RecoveryError('Local recovery selection changed. Refresh and choose the snapshot again.')
      }
      if (await digest(entry.document) !== entry.integrity) invalid()
      let verified: AislideDocument
      try { verified = documentCopy(await validate(structuredClone(entry.document))) }
      catch { throw new RecoveryError('Local recovery validation failed. The current document was not replaced.') }
      if (verified.id !== entry.id || verified.revision !== entry.revision || verified.hash !== entry.hash || await digest(verified) !== entry.integrity) {
        throw new RecoveryError('Local recovery validation changed the snapshot. The current document was not replaced.')
      }
      return Object.freeze({ document: verified, filename: entry.filename })
    },
    async remove(id, expected) {
      identity(id)
      return transact<void>(factory, ['snapshots'], 'readwrite', (transaction, _result, guard) => {
        const records = transaction.objectStore('snapshots')
        if (!expected) { records.delete(id); return }
        const request = records.get(id)
        request.onsuccess = guard(() => {
          if (!request.result) throw new RecoveryError('Local recovery snapshot no longer exists. Refresh the list.')
          const entry = checkEntry(request.result)
          if (entry.id !== id || entry.hash !== expected.hash || entry.revision !== expected.revision || entry.savedAt !== expected.savedAt) {
            throw new RecoveryError('Local recovery selection changed. Refresh and choose the snapshot again.')
          }
          records.delete(id)
        })
      })
    },
    async clear() {
      return transact<void>(factory, ['snapshots'], 'readwrite', transaction => { transaction.objectStore('snapshots').clear() })
    },
  }
}

export const recoveryStore = createRecoveryStore()

export type RecoveryScheduler = {
  readonly pending: boolean
  schedule(document: AislideDocument, filename: string): void
  flush(): Promise<void>
  cancel(): void
  dispose(): void
}

export function createRecoveryScheduler({ store = recoveryStore, onError, onSaved }: {
  store?: RecoveryStore
  onError: (error: RecoveryError) => void
  onSaved?: (entry: RecoveryEntry | null) => void
}): RecoveryScheduler {
  let queued: { document: AislideDocument; filename: string } | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  let running: Promise<void> | null = null
  let disposed = false

  function cancel() {
    clearTimeout(timer)
    timer = undefined
    queued = null
  }
  function flush(): Promise<void> {
    clearTimeout(timer)
    timer = undefined
    if (running) return running
    if (!queued || disposed) return Promise.resolve()
    running = (async () => {
      let failure: RecoveryError | undefined
      while (queued && !disposed) {
        const next = queued
        queued = null
        try {
          const entry = await store.save(next.document, next.filename)
          if (!disposed) onSaved?.(entry)
        } catch (reason) {
          failure = recoveryError(reason)
          if (!disposed) onError(failure)
        }
      }
      if (failure) throw failure
    })().finally(() => { running = null })
    return running
  }
  return {
    get pending() { return queued !== null || running !== null },
    schedule(document, filename) {
      if (disposed) throw new RecoveryError('Local recovery scheduler is disposed.')
      try { queued = { document: documentCopy(document), filename: basename(filename) } }
      catch (reason) { cancel(); onError(recoveryError(reason)); return }
      clearTimeout(timer)
      timer = setTimeout(() => { timer = undefined; void flush().catch(() => undefined) }, RECOVERY_DEBOUNCE_MS)
    },
    flush,
    cancel,
    dispose() { disposed = true; cancel() },
  }
}