import { invoke } from '@tauri-apps/api/core'

// Shared in-flight reads prevent navigation from queueing the same SSH commands repeatedly.
const pending = new Map<string, Promise<unknown>>()
const cache = new Map<string, { value: unknown; until: number }>()
let generation = 0
export function resetReadCache() { generation++; pending.clear(); cache.clear() }
export function cachedRead<T>(command: string, args?: Record<string, unknown>, force = false): Promise<T> {
  const key = JSON.stringify([command, args ?? {}])
  const existing = pending.get(key)
  if (existing) return existing as Promise<T>
  const stored = cache.get(key)
  if (!force && stored && stored.until > Date.now()) return Promise.resolve(stored.value as T)
  const version = generation
  const request = invoke<T>(command, args).then(value => {
    if (version === generation) cache.set(key, { value, until: Date.now() + 30_000 })
    return value
  }).finally(() => { if (pending.get(key) === request) pending.delete(key) })
  pending.set(key, request)
  return request
}
