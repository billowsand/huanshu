import { ref, computed, nextTick } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { readFile } from '@tauri-apps/plugin-fs'
import { open } from '@tauri-apps/plugin-dialog'
import { useProjectsStore } from '../stores/projects'
import type { Project } from '../stores/projects'

export interface MediaItem {
  id: number
  name: string
  ref: string
  url: string
  type: 'image' | 'video'
  caption?: string
  description?: string
}

const MEDIA_EXTS_IMG = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg']
const MEDIA_EXTS_VID = ['mp4', 'webm', 'mov']
const MEDIA_EXTS = [...MEDIA_EXTS_IMG, ...MEDIA_EXTS_VID]

export function useMediaLibrary(options: {
  projectId: () => number | null
  projectName: () => string
  mdContent: () => string
  ensureProjectForMedia: () => Promise<void>
}) {
  const projectsStore = useProjectsStore()

  const mediaItems = ref<MediaItem[]>([])
  const mediaDropOver = ref(false)

  const mediaMap = computed<Record<string, string>>(() =>
    Object.fromEntries(mediaItems.value.map(item => [String(item.id), item.url])),
  )

  function getMimeType(ext: string): string {
    const map: Record<string, string> = {
      png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg',
      gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml',
      mp4: 'video/mp4', webm: 'video/webm', mov: 'video/quicktime',
    }
    return map[ext] || 'application/octet-stream'
  }

  function toMediaRef(id: number): string {
    return `media:${id}`
  }

  async function loadProjectMedia() {
    const pid = options.projectId()
    if (!pid) return
    try {
      const assets = await invoke<any[]>('get_project_media', { projectId: pid })
      mediaItems.value = assets.map((a: any) => ({
        id: a.id,
        name: a.original_name,
        ref: toMediaRef(a.id),
        url: convertFileSrc(a.url),
        type: a.media_type,
        caption: a.caption,
        description: a.description,
      }))
    } catch (e) {
      console.error('Failed to load project media:', e)
    }
  }

  async function openMediaFiles() {
    try {
      const result = await open({
        multiple: true,
        filters: [{ name: '图片 / 视频', extensions: MEDIA_EXTS }],
      })
      if (!result) return
      const paths = Array.isArray(result) ? result : [result]
      for (const p of paths) await addMediaFromPath(p)
    } catch (e) { console.error(e) }
  }

  async function addMediaFromPath(path: string) {
    await options.ensureProjectForMedia()
    const pid = options.projectId()
    if (!pid) return
    const name = path.split(/[/\\]/).pop() ?? path
    const ext = name.split('.').pop()?.toLowerCase() ?? ''
    const mediaType = MEDIA_EXTS_VID.includes(ext) ? 'video' : 'image'

    try {
      const bytes = await readFile(path)
      const asset = await invoke<any>('import_media_asset', {
        req: {
          project_id: pid,
          slide_index: null,
          filename: name,
          mime_type: getMimeType(ext),
          data: Array.from(bytes),
          media_type: mediaType,
        },
      })

      mediaItems.value.push({
        id: asset.id,
        name: asset.original_name,
        ref: toMediaRef(asset.id),
        url: convertFileSrc(asset.url),
        type: asset.media_type,
        caption: asset.caption,
        description: asset.description,
      })
    } catch (e) {
      console.error('Failed to import media:', e)
    }
  }

  async function addMediaFromFile(file: File) {
    await options.ensureProjectForMedia()
    const pid = options.projectId()
    if (!pid) return
    const ext = file.name.split('.').pop()?.toLowerCase() ?? ''
    const mediaType = MEDIA_EXTS_VID.includes(ext) ? 'video' : 'image'

    try {
      const buffer = await file.arrayBuffer()
      const bytes = Array.from(new Uint8Array(buffer))

      const asset = await invoke<any>('import_media_asset', {
        req: {
          project_id: pid,
          slide_index: null,
          filename: file.name,
          mime_type: getMimeType(ext),
          data: bytes,
          media_type: mediaType,
        },
      })

      mediaItems.value.push({
        id: asset.id,
        name: asset.original_name,
        ref: toMediaRef(asset.id),
        url: convertFileSrc(asset.url),
        type: asset.media_type,
        caption: asset.caption,
        description: asset.description,
      })
    } catch (e) {
      console.error('Failed to import media:', e)
    }
  }

  async function removeMedia(id: number) {
    const idx = mediaItems.value.findIndex(m => m.id === id)
    if (idx === -1) return
    try {
      await invoke('delete_media_asset', { id })
    } catch (e) {
      console.error('Failed to delete media:', e)
    }
    mediaItems.value.splice(idx, 1)
  }

  function onMediaDragOver(e: DragEvent) { e.preventDefault(); mediaDropOver.value = true }
  function onMediaDragLeave() { mediaDropOver.value = false }
  function onMediaDrop(e: DragEvent) {
    e.preventDefault()
    mediaDropOver.value = false
    for (const file of Array.from(e.dataTransfer?.files ?? [])) {
      const ext = file.name.split('.').pop()?.toLowerCase() ?? ''
      if (MEDIA_EXTS.includes(ext)) addMediaFromFile(file)
    }
  }

  function insertMediaUrl(ref: string, jsonTextareaRef: HTMLTextAreaElement | null, onEditorJsonInput: () => void) {
    const el = jsonTextareaRef
    if (!el) { navigator.clipboard.writeText(`"${ref}"`).catch(() => {}); return }
    const s = el.selectionStart
    const en = el.selectionEnd
    const quoted = `"${ref}"`
    nextTick(() => {
      el.selectionStart = el.selectionEnd = s + quoted.length
      el.focus()
      onEditorJsonInput()
    })
  }

  return {
    mediaItems,
    mediaDropOver,
    mediaMap,
    MEDIA_EXTS_IMG,
    MEDIA_EXTS_VID,
    MEDIA_EXTS,
    loadProjectMedia,
    openMediaFiles,
    addMediaFromPath,
    addMediaFromFile,
    removeMedia,
    onMediaDragOver,
    onMediaDragLeave,
    onMediaDrop,
    insertMediaUrl,
  }
}
