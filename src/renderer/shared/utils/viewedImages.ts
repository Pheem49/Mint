import type { AgentProgress, McpArtifact } from '../types'

/** view_image returns an inline image, optionally followed by post-tool hook notes. */
export function viewedImageDataUri(result: string): string | null {
  const firstLine = result.trim().split('\n', 1)[0].trim()
  return /^data:image\/(?:png|jpeg|gif|webp|svg\+xml);base64,[A-Za-z0-9+/]+={0,2}$/.test(firstLine)
    ? firstLine : null
}

export interface ViewedImageItem {
  id: string
  src?: string
  artifactId?: string
  path: string
  name: string
}

export function viewedImagesFrom(progress: AgentProgress[]): ViewedImageItem[] {
  const images = new Map<string, ViewedImageItem>()
  const artifactsByCallId = new Map<string, McpArtifact>()
  const artifactsById = new Map<string, McpArtifact>()
  const viewImageArtifacts: McpArtifact[] = []

  progress.forEach(event => {
    if (event.type === 'ToolArtifacts') {
      for (const artifact of event.data.artifacts) {
        if (artifact.kind === 'image') {
          artifactsById.set(artifact.id, artifact)
          if (event.data.callId) artifactsByCallId.set(event.data.callId, artifact)
          if (artifact.callId) artifactsByCallId.set(artifact.callId, artifact)
          if (artifact.server === 'view_image') {
            viewImageArtifacts.push(artifact)
          }
        }
      }
    }
  })

  let anonymousCount = 0
  const handledCallIds = new Set<string>()
  const handledArtifactIds = new Set<string>()

  progress.forEach(event => {
    if (event.type !== 'ToolEnd' || event.data.action !== 'view_image'
      || (event.data.status && event.data.status !== 'success')) return

    const callId = event.data.callId
    const path = typeof event.data.input.path === 'string' ? event.data.input.path : ''
    const fallbackName = path.replaceAll('\\', '/').split('/').at(-1) || 'Image'
    const id = callId || `view-image-${anonymousCount++}`

    // 1. Try inline data URI first (legacy or direct)
    const src = viewedImageDataUri(event.data.result)
    if (src) {
      if (callId) handledCallIds.add(callId)
      images.set(id, { id, src, path, name: fallbackName })
      return
    }

    // 2. Check if there is an artifact associated with this callId or mentioned in result
    let artifact = (callId ? artifactsByCallId.get(callId) : undefined)
    if (!artifact) {
      const match = event.data.result.match(/artifactId:\s*([a-f0-9-]+)/i)
      if (match) {
        artifact = artifactsById.get(match[1])
      }
    }

    if (artifact) {
      if (callId) handledCallIds.add(callId)
      handledArtifactIds.add(artifact.id)
      images.set(id, {
        id,
        artifactId: artifact.id,
        path: path || artifact.name,
        name: artifact.name || fallbackName,
      })
    }
  })

  // 3. Fallback for ToolArtifacts tagged as view_image when ToolEnd might be compacted or delayed
  for (const artifact of viewImageArtifacts) {
    if (handledArtifactIds.has(artifact.id) || (artifact.callId && handledCallIds.has(artifact.callId))) {
      continue
    }
    const id = artifact.callId || artifact.id
    images.set(id, {
      id,
      artifactId: artifact.id,
      path: artifact.name,
      name: artifact.name || 'Image',
    })
  }

  return [...images.values()]
}

