import { invoke, isTauri } from '@tauri-apps/api/core'
import type { AislideDocument, PresentationExport, ProjectExport, AssetInput } from './types'

export async function core<T>(request: unknown, { signal }: { signal?: AbortSignal } = {}): Promise<T> {
  if (signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError')
  if (isTauri()) {
    const operationId = crypto.randomUUID()
    const pending = invoke<T>('core_request', { request, operationId })
    let cancellation: Promise<unknown> | undefined
    const cancel = () => { cancellation = invoke('cancel_core_request', { operationId }).catch(() => false) }
    signal?.addEventListener('abort', cancel, { once: true })
    try {
      const result = await pending
      if (signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError')
      return result
    } finally {
      signal?.removeEventListener('abort', cancel)
      await cancellation
    }
  }
  const response = await fetch('/api/core', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
    signal,
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.error ?? 'Core request failed')
  return value as T
}

export async function downloadBytes(bytes: Uint8Array, filename: string, mime: string) {
  if (isTauri()) {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const { writeFile } = await import('@tauri-apps/plugin-fs')
    const path = await save({ defaultPath: filename, filters: [{ name: 'Document', extensions: [filename.split('.').at(-1) ?? 'pptx'] }] })
    if (!path) return false
    await writeFile(path, bytes, { createNew: true })
    return true
  }
  const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: mime }))
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.click()
  setTimeout(() => URL.revokeObjectURL(url), 30_000)
  return true
}

export async function downloadProject(document: AislideDocument, result: ProjectExport) {
  if (isTauri()) {
    const saved = await invoke('save_project', { document, operationId: crypto.randomUUID() })
    return Boolean(saved)
  }
  await downloadBytes(decodeBase64(result.base64), result.filename, 'application/vnd.openxmlformats-officedocument.presentationml.presentation')
  await downloadBytes(new TextEncoder().encode(JSON.stringify(result.checkpoint)), result.checkpoint_filename, 'application/json')
  return true
}

export function decodeBase64(value: string) {
  return Uint8Array.from(atob(value), (character) => character.charCodeAt(0))
}

export async function fileBase64(file: File) {
  if (file.size > 2.8 * 1024 * 1024) throw new Error('This slice accepts PPTX files up to 2.8 MiB')
  const bytes = new Uint8Array(await file.arrayBuffer())
  let text = ''
  for (let index = 0; index < bytes.length; index += 16384) text += String.fromCharCode(...bytes.subarray(index, index + 16384))
  return btoa(text)
}

export async function downloadPresentation(document: AislideDocument, result: PresentationExport) {
  if (isTauri()) return Boolean(await invoke('save_presentation', { document, operationId: crypto.randomUUID(), filename: result.filename }))
  return downloadBytes(decodeBase64(result.base64), result.filename, 'application/vnd.openxmlformats-officedocument.presentationml.presentation')
}

export function svgAsset(svg: string, size = 96, alt = 'Imported SVG icon'): AssetInput {
  if (new TextEncoder().encode(svg).length > 262144) throw new Error('SVG must be at most 256 KiB')
  const bytes = new TextEncoder().encode(svg)
  let encoded = ''
  for (let index = 0; index < bytes.length; index += 16384) encoded += String.fromCharCode(...bytes.subarray(index, index + 16384))
  return { id: `asset-${crypto.randomUUID().slice(0, 8)}`, base64: btoa(encoded), mime_type: 'image/svg+xml', alt, size }
}

export async function fileAssets(files: File[], size = 128): Promise<AssetInput[]> {
  if (!files.length || files.length > 8) throw new Error('Select between 1 and 8 assets')
  return Promise.all(files.map(async (file) => {
    const extension = file.name.split('.').at(-1)?.toLowerCase()
    const mime = file.type || (extension === 'svg' ? 'image/svg+xml' : extension === 'png' ? 'image/png' : ['jpg', 'jpeg'].includes(extension ?? '') ? 'image/jpeg' : '')
    if (!['image/svg+xml', 'image/png', 'image/jpeg'].includes(mime)) throw new Error('Choose SVG, PNG or JPEG assets; export .fig assets from Figma first')
    if (file.size > (mime === 'image/svg+xml' ? 262144 : 1048576)) throw new Error(mime === 'image/svg+xml' ? 'SVG must be at most 256 KiB' : 'Images must be at most 1 MiB')
    return { id: `asset-${crypto.randomUUID().slice(0, 8)}`, base64: await fileBase64(file), mime_type: mime as AssetInput['mime_type'], alt: file.name.slice(0, 500), size }
  }))
}