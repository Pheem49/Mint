/** Capture one frame from an explicitly user-approved screen share. */
async function captureBrowserScreen(): Promise<string> {
  if (!navigator.mediaDevices?.getDisplayMedia) {
    throw new Error('Screen sharing is unavailable in this browser. Use a supported desktop browser or attach an image file.')
  }

  const stream = await navigator.mediaDevices.getDisplayMedia({ video: true, audio: false })
  try {
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
    const video = document.createElement('video')
    video.srcObject = stream
    video.muted = true
    video.playsInline = true
    await new Promise<void>((resolve, reject) => {
      const timeout = window.setTimeout(() => reject(new Error('The screen share did not produce a frame. Please try again.')), 8000)
      video.addEventListener('loadeddata', () => { window.clearTimeout(timeout); resolve() }, { once: true })
      video.addEventListener('error', () => { window.clearTimeout(timeout); reject(new Error('Unable to read the shared screen.')) }, { once: true })
      void video.play().catch((reason) => { window.clearTimeout(timeout); reject(reason) })
    })
    if (!video.videoWidth || !video.videoHeight) throw new Error('The selected screen has no image.')
    const canvas = document.createElement('canvas')
    canvas.width = video.videoWidth
    canvas.height = video.videoHeight
    const context = canvas.getContext('2d')
    if (!context) throw new Error('Unable to prepare the screenshot.')
    context.drawImage(video, 0, 0)
    return canvas.toDataURL('image/png')
  } finally {
    stream.getTracks().forEach((track) => track.stop())
  }
}

async function captureNativeWithoutMint(): Promise<string> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<string>('capture_chat_screen')
}

export async function captureScreenForChat(isDesktopApp: boolean): Promise<string> {
  if (isDesktopApp && navigator.userAgent.toLowerCase().includes('linux')) {
    return captureNativeWithoutMint()
  }
  return captureBrowserScreen()
}
