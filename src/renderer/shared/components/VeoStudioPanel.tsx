import React, { useState, useRef, useCallback, useEffect } from 'react'
import { getActiveModel, setActiveModel, subscribeModelChange } from '../utils/modelManager'
import {
  generateVideo,
  getVideoGenProviders,
  convertFileSrc,
  getProfileValue,
  setProfileValue,
  isTauriRuntime,
  type VideoGenRequest,
  type VideoGenResponse,
  type VideoGenProviders,
  type VideoGenEntry,
} from '@/tauri'

type AspectRatio = '16:9' | '9:16' | '1:1'
type Duration = 5 | 8

interface VeoStudioPanelProps {
  view: string
  onSendToChat?: (prompt: string) => void
  onToggleMobileSidebar?: () => void
}

import { VIDEO_ASPECT_RATIOS, VIDEO_STYLE_PRESETS } from '../constants/studio'

const ASPECT_OPTIONS = VIDEO_ASPECT_RATIOS

const DURATION_OPTIONS: { value: Duration; label: string }[] = [
  { value: 5, label: '5s' },
  { value: 8, label: '8s' },
]

const STYLE_SUGGESTIONS = VIDEO_STYLE_PRESETS

import { VEO_STUDIO_MODELS } from '../constants/models'
const PROVIDER_MODELS = VEO_STUDIO_MODELS

const PROVIDER_LABELS: Record<string, string> = {
  veo: 'Google Veo (Gemini Videos)',
}

function providerLabel(key: string) {
  return PROVIDER_LABELS[key] ?? key
}

function defaultModelForProvider(provider: string): string {
  const models = PROVIDER_MODELS[provider]
  return models?.[0]?.value ?? ''
}

function formatTimecode(seconds: number): string {
  if (isNaN(seconds) || seconds < 0) return '00:00:00'
  const hrs = Math.floor(seconds / 3600)
  const mins = Math.floor((seconds % 3600) / 60)
  const secs = Math.floor(seconds % 60)
  const ms = Math.floor((seconds % 1) * 10)
  if (hrs > 0) {
    return `${hrs.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}.${ms}`
  }
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}.${ms}`
}

function VideoSkeletonCard() {
  return (
    <div className="veo-studio-skeleton">
      <div className="veo-studio-skeleton-shine" />
      <div className="veo-studio-skeleton-icon" aria-hidden="true">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
          <polygon points="23 7 16 12 23 17 23 7" />
          <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
        </svg>
      </div>
    </div>
  )
}

export default function VeoStudioPanel({ view, onSendToChat, onToggleMobileSidebar }: VeoStudioPanelProps) {
  const [prompt, setPrompt] = useState('')
  const [negativePrompt, setNegativePrompt] = useState('')
  const [showNegative, setShowNegative] = useState(false)
  const [aspectRatio, setAspectRatio] = useState<AspectRatio>('16:9')
  const [duration, setDuration] = useState<Duration>(5)
  const [generating, setGenerating] = useState(false)
  const [error, setError] = useState('')
  const [result, setResult] = useState<VideoGenResponse | null>(null)
  const [promptHistory, setPromptHistory] = useState<string[]>([])
  const [historyVideos, setHistoryVideos] = useState<VideoGenEntry[]>([])

  const [providers, setProviders] = useState<VideoGenProviders>({ active: 'veo', available: ['veo'] })
  const [selectedProvider, setSelectedProvider] = useState('veo')
  const [selectedModel, setSelectedModel] = useState(defaultModelForProvider('veo'))

  const promptRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    let cancelled = false
    getVideoGenProviders().then(async (data) => {
      if (cancelled) return
      setProviders(data)
      setSelectedProvider(data.active)
      const activeModel = await getActiveModel('veoModel')
      if (activeModel && !cancelled) {
        setSelectedModel(activeModel)
      } else if (!cancelled) {
        setSelectedModel(defaultModelForProvider(data.active))
      }
    }).catch(() => { /* keep defaults */ })

    // Subscribe to real-time model changes from Settings or other components
    const unsubscribe = subscribeModelChange((detail) => {
      if (detail.provider === 'veoModel' || detail.category === 'video') {
        setSelectedModel(detail.model)
      }
    })

    return () => {
      cancelled = true
      unsubscribe()
    }
  }, [])

  const handleProviderChange = (provider: string) => {
    setSelectedProvider(provider)
    const def = defaultModelForProvider(provider)
    setSelectedModel(def)
    setActiveModel('veoModel', def, 'video')
  }

  const handleModelChange = (model: string) => {
    setSelectedModel(model)
    setActiveModel('veoModel', model, 'video')
  }

  const handleGenerate = useCallback(async () => {
    const trimmed = prompt.trim()
    if (!trimmed || generating) return

    setGenerating(true)
    setError('')
    setResult(null)

    const request: VideoGenRequest = {
      prompt: trimmed,
      negativePrompt: negativePrompt.trim() || undefined,
      aspectRatio,
      duration,
      model: selectedModel || undefined,
      provider: selectedProvider,
    }

    try {
      const response = await generateVideo(request)
      setResult(response)
      setPromptHistory((prev) =>
        [trimmed, ...prev.filter((p) => p !== trimmed)].slice(0, 8),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setGenerating(false)
    }
  }, [prompt, negativePrompt, aspectRatio, duration, generating, selectedProvider, selectedModel])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      handleGenerate()
    }
  }

  const downloadVideo = (entry: VideoGenEntry, index: number) => {
    const a = document.createElement('a')
    a.href = entry.url || entry.path
    a.download = `mint-veo-${index + 1}.mp4`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
  }

  const modelOptions = PROVIDER_MODELS[selectedProvider] ?? []

  // ── Edit tab state (CapCut NLE Editor) ──────────────────────────────────
  const [activeTab, setActiveTab] = useState<'generate' | 'edit'>('generate')
  const [editMode, setEditMode] = useState<'ai' | 'manual'>('ai')
  const [aiInstruction, setAiInstruction] = useState('')
  const [editInputFile, setEditInputFile] = useState('')
  const [editOutputFile, setEditOutputFile] = useState('')
  const [editVideoInfo, setEditVideoInfo] = useState<any>(null)
  const [editOperation, setEditOperation] = useState<string>('trim')
  const [editParams, setEditParams] = useState<Record<string, any>>({
    trimStart: 0, trimEnd: 30,
    resizeWidth: 1920, resizeHeight: 1080,
    resolution: '1920x1080',
    thresholdDb: -30, minSilenceSecs: 0.5,
    transcribeLang: 'auto', subPreset: 'tiktok', targetLanguage: 'th',
    maxShortsClips: 3, shortsDuration: 60,
  })
  const [editRunning, setEditRunning] = useState(false)
  const [editError, setEditError] = useState('')
  const [editLog, setEditLog] = useState<string[]>([])
  const [editResult, setEditResult] = useState<any>(null)
  const [videoPreviewUrl, setVideoPreviewUrl] = useState<string>('')
  const [outputVideoUrl, setOutputVideoUrl] = useState<string>('')
  const [activePreview, setActivePreview] = useState<'input' | 'output'>('input')

  // Interactive Playhead & Canvas State
  const videoRef = useRef<HTMLVideoElement>(null)
  const timelineTracksRef = useRef<HTMLDivElement>(null)
  const [isPlaying, setIsPlaying] = useState(false)
  const [currentTime, setCurrentTime] = useState(0)
  const [videoDuration, setVideoDuration] = useState(30)
  const [splitMarkers, setSplitMarkers] = useState<number[]>([])
  const [timelineZoom, setTimelineZoom] = useState<number>(30)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const addLog = (msg: string) =>
    setEditLog((prev) => [...prev, `${new Date().toLocaleTimeString()} — ${msg}`])

  const handleBrowseFile = async () => {
    if (isTauriRuntime()) {
      try {
        // Native Tauri file dialog gives us the full absolute system path.
        const { open } = await import('@tauri-apps/plugin-dialog')
        const selected = await open({
          multiple: false,
          filters: [{ name: 'Video Files', extensions: ['mp4', 'mov', 'avi', 'mkv', 'webm', 'flv'] }]
        })
        if (selected && typeof selected === 'string') {
          setEditInputFile(selected)
          const ext = selected.split('.').pop() || 'mp4'
          setEditOutputFile(selected.replace(`.${ext}`, `_edited.${ext}`))
          addLog(`✓ Selected local video file: ${selected}`)
          return
        }
      } catch {
        // Fall through to the browser file picker below.
      }
    }
    fileInputRef.current?.click()
  }

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    const filePath = (file as any).path || file.name
    setEditInputFile(filePath)
    setVideoPreviewUrl(URL.createObjectURL(file))
    if (!editOutputFile.trim()) {
      const ext = filePath.split('.').pop() || 'mp4'
      setEditOutputFile(filePath.replace(`.${ext}`, `_edited.${ext}`))
    }
    if (!(file as any).path) {
      addLog(`ℹ️ Browser Sandbox: Selected file "${file.name}". If rpc needs backend file, type full path.`)
    }
  }

  const handleLoadVideo = useCallback(async () => {
    if (!editInputFile.trim()) return
    setEditVideoInfo(null); setEditError('')
    try {
      const { videoLoad } = await import('@/tauri')
      const info = await videoLoad(editInputFile.trim())
      setEditVideoInfo(info)
      if (info.duration && info.duration > 0) {
        setVideoDuration(info.duration)
        setEditParams(p => ({ ...p, trimEnd: Math.round(info.duration) }))
      }
      if (!editOutputFile.trim()) {
        const ext = editInputFile.trim().split('.').pop() || 'mp4'
        setEditOutputFile(editInputFile.trim().replace(`.${ext}`, `_edited.${ext}`))
      }
      addLog(`✓ Loaded Video Metadata: ${info.width}x${info.height}, ${info.duration.toFixed(2)}s @ ${info.fps.toFixed(2)} fps`)
    } catch (e) {
      setEditError(e instanceof Error ? e.message : String(e))
    }
  }, [editInputFile, editOutputFile])

  // Auto load metadata when input file path changes
  useEffect(() => {
    if (editInputFile.trim() && !editInputFile.startsWith('sample_demo')) {
      handleLoadVideo()
    }
  }, [editInputFile, handleLoadVideo])

  const handleLoadDemoVideo = () => {
    const demoPath = 'sample_video_demo.mp4'
    setEditInputFile(demoPath)
    setVideoPreviewUrl('')
    setEditVideoInfo({
      width: 1920,
      height: 1080,
      duration: 30.0,
      fps: 30.0,
      format: 'h264 / mp4',
    })
    setVideoDuration(30.0)
    setEditParams(p => ({ ...p, trimStart: 0, trimEnd: 30 }))
    addLog('✓ Demo Sample Video loaded into workspace (30.0s, 1080p @ 30 FPS)')
  }

  // Player controls
  const handleTogglePlay = () => {
    if (!videoRef.current) return
    if (isPlaying) {
      videoRef.current.pause()
    } else {
      videoRef.current.play().catch(() => {})
    }
  }

  const handleStopVideo = () => {
    if (!videoRef.current) return
    videoRef.current.pause()
    videoRef.current.currentTime = 0
    setCurrentTime(0)
    setIsPlaying(false)
  }

  const handleSeekDelta = (delta: number) => {
    if (!videoRef.current) return
    const target = Math.max(0, Math.min(videoDuration, videoRef.current.currentTime + delta))
    videoRef.current.currentTime = target
    setCurrentTime(target)
  }

  // Timeline Click / Scrub Seek
  const handleTimelineSeek = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!timelineTracksRef.current || videoDuration <= 0) return
    const rect = timelineTracksRef.current.getBoundingClientRect()
    const clickX = e.clientX - rect.left - 80 // subtract header column width (80px)
    const availableWidth = rect.width - 80
    if (availableWidth <= 0) return
    const ratio = Math.max(0, Math.min(1, clickX / availableWidth))
    const targetTime = ratio * videoDuration
    setCurrentTime(targetTime)
    if (videoRef.current) {
      videoRef.current.currentTime = targetTime
    }
  }

  const handleSplitAtPlayhead = () => {
    const mark = Number(currentTime.toFixed(1))
    if (mark > 0 && mark < videoDuration && !splitMarkers.includes(mark)) {
      const updated = [...splitMarkers, mark].sort((a, b) => a - b)
      setSplitMarkers(updated)
      setEditParams(p => ({ ...p, trimStart: 0, trimEnd: mark }))
      addLog(`✂️ Split clip at ${mark.toFixed(1)}s`)
    }
  }

  const handleDeleteSplitMarkers = () => {
    setSplitMarkers([])
    setEditParams(p => ({ ...p, trimStart: 0, trimEnd: Math.round(videoDuration) }))
    addLog('🗑️ Reset clip markers and timeline splits')
  }

  const handleCycleTimelineZoom = () => {
    const zooms = [15, 30, 60]
    const nextIdx = (zooms.indexOf(timelineZoom) + 1) % zooms.length
    setTimelineZoom(zooms[nextIdx])
  }

  const handleRunEdit = useCallback(async () => {
    if (!editInputFile.trim()) return
    setEditRunning(true); setEditError(''); setEditResult(null)
    const tauri = await import('@/tauri')

    if (editMode === 'ai') {
      if (!aiInstruction.trim()) {
        setEditError('Please type AI editing instructions or select a prompt chip.')
        setEditRunning(false)
        return
      }
      addLog(`AI Agent executing: "${aiInstruction.trim()}"...`)
      try {
        const aiRes = await tauri.videoAiEdit({
          input: editInputFile.trim(),
          output: editOutputFile.trim() || undefined,
          instruction: aiInstruction.trim(),
        })
        for (const step of aiRes.stepsPerformed) {
          addLog(`✓ Step ${step.step} [${step.operation}]: ${step.description}`)
        }
        setEditResult({ outputPath: aiRes.outputPath, summary: aiRes.summary, steps: aiRes.stepsPerformed })
        setOutputVideoUrl(aiRes.outputPath)
        setActivePreview('output')
        addLog(`✓ AI Video Edit finished! Output saved → ${aiRes.outputPath}`)
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setEditError(msg); addLog(`✗ AI Edit Error: ${msg}`)
      } finally {
        setEditRunning(false)
      }
      return
    }

    if (!editOutputFile.trim()) return
    addLog(`Running: ${editOperation}...`)
    try {
      let result: any
      switch (editOperation) {
        case 'trim':
          result = await tauri.videoTrim({ input: editInputFile.trim(), output: editOutputFile.trim(), start: Number(editParams.trimStart), end: Number(editParams.trimEnd) })
          break
        case 'resize':
          result = await tauri.videoResize({ input: editInputFile.trim(), output: editOutputFile.trim(), width: Number(editParams.resizeWidth), height: Number(editParams.resizeHeight) })
          break
        case 'remove-silence':
          result = await tauri.videoRemoveSilence({ input: editInputFile.trim(), output: editOutputFile.trim(), thresholdDb: Number(editParams.thresholdDb), minSilenceSecs: Number(editParams.minSilenceSecs) })
          break
        case 'extract-audio':
          result = await tauri.videoExtractAudio({ input: editInputFile.trim(), output: editOutputFile.trim().replace(/\.(mp4|mov|avi|mkv)$/i, '.wav') })
          break
        case 'transcribe': {
          const transRes = await tauri.speechTranscribe({ input: editInputFile.trim(), language: editParams.transcribeLang || undefined })
          const { srt } = await tauri.subtitleGenerate(transRes.segments)
          const srtPath = editOutputFile.trim().endsWith('.srt') ? editOutputFile.trim() : `${editOutputFile.trim()}.srt`
          setEditParams(p => ({ ...p, generatedSrt: srt, segments: transRes.segments }))
          result = { outputPath: srtPath, segmentsCount: transRes.segments.length }
          addLog(`✓ Transcribed ${transRes.segments.length} segments (${transRes.language})`)
          break
        }
        case 'translate-subtitles': {
          const srtInput = editParams.generatedSrt || editParams.srtContent || ''
          const { srt } = await tauri.subtitleTranslate({ srtContent: srtInput, targetLanguage: editParams.targetLanguage || 'th' })
          const outSrtPath = editOutputFile.trim().endsWith('.srt') ? editOutputFile.trim() : `${editOutputFile.trim()}_translated.srt`
          setEditParams(p => ({ ...p, translatedSrt: srt }))
          result = { outputPath: outSrtPath }
          addLog(`✓ Subtitles translated to ${editParams.targetLanguage || 'th'}`)
          break
        }
        case 'burn-subtitles': {
          const srtInput = editParams.translatedSrt || editParams.generatedSrt || editParams.srtContent || ''
          result = await tauri.subtitleBurn({
            inputVideo: editInputFile.trim(),
            srtInput: srtInput,
            outputVideo: editOutputFile.trim(),
            preset: editParams.subPreset || 'tiktok',
          })
          break
        }
        case 'make-shorts': {
          const shortsRes = await tauri.videoMakeShorts({
            input: editInputFile.trim(),
            outputDir: editOutputFile.trim() || undefined,
            maxClips: Number(editParams.maxShortsClips || 3),
            targetDuration: Number(editParams.shortsDuration || 60),
            burnSubtitles: editParams.shortsBurnSubtitles !== false,
          })
          result = { outputPath: `${shortsRes.clips.length} shorts clip(s) created`, clips: shortsRes.clips }
          addLog(`✓ Auto Shorts generated: ${shortsRes.clips.length} clip(s)`)
          break
        }
        case 'export':
          result = await tauri.videoExport({ input: editInputFile.trim(), output: editOutputFile.trim(), resolution: editParams.resolution || undefined })
          break
        default:
          throw new Error(`Unknown operation: ${editOperation}`)
      }
      setEditResult(result)
      if (result?.outputPath) {
        setOutputVideoUrl(result.outputPath)
        setActivePreview('output')
      }
      addLog(`✓ Done → ${result.outputPath}${result.duration ? ` (${result.duration.toFixed(2)}s)` : ''}`)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      setEditError(msg); addLog(`✗ Error: ${msg}`)
    } finally {
      setEditRunning(false)
    }
  }, [editInputFile, editOutputFile, editOperation, editParams, editMode, aiInstruction])

  // Get active video src for canvas player
  const getActiveVideoSrc = () => {
    if (activePreview === 'output' && outputVideoUrl) {
      return convertFileSrc(outputVideoUrl)
    }
    if (videoPreviewUrl) return videoPreviewUrl
    if (!editInputFile) return ''
    if (editInputFile.startsWith('http://') || editInputFile.startsWith('https://') || editInputFile.startsWith('blob:')) {
      return editInputFile
    }
    return convertFileSrc(editInputFile)
  }

  const activeVideoSrc = getActiveVideoSrc()
  const playheadPercent = videoDuration > 0 ? (currentTime / videoDuration) * 100 : 0

  return (
    <div
      className={`veo-studio ${view === 'veo' ? 'is-visible' : ''}`}
      id="veo-studio-panel"
      role="main"
      aria-label="AI Video Editor"
      aria-hidden={view !== 'veo'}
    >
      {/* Header */}
      <header className="veo-studio-header">
        <div className="veo-studio-header-title">
          <button
            className="mobile-menu-btn"
            type="button"
            onClick={onToggleMobileSidebar}
            aria-label="Toggle menu"
            style={{ marginRight: '8px' }}
          >
            ☰
          </button>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polygon points="23 7 16 12 23 17 23 7" />
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
          </svg>
          <h1>Veo Studio</h1>
          <span className="veo-studio-badge">
            {activeTab === 'generate' ? providerLabel(selectedProvider) : 'AI Editor'}
          </span>
        </div>

        {/* Tab switcher */}
        <div className="veo-studio-tabs" role="tablist" aria-label="Studio mode">
          <button
            id="veo-tab-generate"
            role="tab"
            type="button"
            aria-selected={activeTab === 'generate'}
            className={`veo-studio-tab ${activeTab === 'generate' ? 'is-active' : ''}`}
            onClick={() => setActiveTab('generate')}
          >
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <polygon points="23 7 16 12 23 17 23 7" /><rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
            </svg>
            Generate
          </button>
          <button
            id="veo-tab-edit"
            role="tab"
            type="button"
            aria-selected={activeTab === 'edit'}
            className={`veo-studio-tab ${activeTab === 'edit' ? 'is-active' : ''}`}
            onClick={() => setActiveTab('edit')}
          >
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
            </svg>
            Edit
          </button>
        </div>
      </header>

      {/* ── GENERATE TAB ─────────────────────────────────────────────── */}
      {activeTab === 'generate' && (
        <div className="veo-studio-content" role="tabpanel" aria-labelledby="veo-tab-generate">
          {/* Left: controls */}
          <section className="veo-studio-controls" aria-label="Video generation settings">

            {/* Provider & Model side-by-side */}
            <div className="veo-studio-field" style={{ display: 'flex', gap: '10px', width: '100%' }}>
              <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <label className="veo-studio-label" htmlFor="veo-studio-provider" style={{ display: 'flex', alignItems: 'center', width: '100%', height: '18px' }}>
                  <span>Provider</span>
                </label>
                <select
                  id="veo-studio-provider"
                  className="veo-studio-textarea"
                  style={{ padding: '8px 10px', height: '38px', cursor: 'pointer' }}
                  value={selectedProvider}
                  onChange={(e) => handleProviderChange(e.target.value)}
                  disabled={generating}
                >
                  {providers.available.map((prov) => (
                    <option key={prov} value={prov}>
                      {providerLabel(prov).replace(/^[^a-zA-Z0-9]+/, '')}
                    </option>
                  ))}
                </select>
              </div>

              <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <label className="veo-studio-label" htmlFor="veo-studio-model" style={{ display: 'flex', alignItems: 'center', height: '18px' }}>
                  Model
                </label>
                <select
                  id="veo-studio-model"
                  className="veo-studio-textarea"
                  style={{ padding: '8px 10px', height: '38px', cursor: 'pointer' }}
                  value={selectedModel}
                  onChange={(e) => handleModelChange(e.target.value)}
                  disabled={generating}
                >
                  {modelOptions.map(({ value, label }) => (
                    <option key={value} value={value}>{label}</option>
                  ))}
                </select>
              </div>
            </div>

            {/* Prompt */}
            <div className="veo-studio-field">
              <label className="veo-studio-label" htmlFor="veo-studio-prompt">
                Prompt
                <span className="veo-studio-label-hint">Describe the video you want</span>
              </label>
              <textarea
                id="veo-studio-prompt"
                ref={promptRef}
                className="veo-studio-textarea"
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="a golden eagle soaring over mountain peaks at sunset, cinematic 4K, slow motion..."
                rows={4}
                disabled={generating}
                aria-required="true"
              />
              <div className="veo-studio-prompt-hint">⌘+Enter to generate</div>
            </div>

            {/* Style suggestion chips */}
            <div className="veo-studio-field">
              <span className="veo-studio-label">Style suggestions</span>
              <div className="veo-studio-chips" role="group" aria-label="Style suggestions">
                {STYLE_SUGGESTIONS.map((style) => (
                  <button
                    key={style}
                    type="button"
                    className="veo-studio-chip"
                    onClick={() => setPrompt((p) => p.trim() ? `${p.trim()}, ${style}` : style)}
                    disabled={generating}
                    title={`Add "${style}" to prompt`}
                  >
                    {style}
                  </button>
                ))}
              </div>
            </div>

            {/* Negative prompt (collapsible) */}
            <div className="veo-studio-field">
              <button
                type="button"
                className="veo-studio-toggle"
                onClick={() => setShowNegative((v) => !v)}
                aria-expanded={showNegative}
                aria-controls="veo-studio-negative-wrap"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"
                  style={{ transform: showNegative ? 'rotate(90deg)' : 'none', transition: 'transform 0.2s' }}>
                  <polyline points="9 18 15 12 9 6" />
                </svg>
                Negative prompt
                {negativePrompt.trim() && <span className="veo-studio-badge-dot" aria-label="has content" />}
              </button>
              {showNegative && (
                <div id="veo-studio-negative-wrap">
                  <textarea
                    id="veo-studio-negative"
                    className="veo-studio-textarea veo-studio-textarea--sm"
                    value={negativePrompt}
                    onChange={(e) => setNegativePrompt(e.target.value)}
                    placeholder="blurry, shaky camera, low quality, watermark..."
                    rows={2}
                    disabled={generating}
                  />
                </div>
              )}
            </div>

            {/* Aspect ratio */}
            <div className="veo-studio-field">
              <span className="veo-studio-label">Aspect ratio</span>
              <div className="veo-studio-aspect-group" role="radiogroup" aria-label="Aspect ratio">
                {ASPECT_OPTIONS.map(({ value, label, icon }) => (
                  <button
                    key={value}
                    type="button"
                    role="radio"
                    aria-checked={aspectRatio === value}
                    className={`veo-studio-aspect-btn ${aspectRatio === value ? 'is-active' : ''}`}
                    onClick={() => setAspectRatio(value)}
                    disabled={generating}
                    id={`veo-studio-aspect-${value.replace(':', 'x')}`}
                  >
                    <span className="veo-studio-aspect-icon" aria-hidden="true">{icon}</span>
                    <span>{label}</span>
                  </button>
                ))}
              </div>
            </div>

            {/* Duration */}
            <div className="veo-studio-field">
              <span className="veo-studio-label">Duration</span>
              <div className="veo-studio-duration-group" role="radiogroup" aria-label="Video duration">
                {DURATION_OPTIONS.map(({ value, label }) => (
                  <button
                    key={value}
                    type="button"
                    role="radio"
                    aria-checked={duration === value}
                    className={`veo-studio-duration-btn ${duration === value ? 'is-active' : ''}`}
                    onClick={() => setDuration(value)}
                    disabled={generating}
                    id={`veo-studio-duration-${value}`}
                  >
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" />
                    </svg>
                    {label}
                  </button>
                ))}
              </div>
            </div>

            {/* Generate button */}
            <button
              id="veo-studio-generate-btn"
              type="button"
              className={`veo-studio-generate-btn ${generating ? 'is-loading' : ''}`}
              onClick={handleGenerate}
              disabled={!prompt.trim() || generating}
              aria-busy={generating}
            >
              {generating ? (
                <>
                  <span className="veo-studio-spinner" aria-hidden="true" />
                  Generating video...
                </>
              ) : (
                <>
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <polygon points="23 7 16 12 23 17 23 7" />
                    <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                  </svg>
                  Generate Video
                </>
              )}
            </button>

            {/* Error */}
            {error && (
              <div className="veo-studio-error" role="alert">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
                </svg>
                <span>{error}</span>
                <button type="button" onClick={() => setError('')} aria-label="Dismiss error">×</button>
              </div>
            )}

            {/* Prompt history */}
            {promptHistory.length > 0 && (
              <div className="veo-studio-field">
                <span className="veo-studio-label">Recent prompts</span>
                <div className="veo-studio-history">
                  {promptHistory.map((p) => (
                    <button
                      key={p}
                      type="button"
                      className="veo-studio-history-item"
                      onClick={() => { setPrompt(p); promptRef.current?.focus() }}
                      disabled={generating}
                      title={p}
                    >
                      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                        <circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" />
                      </svg>
                      <span>{p}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </section>

          {/* Right: results */}
          <section className="veo-studio-results" aria-label="Generated videos" aria-live="polite">
            <div className="veo-studio-active-workspace">
              {generating && (
                <div className="veo-studio-skeleton-wrap">
                  <VideoSkeletonCard />
                  <p className="veo-studio-generating-label" aria-live="assertive">
                    Generating {duration}s video with {providerLabel(selectedProvider)} — this may take a moment…
                  </p>
                </div>
              )}

              {!generating && result && (
                <>
                  {result.description && (
                    <p className="veo-studio-description">{result.description}</p>
                  )}
                  <div className="veo-studio-video-grid">
                    {result.videos.map((entry: any, idx: number) => (
                      <article key={entry.id} className="veo-studio-card" aria-label={`Generated video ${idx + 1}`}>
                        <div className="veo-studio-card-video-wrap">
                          <video
                            src={convertFileSrc(entry.path || entry.url)}
                            className="veo-studio-card-video"
                            controls
                            preload="metadata"
                            aria-label={entry.message || prompt}
                          />
                        </div>
                        <div className="veo-studio-card-actions">
                          <button
                            type="button"
                            className="veo-studio-action-btn"
                            onClick={() => downloadVideo(entry, idx)}
                            title="Download video"
                            id={`veo-studio-download-${idx}`}
                          >
                            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" />
                            </svg>
                            Download
                          </button>
                          {onSendToChat && (
                            <button
                              type="button"
                              className="veo-studio-action-btn veo-studio-action-btn--primary"
                              onClick={() => onSendToChat(prompt)}
                              title="Send prompt to Chat"
                              id={`veo-studio-send-${idx}`}
                            >
                              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                                <line x1="22" y1="2" x2="11" y2="13" /><polygon points="22 2 15 22 11 13 2 9 22 2" />
                              </svg>
                              Send to Chat
                            </button>
                          )}
                        </div>
                      </article>
                    ))}
                  </div>
                  <p className="veo-studio-meta">
                    Provider: <strong>{providerLabel(result.provider)}</strong> · Model: <strong>{result.model}</strong> · {result.videos.length} video{result.videos.length > 1 ? 's' : ''}
                  </p>
                </>
              )}

              {!generating && !result && (
                <div className="veo-studio-active-empty">
                  <div className="veo-studio-empty-icon" aria-hidden="true">
                    <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
                      <polygon points="23 7 16 12 23 17 23 7" />
                      <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                    </svg>
                  </div>
                  <p>No video generated yet</p>
                  <span>Enter a prompt on the left and click Generate Video to begin.</span>
                </div>
              )}
            </div>

            {/* History gallery */}
            <div className="veo-studio-history-header">
              <div className="veo-studio-history-title">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <polygon points="23 7 16 12 23 17 23 7" />
                  <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                </svg>
                <span>Video History</span>
                <span className="veo-studio-history-badge">{historyVideos.length}</span>
              </div>
            </div>
            <div className="veo-studio-history-gallery">
              {historyVideos.length === 0 ? (
                <div className="veo-studio-gallery-empty">
                  <p>Your video gallery is empty</p>
                  <span>Generated videos will appear here once the backend is connected.</span>
                </div>
              ) : (
                <div className="veo-studio-video-grid">
                  {historyVideos.map((video, idx) => (
                    <article key={video.id} className="veo-studio-card" aria-label={`Saved video ${idx + 1}`}>
                      <div className="veo-studio-card-video-wrap">
                        <video
                          src={convertFileSrc(video.path || video.url)}
                          className="veo-studio-card-video"
                          controls
                          preload="metadata"
                        />
                      </div>
                      <div className="veo-studio-card-actions">
                        <button
                          type="button"
                          className="veo-studio-action-btn"
                          onClick={() => downloadVideo(video, idx)}
                          title="Download video"
                          id={`veo-studio-gallery-download-${idx}`}
                        >
                          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" />
                          </svg>
                          Download
                        </button>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </div>
          </section>
        </div>
      )}

      {/* ── EDIT TAB (CapCut-Style Interactive NLE Editor) ──────────────── */}
      {activeTab === 'edit' && (
        <div className="capcut-nle-container" role="tabpanel" aria-labelledby="veo-tab-edit">
          {/* CapCut Header Bar */}
          <div className="capcut-header-bar">
            <div className="capcut-header-left">
              <div className="capcut-logo-badge">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/></svg>
                AI Video Editor
              </div>
              <div className="capcut-nav-tabs">
                <button type="button" className={`capcut-nav-tab ${editMode === 'ai' ? 'is-active' : ''}`} onClick={() => setEditMode('ai')}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect width="18" height="12" x="3" y="8" rx="2"/><path d="M12 2v6"/><path d="M8 5h8"/><path d="M15 13v2"/><path d="M9 13v2"/></svg>
                  AI Prompt
                </button>
                <button type="button" className={`capcut-nav-tab ${editMode === 'manual' ? 'is-active' : ''}`} onClick={() => setEditMode('manual')}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
                  Manual Tools
                </button>
                <button type="button" className="capcut-nav-tab" onClick={() => { setAiInstruction('Make Shorts 9:16'); setEditMode('ai'); }}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                  Auto Shorts
                </button>
                <button type="button" className="capcut-nav-tab" onClick={() => { setAiInstruction('Burn Subtitles'); setEditMode('ai'); }}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                  Subtitles
                </button>
              </div>
            </div>
            <div className="capcut-header-right">
              {outputVideoUrl && (
                <div style={{ display: 'flex', gap: 6, marginRight: 8 }}>
                  <button
                    type="button"
                    className={`capcut-nav-tab ${activePreview === 'input' ? 'is-active' : ''}`}
                    onClick={() => setActivePreview('input')}
                    title="View Source Video"
                  >
                    Source
                  </button>
                  <button
                    type="button"
                    className={`capcut-nav-tab ${activePreview === 'output' ? 'is-active' : ''}`}
                    onClick={() => setActivePreview('output')}
                    title="View Output Video"
                  >
                    Output Result ✓
                  </button>
                </div>
              )}
              <button
                type="button"
                className="capcut-export-btn"
                onClick={() => handleRunEdit()}
                disabled={!editInputFile.trim() || editRunning}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                {editRunning ? 'Exporting...' : 'Export Video'}
              </button>
            </div>
          </div>

          {/* 3-Pane Upper Workspace */}
          <div className="capcut-workspace-top">
            {/* Left Panel: Media Bin & Controls */}
            <div className="capcut-panel-left">
              <div className="capcut-section-title">Media & File Input</div>
              <div className="veo-input-file-row">
                <div className="veo-input-file-wrap">
                  <svg className="veo-input-file-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M23 7l-7 5 7 5V7z"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/></svg>
                  <input
                    id="veo-edit-input"
                    type="text"
                    className="veo-input-file-text"
                    placeholder="Select or enter video file path..."
                    value={editInputFile}
                    onChange={(e) => setEditInputFile(e.target.value)}
                    disabled={editRunning}
                  />
                  <input type="file" ref={fileInputRef} accept="video/*" style={{ display: 'none' }} onChange={handleFileSelect} />
                  <button type="button" className="veo-browse-btn" onClick={handleBrowseFile} disabled={editRunning}>Browse</button>
                </div>
                {editInputFile.trim() && (
                  <button type="button" className="veo-load-btn" onClick={handleLoadVideo} disabled={editRunning}>Inspect</button>
                )}
              </div>

              {!editInputFile.trim() && (
                <button
                  type="button"
                  className="veo-studio-chip"
                  style={{ width: '100%', marginTop: 6, justifyContent: 'center', background: 'color-mix(in srgb, var(--accent, #10b981) 10%, transparent)', color: 'var(--accent, #10b981)', border: '1px dashed var(--accent, #10b981)', gap: 6 }}
                  onClick={handleLoadDemoVideo}
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                  Load Demo Sample Video (30s 1080p)
                </button>
              )}

              {/* AI Prompt Mode Controls */}
              {editMode === 'ai' && (
                <>
                  <div className="capcut-section-title" style={{ marginTop: 10 }}>AI Prompt Instructions</div>
                  <textarea
                    className="veo-studio-textarea"
                    rows={3}
                    placeholder="e.g. Trim this video 0s to 30s, convert to vertical 9:16, and burn subtitles..."
                    value={aiInstruction}
                    onChange={(e) => setAiInstruction(e.target.value)}
                    disabled={editRunning}
                  />
                  <div className="veo-studio-chips">
                    <button type="button" className="veo-studio-chip" onClick={() => { setAiInstruction('Make Shorts 9:16'); setEditMode('ai'); }}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                      Shorts 9:16
                    </button>
                    <button type="button" className="veo-studio-chip" onClick={() => { setAiInstruction('Trim 0s - 30s'); setEditMode('ai'); }}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><line x1="20" y1="4" x2="8.12" y2="15.88"/><line x1="14.47" y1="14.48" x2="20" y2="20"/><line x1="8.12" y1="8.12" x2="12" y2="12"/></svg>
                      Trim 0s-30s
                    </button>
                    <button type="button" className="veo-studio-chip" onClick={() => { setAiInstruction('Burn Subtitles'); setEditMode('ai'); }}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                      Subtitles
                    </button>
                    <button type="button" className="veo-studio-chip" onClick={() => { setAiInstruction('Remove Silence'); setEditMode('ai'); }}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><line x1="23" y1="9" x2="17" y2="15"/><line x1="17" y1="9" x2="23" y2="15"/></svg>
                      No Silence
                    </button>
                    <button type="button" className="veo-studio-chip" onClick={() => { setAiInstruction('Auto Ducking'); setEditMode('ai'); }}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
                      Duck Music
                    </button>
                  </div>

                  <button
                    type="button"
                    className="veo-studio-generate-btn"
                    style={{ marginTop: 10, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
                    onClick={() => handleRunEdit()}
                    disabled={!editInputFile.trim() || editRunning}
                  >
                    {editRunning ? 'AI Processing...' : (
                      <>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
                        Execute with AI
                      </>
                    )}
                  </button>
                </>
              )}

              {/* Manual Tools Mode Controls */}
              {editMode === 'manual' && (
                <>
                  <div className="capcut-section-title" style={{ marginTop: 10 }}>Select Editing Tool</div>
                  <select
                    className="veo-studio-textarea"
                    style={{ padding: '6px 10px', height: '36px', marginBottom: 8, cursor: 'pointer' }}
                    value={editOperation}
                    onChange={(e) => setEditOperation(e.target.value)}
                  >
                    <option value="trim">Trim Clip (Start / End)</option>
                    <option value="resize">Resize & Aspect Ratio</option>
                    <option value="remove-silence">Remove Dead Air Silence</option>
                    <option value="extract-audio">Extract Audio Track</option>
                    <option value="transcribe">Speech Transcription (STT)</option>
                    <option value="burn-subtitles">Burn Subtitles onto Video</option>
                    <option value="make-shorts">Make Auto Shorts Clips</option>
                    <option value="export">Export Video</option>
                  </select>

                  {/* Dynamic Parameters per Tool */}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 6, background: 'var(--input-bg, rgba(0,0,0,0.2))', padding: 8, borderRadius: 6, fontSize: '0.78rem' }}>
                    {editOperation === 'trim' && (
                      <>
                        <label style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                          <span>Start (sec):</span>
                          <input
                            type="number" step="0.5" min="0" max={videoDuration}
                            style={{ width: 70, background: 'var(--bg-color, #0f172a)', border: '1px solid var(--border, #334155)', color: 'var(--text-main, #fff)', padding: '2px 4px', borderRadius: 4 }}
                            value={editParams.trimStart || 0}
                            onChange={(e) => setEditParams(p => ({ ...p, trimStart: e.target.value }))}
                          />
                        </label>
                        <button
                          type="button" className="veo-studio-chip" style={{ fontSize: '0.7rem' }}
                          onClick={() => setEditParams(p => ({ ...p, trimStart: currentTime.toFixed(1) }))}
                        >
                          Set Start = Current Playhead ({currentTime.toFixed(1)}s)
                        </button>
                        <label style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 4 }}>
                          <span>End (sec):</span>
                          <input
                            type="number" step="0.5" min="0" max={videoDuration}
                            style={{ width: 70, background: 'var(--bg-color, #0f172a)', border: '1px solid var(--border, #334155)', color: 'var(--text-main, #fff)', padding: '2px 4px', borderRadius: 4 }}
                            value={editParams.trimEnd ?? Math.round(videoDuration)}
                            onChange={(e) => setEditParams(p => ({ ...p, trimEnd: e.target.value }))}
                          />
                        </label>
                        <button
                          type="button" className="veo-studio-chip" style={{ fontSize: '0.7rem' }}
                          onClick={() => setEditParams(p => ({ ...p, trimEnd: currentTime.toFixed(1) }))}
                        >
                          Set End = Current Playhead ({currentTime.toFixed(1)}s)
                        </button>
                      </>
                    )}

                    {editOperation === 'resize' && (
                      <>
                        <div style={{ display: 'flex', gap: 4, marginBottom: 4 }}>
                          <button type="button" className="veo-studio-chip" onClick={() => setEditParams(p => ({ ...p, resizeWidth: 1920, resizeHeight: 1080 }))}>16:9 1080p</button>
                          <button type="button" className="veo-studio-chip" onClick={() => setEditParams(p => ({ ...p, resizeWidth: 1080, resizeHeight: 1920 }))}>9:16 Shorts</button>
                          <button type="button" className="veo-studio-chip" onClick={() => setEditParams(p => ({ ...p, resizeWidth: 1080, resizeHeight: 1080 }))}>1:1 Square</button>
                        </div>
                        <label style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                          <span>Width:</span>
                          <input type="number" style={{ width: 80, background: 'var(--bg-color, #0f172a)', border: '1px solid var(--border, #334155)', color: 'var(--text-main, #fff)', padding: '2px 4px' }} value={editParams.resizeWidth || 1920} onChange={(e) => setEditParams(p => ({ ...p, resizeWidth: e.target.value }))} />
                        </label>
                        <label style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                          <span>Height:</span>
                          <input type="number" style={{ width: 80, background: 'var(--bg-color, #0f172a)', border: '1px solid var(--border, #334155)', color: 'var(--text-main, #fff)', padding: '2px 4px' }} value={editParams.resizeHeight || 1080} onChange={(e) => setEditParams(p => ({ ...p, resizeHeight: e.target.value }))} />
                        </label>
                      </>
                    )}

                    {editOperation === 'remove-silence' && (
                      <>
                        <label>
                          Threshold ({editParams.thresholdDb || -30} dB):
                          <input type="range" min="-50" max="-10" value={editParams.thresholdDb || -30} onChange={(e) => setEditParams(p => ({ ...p, thresholdDb: e.target.value }))} style={{ width: '100%', accentColor: 'var(--accent, #10b981)' }} />
                        </label>
                        <label>
                          Min Silence ({editParams.minSilenceSecs || 0.5}s):
                          <input type="range" min="0.1" max="2.0" step="0.1" value={editParams.minSilenceSecs || 0.5} onChange={(e) => setEditParams(p => ({ ...p, minSilenceSecs: e.target.value }))} style={{ width: '100%', accentColor: 'var(--accent, #10b981)' }} />
                        </label>
                      </>
                    )}

                    {editOperation === 'burn-subtitles' && (
                      <>
                        <label style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                          <span>Style Preset:</span>
                          <select value={editParams.subPreset || 'tiktok'} onChange={(e) => setEditParams(p => ({ ...p, subPreset: e.target.value }))} style={{ background: 'var(--bg-color, #0f172a)', color: 'var(--text-main, #fff)', border: '1px solid var(--border, #334155)' }}>
                            <option value="tiktok">TikTok Bold Yellow</option>
                            <option value="minimal">Minimal White</option>
                            <option value="default">Standard Clean</option>
                          </select>
                        </label>
                      </>
                    )}

                    {editOperation === 'make-shorts' && (
                      <>
                        <label>
                          Clips Count ({editParams.maxShortsClips || 3}):
                          <input type="range" min="1" max="5" value={editParams.maxShortsClips || 3} onChange={(e) => setEditParams(p => ({ ...p, maxShortsClips: e.target.value }))} style={{ width: '100%', accentColor: 'var(--accent, #10b981)' }} />
                        </label>
                        <label>
                          Max Duration ({editParams.shortsDuration || 60}s):
                          <input type="range" min="15" max="60" step="5" value={editParams.shortsDuration || 60} onChange={(e) => setEditParams(p => ({ ...p, shortsDuration: e.target.value }))} style={{ width: '100%', accentColor: 'var(--accent, #10b981)' }} />
                        </label>
                      </>
                    )}
                  </div>

                  <button
                    type="button"
                    className="veo-studio-generate-btn"
                    style={{ marginTop: 10 }}
                    onClick={() => handleRunEdit()}
                    disabled={!editInputFile.trim() || editRunning}
                  >
                    {editRunning ? 'Processing Tool...' : `Execute ${editOperation.toUpperCase()}`}
                  </button>
                </>
              )}
            </div>

            {/* Center Panel: CapCut Synchronized Video Player Canvas */}
            <div className="capcut-player-center">
              <div className="capcut-video-canvas-container">
                <div className={`capcut-video-viewport ${editParams.resolution === '1080x1920' || editParams.resizeHeight > editParams.resizeWidth ? 'aspect-9-16' : ''}`}>
                  {activeVideoSrc ? (
                    <video
                      ref={videoRef}
                      className="capcut-video-element"
                      src={activeVideoSrc}
                      onTimeUpdate={(e) => setCurrentTime(e.currentTarget.currentTime)}
                      onLoadedMetadata={(e) => {
                        const d = e.currentTarget.duration
                        if (d && !isNaN(d) && isFinite(d)) setVideoDuration(d)
                      }}
                      onPlay={() => setIsPlaying(true)}
                      onPause={() => setIsPlaying(false)}
                      onEnded={() => setIsPlaying(false)}
                    />
                  ) : (
                    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--text-muted, #8f8f94)' }}>
                      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/></svg>
                      <span style={{ marginTop: 8, fontSize: '0.85rem' }}>No Video Loaded</span>
                      <span style={{ fontSize: '0.75rem', opacity: 0.7, marginTop: 4 }}>Select a file or click Load Demo Video on left panel</span>
                    </div>
                  )}
                </div>

                {/* Player Controls Bar */}
                <div className="capcut-player-controls-bar" style={{ gap: 8 }}>
                  <button
                    type="button"
                    onClick={handleTogglePlay}
                    disabled={!activeVideoSrc}
                    style={{ background: 'none', border: 'none', color: '#fff', cursor: 'pointer', display: 'flex', alignItems: 'center', padding: '2px 4px' }}
                    title={isPlaying ? 'Pause' : 'Play'}
                  >
                    {isPlaying ? (
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
                    ) : (
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                    )}
                  </button>

                  <button
                    type="button"
                    onClick={handleStopVideo}
                    disabled={!activeVideoSrc}
                    style={{ background: 'none', border: 'none', color: '#fff', cursor: 'pointer', display: 'flex', alignItems: 'center', padding: '2px 4px' }}
                    title="Stop"
                  >
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><rect x="4" y="4" width="16" height="16"/></svg>
                  </button>

                  <button
                    type="button"
                    onClick={() => handleSeekDelta(-5)}
                    disabled={!activeVideoSrc}
                    style={{ background: 'none', border: 'none', color: 'var(--text-muted, #94a3b8)', cursor: 'pointer', fontSize: '0.72rem' }}
                    title="Rewind 5s"
                  >
                    -5s
                  </button>

                  <button
                    type="button"
                    onClick={() => handleSeekDelta(5)}
                    disabled={!activeVideoSrc}
                    style={{ background: 'none', border: 'none', color: 'var(--text-muted, #94a3b8)', cursor: 'pointer', fontSize: '0.72rem' }}
                    title="Forward 5s"
                  >
                    +5s
                  </button>

                  <span className="capcut-timecode" style={{ fontFamily: 'monospace', fontWeight: 600, color: 'var(--accent, #10b981)' }}>
                    {formatTimecode(currentTime)} / {formatTimecode(videoDuration)}
                  </span>

                  <div style={{ marginLeft: 'auto', display: 'flex', gap: 8, alignItems: 'center' }}>
                    {activePreview === 'output' && (
                      <span style={{ fontSize: '0.7rem', color: 'var(--accent, #10b981)', background: 'color-mix(in srgb, var(--accent, #10b981) 15%, transparent)', padding: '2px 6px', borderRadius: 4, border: '1px solid color-mix(in srgb, var(--accent, #10b981) 30%, transparent)' }}>
                        Previewing Output Result
                      </span>
                    )}
                    <span style={{ fontSize: '0.75rem', color: 'var(--text-muted, #8f8f94)' }}>
                      {editVideoInfo ? `${editVideoInfo.width}×${editVideoInfo.height} (${editVideoInfo.fps.toFixed(0)} FPS)` : '1920×1080'}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* Right Panel: Inspector & Output Log */}
            <div className="capcut-inspector-right">
              <div className="capcut-section-title">Inspector & Parameters</div>
              {editVideoInfo ? (
                <div className="veo-editor-info-card">
                  <div className="veo-editor-info-row"><span>Resolution</span><strong>{editVideoInfo.width}×{editVideoInfo.height}</strong></div>
                  <div className="veo-editor-info-row"><span>Duration</span><strong>{editVideoInfo.duration.toFixed(2)}s</strong></div>
                  <div className="veo-editor-info-row"><span>FPS</span><strong>{editVideoInfo.fps.toFixed(2)}</strong></div>
                  <div className="veo-editor-info-row"><span>Format</span><strong>{editVideoInfo.format}</strong></div>
                  <div className="veo-editor-info-row"><span>Playhead Time</span><strong>{currentTime.toFixed(2)}s</strong></div>
                </div>
              ) : (
                <div style={{ fontSize: '0.78rem', color: 'var(--text-muted, #8f8f94)', fontStyle: 'italic' }}>Load a video file to view stream inspection metadata.</div>
              )}

              <div className="capcut-section-title" style={{ marginTop: 12 }}>Activity Log</div>
              <div className="veo-editor-log" style={{ flex: 1, minHeight: 140 }}>
                <div className="veo-editor-log-body">
                  {editLog.length === 0 ? (
                    <span className="veo-editor-log-empty">Activity log will appear here...</span>
                  ) : (
                    editLog.map((entry, i) => (
                      <div key={i} className={`veo-editor-log-entry ${entry.includes('✓') ? 'is-success' : entry.includes('✗') ? 'is-error' : ''}`}>{entry}</div>
                    ))
                  )}
                </div>
              </div>
            </div>
          </div>

          {/* Bottom Panel: Interactive Minimal Storyboard & Transcript Cards (No Multi-Track Lines) */}
          <div className="capcut-timeline-bottom" style={{ background: 'var(--panel-soft, var(--chrome-bg, #171718))', borderTop: '1px solid var(--border, rgba(255,255,255,0.08))', padding: '12px 16px', display: 'flex', flexDirection: 'column', gap: 10 }}>
            {/* Time Scrubber Slider & Quick Action Tools */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
              <button
                type="button"
                className="capcut-timeline-tool-btn"
                onClick={handleSplitAtPlayhead}
                title="Split clip at current playhead"
                style={{ padding: '6px 12px', background: 'var(--input-bg, rgba(255,255,255,0.05))', color: 'var(--text-main)', borderRadius: 6, border: '1px solid var(--border, rgba(255,255,255,0.1))', cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 6 }}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><line x1="20" y1="4" x2="8.12" y2="15.88"/><line x1="14.47" y1="14.48" x2="20" y2="20"/><line x1="8.12" y1="8.12" x2="12" y2="12"/></svg>
                Split ({currentTime.toFixed(1)}s)
              </button>
              <button
                type="button"
                className="capcut-timeline-tool-btn"
                onClick={handleDeleteSplitMarkers}
                title="Reset splits"
                style={{ padding: '6px 12px', background: 'var(--input-bg, rgba(255,255,255,0.05))', color: 'var(--text-main)', borderRadius: 6, border: '1px solid var(--border, rgba(255,255,255,0.1))', cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 6 }}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                Reset Splits ({splitMarkers.length})
              </button>

              {/* Range Scrubber */}
              <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 10, background: 'var(--input-bg, rgba(0,0,0,0.3))', padding: '6px 12px', borderRadius: 8, border: '1px solid var(--border, rgba(255,255,255,0.06))' }}>
                <span style={{ fontSize: '0.75rem', fontFamily: 'monospace', color: 'var(--accent, #10b981)', fontWeight: 600 }}>{formatTimecode(currentTime)}</span>
                <input
                  type="range"
                  min={0}
                  max={videoDuration || 30}
                  step={0.1}
                  value={currentTime}
                  onChange={(e) => {
                    const val = Number(e.target.value)
                    setCurrentTime(val)
                    if (videoRef.current) videoRef.current.currentTime = val
                  }}
                  style={{ flex: 1, accentColor: 'var(--accent, #10b981)', cursor: 'pointer' }}
                />
                <span style={{ fontSize: '0.75rem', fontFamily: 'monospace', color: 'var(--text-muted, #8f8f94)' }}>{formatTimecode(videoDuration)}</span>
              </div>
            </div>

            {/* Interactive Subtitle & Scene Transcript Cards Carousel */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '0.72rem', color: 'var(--text-muted, #8f8f94)', textTransform: 'uppercase', fontWeight: 700 }}>
                <span>Transcript & Scene Timeline Cards</span>
                <span>{editParams.segments ? `${editParams.segments.length} Segments` : 'Click card to jump to timestamp'}</span>
              </div>

              <div style={{ display: 'flex', gap: 8, overflowX: 'auto', paddingBottom: 4, scrollbarWidth: 'thin' }}>
                {editParams.segments && editParams.segments.length > 0 ? (
                  editParams.segments.map((seg: any) => (
                    <div
                      key={seg.id || seg.start}
                      onClick={() => {
                        if (videoRef.current) {
                          videoRef.current.currentTime = seg.start
                          setCurrentTime(seg.start)
                        }
                      }}
                      style={{
                        flexShrink: 0,
                        minWidth: 160,
                        maxWidth: 240,
                        background: currentTime >= seg.start && currentTime <= seg.end ? 'color-mix(in srgb, var(--accent, #10b981) 18%, transparent)' : 'var(--input-bg, rgba(255,255,255,0.04))',
                        border: currentTime >= seg.start && currentTime <= seg.end ? '1px solid var(--accent, #10b981)' : '1px solid var(--border, rgba(255,255,255,0.08))',
                        borderRadius: 6,
                        padding: '6px 10px',
                        cursor: 'pointer',
                        transition: 'all 0.15s ease',
                      }}
                    >
                      <div style={{ fontSize: '0.68rem', color: 'var(--accent, #10b981)', fontFamily: 'monospace', fontWeight: 600, marginBottom: 2, display: 'flex', alignItems: 'center', gap: 4 }}>
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                        {seg.start.toFixed(1)}s - {seg.end.toFixed(1)}s
                      </div>
                      <div style={{ fontSize: '0.78rem', color: 'var(--text-main, #fff)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', display: 'flex', alignItems: 'center', gap: 4 }}>
                        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                        {seg.text}
                      </div>
                    </div>
                  ))
                ) : (
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%', background: 'var(--input-bg, rgba(255,255,255,0.03))', padding: '8px 12px', borderRadius: 6, border: '1px dashed var(--border, rgba(255,255,255,0.1))' }}>
                    <span style={{ fontSize: '0.78rem', color: 'var(--text-muted, #8f8f94)', display: 'flex', alignItems: 'center', gap: 6 }}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                      No subtitle transcript cards generated yet.
                    </span>
                    <button
                      type="button"
                      className="veo-studio-chip"
                      onClick={() => { setEditOperation('transcribe'); setEditMode('manual'); handleRunEdit(); }}
                      disabled={editRunning || !editInputFile.trim()}
                      style={{ fontSize: '0.72rem', background: 'var(--accent, #10b981)', color: '#fff', display: 'inline-flex', alignItems: 'center', gap: 4 }}
                    >
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
                      Transcribe Speech to Cards
                    </button>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
