import { invoke, isTauri } from '@tauri-apps/api/core'

// Share results and in-flight requests between thumbnails, preview and playback.
const cache = new Map<string, Promise<string[]>>()
const CACHE_LIMIT = 256

export function getTitleTokens(text: string): Promise<string[]> {
  if (!text || !isTauri()) return Promise.resolve([text])
  const existing = cache.get(text)
  if (existing) return existing

  const request = invoke<string[]>('segment_title', { text }).catch((error) => {
    cache.delete(text)
    throw error
  })
  if (cache.size >= CACHE_LIMIT) cache.delete(cache.keys().next().value!)
  cache.set(text, request)
  return request
}
