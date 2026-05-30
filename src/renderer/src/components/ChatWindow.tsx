import React, { useEffect, useRef, useState } from 'react'
import * as PIXI from 'pixi.js'
import { Live2DModel } from 'pixi-live2d-display/cubism4'
import '../styles/styles.css'

window.PIXI = PIXI
Live2DModel.registerTicker(PIXI.Ticker)

interface Message {
    sender: 'user' | 'ai' | 'system'
    text: string
    timestamp?: string
    isTyping?: boolean
    approval?: any
    action?: any
}

interface SavedPicture {
    id: string
    filename: string
    url: string
    mimeType: string
    createdAt: string
    source: string
    message: string
}

const EXPRESSIONS = [
    { id: null, label: 'Normal' },
    { id: 'Apron', label: 'Apron' },
    { id: 'Dazed', label: 'Dazed' },
    { id: 'Photo', label: 'Photo' },
    { id: 'Glasses', label: 'Glasses' },
    { id: 'Pen', label: 'Writing' },
    { id: 'Click', label: 'Blush' },
    { id: 'CatFilter', label: 'Cat Ears' },
    { id: 'DazedEyes', label: 'Dazed Eyes' }
]

export default function ChatWindow() {
    // UI Layout states
    const [sidebarCollapsed, setSidebarCollapsed] = useState(true)
    const [activeTab, setActiveTab] = useState<'chat' | 'pictures'>('chat')
    const [pictures, setPictures] = useState<SavedPicture[]>([])
    const [themeConfig, setThemeConfig] = useState<any>({})

    // Chat states
    const [messages, setMessages] = useState<Message[]>([])
    const [inputText, setInputText] = useState('')
    const [imagePreview, setImagePreview] = useState<string | null>(null)
    const [smartContext, setSmartContext] = useState(false)
    const [agentMode, setAgentMode] = useState(false)
    const [providers, setProviders] = useState<string[]>([])
    const [activeProvider, setActiveProvider] = useState('gemini')

    // AI Activity states
    const [isLoading, setIsLoading] = useState(true)
    const [mintState, setMintState] = useState<'idle' | 'listening' | 'thinking' | 'speaking' | 'error'>('idle')
    const [modelTextStatus, setModelTextStatus] = useState('')
    const [expressionToast, setExpressionToast] = useState('')

    // Mic & Speech States
    const [isListening, setIsListening] = useState(false)
    const [voiceMode, setVoiceMode] = useState<'speech' | 'recorder' | null>(null)
    const [enableVoiceReply, setEnableVoiceReply] = useState(true)

    // Refs
    const chatContainerRef = useRef<HTMLDivElement>(null)
    const modelMountRef = useRef<HTMLDivElement>(null)
    const pixiAppRef = useRef<PIXI.Application | null>(null)
    const live2dModelRef = useRef<any>(null)
    const speechRecognitionRef = useRef<any>(null)
    const currentAudioPlayerRef = useRef<HTMLAudioElement | null>(null)
    
    // Live2D Accessory and position states
    const [modelLocked, setModelLocked] = useState(false)
    const [modelScale, setModelScale] = useState(100)
    const [activeAccessory, setActiveAccessory] = useState<string | null>(null)

    // Newly added UI controls matching original index.html
    const [modelVisible, setModelVisible] = useState(true)
    const [interactionEnabled, setInteractionEnabled] = useState(true)
    const [showInteractionGuide, setShowInteractionGuide] = useState(false)
    const [expressionIndex, setExpressionIndex] = useState(0)

    const expressionToastTimeoutRef = useRef<any>(null)
    const lastInteractionAtRef = useRef<number>(0)
    const lipSyncIntervalRef = useRef<any>(null)

    const modelLockedRef = useRef(modelLocked)
    const interactionEnabledRef = useRef(interactionEnabled)
    const activeAccessoryRef = useRef(activeAccessory)
    const modelScaleRef = useRef(modelScale)

    useEffect(() => { modelLockedRef.current = modelLocked }, [modelLocked])
    useEffect(() => { interactionEnabledRef.current = interactionEnabled }, [interactionEnabled])
    useEffect(() => { activeAccessoryRef.current = activeAccessory }, [activeAccessory])
    useEffect(() => { modelScaleRef.current = modelScale }, [modelScale])

    // Load configurations and theme
    useEffect(() => {
        if (window.api) {
            window.api.getSettings().then((cfg) => {
                setThemeConfig(cfg || {})
                setEnableVoiceReply(cfg?.enableVoiceReply !== false)
                setAgentMode(cfg?.assistantMode === 'agent')
                setActiveProvider(cfg?.aiProvider || 'gemini')
                applyTheme(cfg)
            })

            window.api.onSettingsChanged((cfg) => {
                setThemeConfig(cfg || {})
                setEnableVoiceReply(cfg?.enableVoiceReply !== false)
                setAgentMode(cfg?.assistantMode === 'agent')
                setActiveProvider(cfg?.aiProvider || 'gemini')
                applyTheme(cfg)
            })

            window.api.getChatHistory().then((history) => {
                const formatted = history.map((h) => ({
                    sender: h.sender === 'user' ? 'user' : 'ai',
                    text: h.text,
                    timestamp: h.createdAt
                }))
                setMessages(formatted as Message[])
            })

            window.api.onProactiveNotification((data) => {
                setMessages((prev) => [...prev, {
                    sender: 'system',
                    text: data.message
                }])
            })

            window.api.onVisionReady((base64Image) => {
                setImagePreview(base64Image)
            })

            window.api.onSpotlightToChat((query) => {
                setInputText(query)
                handleSendMessage(query)
            })
        }

        setProviders(['gemini', 'anthropic', 'openai', 'ollama', 'huggingface', 'local_openai'])
    }, [])

    // Scroll chat to bottom
    useEffect(() => {
        if (chatContainerRef.current) {
            chatContainerRef.current.scrollTop = chatContainerRef.current.scrollHeight
        }
    }, [messages])

    // Load Live2D Model
    useEffect(() => {
        const mountEl = modelMountRef.current
        if (!mountEl) return

        let active = true
        let app: PIXI.Application
        let model: any
        let resizeObserver: ResizeObserver

        // Safety timeout to dismiss loading screen after 6 seconds if it hangs
        const safetyTimeout = setTimeout(() => {
            if (active) {
                console.warn('Live2D model loading timed out. Dismissing loading screen.');
                setIsLoading(false);
            }
        }, 6000);

        const tracking = {
            targetX: 0,
            targetY: 0,
            currentX: 0,
            currentY: 0
        }

        const handleMouseMove = (event: MouseEvent) => {
            if (!active) return
            if (modelLockedRef.current || !interactionEnabledRef.current) return
            
            const rect = {
                left: 0,
                top: 0,
                width: window.innerWidth || mountEl.getBoundingClientRect().width,
                height: window.innerHeight || mountEl.getBoundingClientRect().height
            }
            const centerX = rect.left + rect.width * 0.35
            const centerY = rect.top + rect.height * 0.35
            const rangeX = Math.max(rect.width * 0.35, 1)
            const rangeY = Math.max(rect.height * 0.35, 1)

            tracking.targetX = Math.max(-1, Math.min(1, (event.clientX - centerX) / rangeX))
            tracking.targetY = Math.max(-1, Math.min(1, (event.clientY - centerY) / rangeY))
        }
        window.addEventListener('mousemove', handleMouseMove)

        const loadLive2D = async () => {
            try {
                app = new PIXI.Application({
                    autoDensity: true,
                    antialias: true,
                    backgroundAlpha: 0,
                    resizeTo: mountEl,
                    resolution: window.devicePixelRatio || 1
                })
                
                if (!active) {
                    app.destroy(true, { children: true, texture: true, baseTexture: true })
                    return
                }

                pixiAppRef.current = app
                mountEl.prepend(app.view)

                const modelUrl = new URL('../../models/Shiroko_Model/Shiroko/Shiroko_Core/%E9%9D%A2%E9%A5%BC0.model3.json', window.location.href).href
                model = await Live2DModel.from(modelUrl, { autoInteract: false })
                
                if (!active) {
                    if (model) model.destroy()
                    app.destroy(true, { children: true, texture: true, baseTexture: true })
                    return
                }

                live2dModelRef.current = model

                model.anchor.set(0.5, 0.5)
                app.stage.addChild(model)

                // Drag/Tracking/Fitting logic
                model.interactive = interactionEnabledRef.current
                model.buttonMode = interactionEnabledRef.current

                const fitModel = () => {
                    if (!active || !model) return
                    const mountWidth = mountEl.clientWidth || 460
                    const mountHeight = mountEl.clientHeight || 620
                    app.renderer.resize(mountWidth, mountHeight)

                    const internal = model.internalModel || {}
                    const modelWidth = internal.width || internal.originalWidth || model.width || 1
                    const modelHeight = internal.height || internal.originalHeight || model.height || 1
                    const widthScale = mountWidth / Math.max(modelWidth, 1)
                    const heightScale = mountHeight / Math.max(modelHeight, 1)

                    const scale = Math.min(widthScale, heightScale) * 1.85 * (modelScaleRef.current / 100)
                    model.scale.set(scale)
                    model.position.set(mountWidth / 2, mountHeight / 2 + mountHeight * 0.55)
                }

                requestAnimationFrame(() => {
                    fitModel()
                    requestAnimationFrame(fitModel)
                })

                resizeObserver = new ResizeObserver(fitModel)
                resizeObserver.observe(mountEl)

                // Mouse tracking loop
                const updateTracking = () => {
                    if (!active || !model) return
                    
                    const trackingEnabled = !modelLockedRef.current && interactionEnabledRef.current
                    if (!trackingEnabled) {
                        tracking.targetX = 0
                        tracking.targetY = 0
                    }

                    tracking.currentX += (tracking.targetX - tracking.currentX) * 0.18
                    tracking.currentY += (tracking.targetY - tracking.currentY) * 0.18

                    const x = tracking.currentX
                    const y = tracking.currentY
                    const core = model.internalModel?.coreModel
                    if (core) {
                        try { core.setParameterValueById('ParamAngleX', x * 18) } catch(_) {}
                        try { core.setParameterValueById('ParamAngleY', -y * 14) } catch(_) {}
                        try { core.setParameterValueById('ParamAngleZ', -x * 5) } catch(_) {}
                        try { core.setParameterValueById('ParamEyeBallX', x * 1.45) } catch(_) {}
                        try { core.setParameterValueById('ParamEyeBallY', -y * 1.35) } catch(_) {}
                        try { core.setParameterValueById('Param49', x * 7) } catch(_) {}
                        try { core.setParameterValueById('Param51', -y * 5) } catch(_) {}
                        try { core.setParameterValueById('Param50', -x * 3) } catch(_) {}
                    }

                    // Apply accessories
                    const currentAccessory = activeAccessoryRef.current
                    if (core) {
                        try { core.setParameterValueById('Param96', currentAccessory === 'glasses' ? 1 : 0) } catch(_) {}
                        try { core.setParameterValueById('Param68', currentAccessory === 'pen' ? 1 : 0) } catch(_) {}
                        try { core.setParameterValueById('Param54', currentAccessory === 'cat' ? 1 : 0) } catch(_) {}
                    }

                    const mountWidth = mountEl.clientWidth || 460
                    const mountHeight = mountEl.clientHeight || 620
                    const baseX = mountWidth / 2
                    const baseY = mountHeight / 2 + mountHeight * 0.55
                    model.position.set(baseX + x * 22, baseY + y * 16)
                }
                app.ticker.add(updateTracking)

                // Model Tap interaction
                model.on('pointertap', (e: any) => {
                    if (!active || !model || !interactionEnabledRef.current) return

                    const now = Date.now()
                    if (now - lastInteractionAtRef.current < 3000) return

                    const originalEvent = e.data?.originalEvent
                    const canvasRect = app.view.getBoundingClientRect()
                    if (!originalEvent || !canvasRect) return

                    const pointX = (originalEvent.clientX - canvasRect.left) / Math.max(canvasRect.width, 1)
                    const pointY = (originalEvent.clientY - canvasRect.top) / Math.max(canvasRect.height, 1)

                    const zoom = (modelScaleRef.current / 100) || 1
                    const originX = 0.5
                    const originY = 0.58
                    const x = originX + (pointX - originX) / zoom
                    const y = originY + (pointY - originY) / zoom

                    let region: any = null
                    const isPointInZone = (px: number, py: number, left: number, top: number, w: number, h: number) => {
                        return px >= left && px <= left + w && py >= top && py <= top + h
                    }

                    if (isPointInZone(x, y, 0.36, 0.375, 0.28, 0.12)) {
                        region = {
                            id: 'face',
                            label: 'Cheek Poke',
                            expression: 'CatFilter',
                            prompt: 'The user poked Mint model on the cheek. Reply briefly, shyly or with a light tease. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                        }
                    } else if (isPointInZone(x, y, 0.34, 0.205, 0.32, 0.155)) {
                        region = {
                            id: 'head',
                            label: 'Head Pat',
                            expression: 'Dazed',
                            prompt: 'The user patted Mint model on the head. Reply briefly in a cute, slightly shy way. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                        }
                    } else if (isPointInZone(x, y, 0.14, 0.70, 0.22, 0.16) || isPointInZone(x, y, 0.65, 0.69, 0.23, 0.17)) {
                        region = {
                            id: 'hand',
                            label: 'Hand Tap',
                            expression: 'Pen',
                            prompt: 'The user tapped Mint model’s hand. Reply briefly as if ready to help or take a request. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                        }
                    } else if (isPointInZone(x, y, 0.34, 0.74, 0.30, 0.24)) {
                        region = {
                            id: 'lower-body',
                            label: 'Careful',
                            expression: 'Photo',
                            prompt: 'The user touched the lower body area of Mint model. Reply briefly in a shy, playful way, similar to “hehe~ what are you playing at, that makes me blush,” then gently invite the user back to chatting or work. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                        }
                    } else if (isPointInZone(x, y, 0.36, 0.53, 0.29, 0.145)) {
                        region = {
                            id: 'body',
                            label: 'Shoulder Tap',
                            expression: 'Click',
                            prompt: 'The user tapped Mint model’s body or shoulder. Reply briefly as if turning toward the user and asking what they need help with. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                        }
                    }

                    if (region) {
                        lastInteractionAtRef.current = now
                        
                        try {
                            model.expression(region.expression)
                            setTimeout(() => {
                                if (active && model) {
                                    model.expression(null)
                                }
                            }, 2000)
                        } catch(err) {
                            console.error(err)
                        }

                        setExpressionToast(region.label)
                        setTimeout(() => setExpressionToast(''), 2500)

                        handleSendMessage(region.prompt)
                    }
                })

                model.motion('Idle', 0).catch(() => {})
                clearTimeout(safetyTimeout)
                if (active) setIsLoading(false)
            } catch (err) {
                clearTimeout(safetyTimeout)
                if (active) {
                    console.error('Failed to load Live2D model:', err)
                    setModelTextStatus('Live2D model unavailable.')
                    setIsLoading(false)
                }
            }
        }

        const timer = setTimeout(() => {
            loadLive2D()
        }, 500)

        return () => {
            active = false
            clearTimeout(timer)
            clearTimeout(safetyTimeout)
            window.removeEventListener('mousemove', handleMouseMove)
            if (resizeObserver) resizeObserver.disconnect()
            if (app) app.destroy(true, { children: true, texture: true, baseTexture: true })
            if (lipSyncIntervalRef.current) clearInterval(lipSyncIntervalRef.current)
        }
    }, [])

    // Rescale model when scale changes reactively
    useEffect(() => {
        if (live2dModelRef.current && modelMountRef.current) {
            const mountEl = modelMountRef.current
            const model = live2dModelRef.current
            const mountWidth = mountEl.clientWidth || 460
            const mountHeight = mountEl.clientHeight || 620
            
            const internal = model.internalModel || {}
            const modelWidth = internal.width || internal.originalWidth || model.width || 1
            const modelHeight = internal.height || internal.originalHeight || model.height || 1
            const widthScale = mountWidth / Math.max(modelWidth, 1)
            const heightScale = mountHeight / Math.max(modelHeight, 1)

            const scale = Math.min(widthScale, heightScale) * 1.85 * (modelScale / 100)
            model.scale.set(scale)
            model.position.set(mountWidth / 2, mountHeight / 2 + mountHeight * 0.55)
        }
    }, [modelScale])

    // Update model interactivity reactively
    useEffect(() => {
        if (live2dModelRef.current) {
            live2dModelRef.current.interactive = interactionEnabled
            live2dModelRef.current.buttonMode = interactionEnabled
        }
    }, [interactionEnabled])

    const applyTheme = (cfg: any) => {
        document.documentElement.setAttribute('data-theme', cfg.theme || 'dark')
        const accent = cfg.accentColor || '#8b6cf5'
        document.documentElement.style.setProperty('--accent', accent)
        document.documentElement.style.setProperty('--text-main', cfg.systemTextColor || '#e8e8ea')
        document.documentElement.style.setProperty('--glass-blur', cfg.glassBlur || 'blur(16px)')
        document.body.style.fontFamily = cfg.fontFamily || "'Outfit', sans-serif"
    }

    // Voice Input Setup
    const setupMic = async () => {
        const SpeechRecognitionCtor = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
        if (SpeechRecognitionCtor) {
            const recognition = new SpeechRecognitionCtor()
            recognition.lang = 'th-TH'
            recognition.interimResults = true
            recognition.continuous = false

            recognition.onstart = () => {
                setIsListening(true)
                setMintState('listening')
            }

            recognition.onresult = (event: any) => {
                let interim = ''
                let final = ''
                for (let i = event.resultIndex; i < event.results.length; i++) {
                    if (event.results[i].isFinal) {
                        final += event.results[i][0].transcript
                    } else {
                        interim += event.results[i][0].transcript
                    }
                }
                if (final.trim()) {
                    handleSendMessage(final.trim())
                    recognition.stop()
                } else {
                    setInputText(interim)
                }
            }

            recognition.onend = () => {
                setIsListening(false)
                setMintState('idle')
            }

            speechRecognitionRef.current = recognition
        }
    }

    const handleMicClick = async () => {
        if (!speechRecognitionRef.current) {
            await setupMic()
        }

        const recognition = speechRecognitionRef.current
        if (!recognition) return

        if (isListening) {
            recognition.stop()
        } else {
            recognition.start()
        }
    }

    const handleSendMessage = async (textToSend = inputText) => {
        const text = textToSend.trim()
        if (!text && !imagePreview) return

        // If it's a model interaction prompt, do not add it directly to messages feed as a clean chat interaction
        const isSystemPrompt = text.startsWith("The user patted") || text.startsWith("The user poked") || text.startsWith("The user touched") || text.startsWith("The user tapped")
        
        if (!isSystemPrompt) {
            setMessages((prev) => [...prev, { sender: 'user', text }])
        }
        setInputText('')
        setImagePreview(null)
        setMintState('thinking')

        // Add dummy typing message
        setMessages((prev) => [...prev, { sender: 'ai', text: '', isTyping: true }])

        try {
            const response = await window.api.sendMessage(text, imagePreview, null)
            setMessages((prev) => prev.filter((m) => !m.isTyping)) // remove typing indicator

            setMessages((prev) => [...prev, {
                sender: 'ai',
                text: response.response,
                approval: response.approval,
                action: response.action
            }])

            setMintState('idle')

            if (enableVoiceReply && response.response) {
                speakText(response.response)
            }
        } catch (err) {
            console.error(err)
            setMessages((prev) => prev.filter((m) => !m.isTyping))
            setMessages((prev) => [...prev, { sender: 'ai', text: 'Error contacting AI. Check settings.' }])
            setMintState('error')
        }
    }

    // TTS Voice
    const speakText = async (text: string) => {
        setMintState('speaking')
        
        const startLipSync = () => {
            const model = live2dModelRef.current
            if (!model) return
            model.motion('Speak', 0).catch(() => {})
            
            if (lipSyncIntervalRef.current) clearInterval(lipSyncIntervalRef.current)
            lipSyncIntervalRef.current = setInterval(() => {
                const value = Math.random() * 0.8
                const core = model.internalModel?.coreModel
                if (core) {
                    const mouthIds = ['ParamMouthOpenY', 'ParamMouthOpen', 'PARAM_MOUTH_OPEN_Y']
                    mouthIds.forEach(id => {
                        try { core.setParameterValueById(id, value) } catch(e) {}
                    })
                }
            }, 80)
        }

        const stopLipSync = () => {
            if (lipSyncIntervalRef.current) {
                clearInterval(lipSyncIntervalRef.current)
                lipSyncIntervalRef.current = null
            }
            const model = live2dModelRef.current
            if (model) {
                const core = model.internalModel?.coreModel
                if (core) {
                    const mouthIds = ['ParamMouthOpenY', 'ParamMouthOpen', 'PARAM_MOUTH_OPEN_Y']
                    mouthIds.forEach(id => {
                        try { core.setParameterValueById(id, 0) } catch(e) {}
                    })
                }
                model.motion('Idle', 0).catch(() => {})
            }
        }

        try {
            const urls = await window.api.getTtsUrls(text)
            if (urls && urls.length > 0) {
                if (currentAudioPlayerRef.current) {
                    currentAudioPlayerRef.current.pause()
                }
                const audio = new Audio(urls[0].url)
                currentAudioPlayerRef.current = audio
                
                audio.addEventListener('play', startLipSync)
                audio.addEventListener('pause', stopLipSync)
                audio.addEventListener('ended', () => {
                    stopLipSync()
                    setMintState('idle')
                })
                
                audio.play()
            } else {
                setMintState('idle')
            }
        } catch (e) {
            console.error(e)
            setMintState('idle')
        }
    }

    const handleNewChat = async () => {
        await window.api.resetChat()
        setMessages([])
    }

    const handleOpenSettings = () => {
        window.api.openSettings()
    }

    const handleImageRemove = () => {
        setImagePreview(null)
    }

    const handleProviderChange = async (provider: string) => {
        setActiveProvider(provider)
        const updatedConfig = { ...themeConfig, aiProvider: provider }
        await window.api.saveSettings(updatedConfig)
    }

    const toggleSmartContext = () => {
        setSmartContext(!smartContext)
    }

    const toggleAgentMode = async () => {
        const nextMode = !agentMode
        setAgentMode(nextMode)
        const updatedConfig = { ...themeConfig, assistantMode: nextMode ? 'agent' : 'chat' }
        await window.api.saveSettings(updatedConfig)
    }

    const cycleAccessory = () => {
        const accessories = ['glasses', 'pen', 'cat']
        const currentIdx = accessories.indexOf(activeAccessory || '')
        const nextIdx = (currentIdx + 1) % (accessories.length + 1)
        const nextAccessory = nextIdx === accessories.length ? null : accessories[nextIdx]
        setActiveAccessory(nextAccessory)
    }

    const cycleExpression = () => {
        const nextIdx = (expressionIndex + 1) % EXPRESSIONS.length
        setExpressionIndex(nextIdx)
        const exp = EXPRESSIONS[nextIdx]

        if (live2dModelRef.current) {
            try {
                live2dModelRef.current.expression(exp.id)
            } catch (err) {
                console.error(err)
            }
        }
        setExpressionToast(`Expression: ${exp.label}`)
        if (expressionToastTimeoutRef.current) clearTimeout(expressionToastTimeoutRef.current)
        expressionToastTimeoutRef.current = setTimeout(() => {
            setExpressionToast('')
        }, 1600)
    }

    const handleSuggestionClick = (query: string) => {
        setInputText(query)
        const inputEl = document.getElementById('chat-input')
        if (inputEl) inputEl.focus()
    }

    return (
        <div className={`app-container ${activeTab === 'pictures' ? 'pictures-open' : ''} ${isLoading ? 'is-loading' : ''}`}>
            {isLoading && (
                <div className="startup-loading" id="startup-loading" aria-live="polite">
                    <div className="startup-loading-content">
                        <div className="startup-loading-dots" aria-hidden="true">
                            <span></span>
                            <span></span>
                            <span></span>
                        </div>
                        <div className="startup-loading-text">Loading Agent Mint</div>
                    </div>
                </div>
            )}
            <header className="drag-region">
                <div className="header-content">
                    <img className="app-logo" src="../../assets/icon.png" alt="Mint" draggable="false" />
                    <h1>Agent Mint</h1>
                </div>
                <div className="titlebar-drag-space" aria-hidden="true"></div>
                <div className="window-controls" aria-label="Window controls">
                    <button className="minimize-btn" onClick={() => window.api.minimizeWindow()} title="Minimize to Tray">
                        <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none"
                            stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <line x1="5" y1="12" x2="19" y2="12"></line>
                        </svg>
                    </button>
                    <button className="maximize-btn" onClick={() => window.api.maximizeWindow()} title="Maximize">
                        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none"
                            stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                        </svg>
                    </button>
                    <button className="close-btn" onClick={() => window.api.quitApp()} title="Close">
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                            stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <line x1="18" y1="6" x2="6" y2="18"></line>
                            <line x1="6" y1="6" x2="18" y2="18"></line>
                        </svg>
                    </button>
                </div>
            </header>

            <div className={`app-body ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
                <button
                    className="sidebar-toggle"
                    onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
                    aria-label="Toggle sidebar"
                >
                    <svg className="sidebar-toggle-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                        stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                        <path d="M15 18l-6-6 6-6"></path>
                    </svg>
                </button>

                <aside className="workspace-sidebar" aria-label="Workspace navigation">
                    <button className="sidebar-new-chat" onClick={handleNewChat} title="Start a new chat and clear current history">
                        <span aria-hidden="true">+</span>
                        <span>New Chat</span>
                    </button>
                    <button className={`sidebar-chat-btn sidebar-top-action ${activeTab === 'chat' ? 'is-active' : ''}`} onClick={() => setActiveTab('chat')} title="Chat">
                        <span aria-hidden="true">▣</span>
                        <span>Chat</span>
                    </button>
                    <button className={`sidebar-pictures-btn sidebar-top-action ${activeTab === 'pictures' ? 'is-active' : ''}`} onClick={async () => {
                        setActiveTab('pictures')
                        const pics = await window.api.listSavedPictures()
                        setPictures(pics as SavedPicture[])
                    }} title="Pictures">
                        <span aria-hidden="true">▧</span>
                        <span>Pictures</span>
                    </button>

                    <div className="sidebar-model-controls" aria-label="Model controls">
                        <button 
                            className={`toggle-model-btn ${modelVisible ? 'active' : ''}`} 
                            onClick={() => setModelVisible(!modelVisible)}
                            aria-label="Toggle Model" 
                            title="Toggle Model Visibility"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <line x1="3" y1="12" x2="21" y2="12"></line>
                                <line x1="3" y1="6" x2="21" y2="6"></line>
                                <line x1="3" y1="18" x2="21" y2="18"></line>
                            </svg>
                            <span>Model</span>
                        </button>
                        <button 
                            className="change-expression-btn" 
                            onClick={cycleExpression}
                            aria-label="Change Expression" 
                            title="Change Shiroko's Expression"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <path d="M12 3l1.912 5.886L20 10.8l-4.544 3.414L17.368 21 12 17.186 6.632 21l1.912-6.786L4 10.8l6.088-1.914z"></path>
                            </svg>
                            <span>Expression</span>
                        </button>
                        <button 
                            className="accessory-cycle-btn" 
                            onClick={cycleAccessory}
                            aria-label="Cycle Accessory" 
                            title="Cycle Accessory"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <circle cx="7" cy="15" r="4"></circle>
                                <circle cx="17" cy="15" r="4"></circle>
                                <path d="M11 15h2"></path>
                                <path d="M3 15H2"></path>
                                <path d="M22 15h-1"></path>
                            </svg>
                            <span>Accessory</span>
                        </button>
                        <button 
                            className={`toggle-interaction-btn ${interactionEnabled ? 'active' : ''}`} 
                            onClick={() => setInteractionEnabled(!interactionEnabled)}
                            aria-label="Toggle Model Interaction" 
                            title="Toggle Model Interaction"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <path d="M7 11v-1a2 2 0 0 1 4 0v1"></path>
                                <path d="M11 10V8a2 2 0 0 1 4 0v4"></path>
                                <path d="M15 11a2 2 0 0 1 4 0v2a7 7 0 0 1-14 0v-1a2 2 0 0 1 4 0"></path>
                                {!interactionEnabled && <path className="interaction-off-mark" d="M4 4l16 16"></path>}
                            </svg>
                            <span>Interact</span>
                        </button>
                        <button 
                            className={`interaction-guide-btn ${showInteractionGuide ? 'active' : ''}`} 
                            onClick={() => setShowInteractionGuide(!showInteractionGuide)}
                            aria-label="Show Interaction Areas" 
                            title="Show Interaction Areas"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <circle cx="12" cy="12" r="3"></circle>
                                <path d="M12 2v3"></path>
                                <path d="M12 19v3"></path>
                                <path d="M2 12h3"></path>
                                <path d="M19 12h3"></path>
                            </svg>
                            <span>Areas</span>
                        </button>
                    </div>

                    <div className="sidebar-section">
                        <div className="sidebar-section-title">Assistant</div>
                        <div className="sidebar-project active">
                            <span>Mint</span>
                            <span className="mint-status-pill" data-state={mintState}>
                                <span className="mint-status-dot"></span>
                                <span className="mint-status-label">{mintState.toUpperCase()}</span>
                            </span>
                        </div>
                    </div>

                    <div className="sidebar-bottom-actions" aria-label="Chat and app controls">
                        <button className="clear-btn" onClick={handleNewChat} title="Clear chat history">
                            <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <polyline points="3 6 5 6 21 6"></polyline>
                                <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"></path>
                                <path d="M10 11v6"></path>
                                <path d="M14 11v6"></path>
                                <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"></path>
                            </svg>
                            <span>Clear</span>
                        </button>
                        <button className="settings-btn" onClick={handleOpenSettings} title="Settings">
                            <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <circle cx="12" cy="12" r="3"></circle>
                                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
                            </svg>
                            <span>Settings</span>
                        </button>
                    </div>
                </aside>

                <div className={`assistant-workspace ${!modelVisible ? 'model-hidden' : ''}`}>
                    <section className="model-stage" aria-label="Assistant model">
                        <div className={`model-shell ${showInteractionGuide ? 'show-interaction-guide' : ''}`} id="model-shell">
                            <div className="model-glow"></div>
                            
                            <div className="model-activity-badge" data-state={mintState} title={`Mint is ${mintState}`}>
                                <span className="mint-status-dot" aria-hidden="true"></span>
                                <span className="mint-status-label">{mintState}</span>
                            </div>

                            <div className="model-panel-controls" aria-label="Live2D controls">
                                <button 
                                    className={`model-panel-control ${modelLocked ? 'active' : ''}`} 
                                    onClick={() => setModelLocked(!modelLocked)}
                                    type="button"
                                    title="Lock model position" 
                                    aria-label="Lock model position"
                                >
                                    <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none"
                                        stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                                        <rect x="5" y="11" width="14" height="10" rx="2"></rect>
                                        <path d="M8 11V7a4 4 0 0 1 8 0v4"></path>
                                    </svg>
                                </button>
                                <label className="model-scale-control" title="Model size">
                                    <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none"
                                        stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                                        <path d="M15 3h6v6"></path>
                                        <path d="M9 21H3v-6"></path>
                                        <path d="M21 3l-7 7"></path>
                                        <path d="M3 21l7-7"></path>
                                    </svg>
                                    <input 
                                        type="range" 
                                        min="78" 
                                        max="128" 
                                        value={modelScale} 
                                        onChange={(e) => setModelScale(parseInt(e.target.value))}
                                        aria-label="Model size" 
                                    />
                                    <span className="model-scale-value">{(modelScale / 100).toFixed(2)}x</span>
                                    <button 
                                        className="model-scale-reset" 
                                        onClick={() => setModelScale(100)}
                                        type="button"
                                        title="Reset size to 1.00x" 
                                        aria-label="Reset model size"
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none"
                                            stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                                            <path d="M3 12a9 9 0 1 0 3-6.7"></path>
                                            <path d="M3 4v6h6"></path>
                                        </svg>
                                    </button>
                                </label>
                            </div>

                            <div className="model-mount" ref={modelMountRef} id="model-mount">
                                {modelTextStatus && <div className="model-status">{modelTextStatus}</div>}
                            </div>

                            {expressionToast && (
                                <div className="expression-toast is-visible" id="expression-toast" aria-live="polite">
                                    {expressionToast}
                                </div>
                            )}

                            <div className="interaction-guide" id="interaction-guide" aria-hidden="true">
                                <div className="interaction-zone zone-head"><span>Head Pat</span></div>
                                <div className="interaction-zone zone-face"><span>Cheek Poke</span></div>
                                <div className="interaction-zone zone-left-hand"><span>Hand Tap</span></div>
                                <div className="interaction-zone zone-right-hand"><span>Hand Tap</span></div>
                                <div className="interaction-zone zone-body"><span>Shoulder Tap</span></div>
                                <div className="interaction-zone zone-lower"><span>Careful</span></div>
                            </div>
                            
                            <div className="model-shadow"></div>
                        </div>
                    </section>

                    <div className="conversation-panel">
                        <main className="chat-container" ref={chatContainerRef}>
                            {messages.length === 0 && (
                                <div className="message ai-message initial" style={{ display: 'block', opacity: 1, width: '100%', maxWidth: '100%' }}>
                                    <div className="bubble-wrapper" style={{ width: '100%', maxWidth: '100%' }}>
                                        <div className="message-bubble welcome-bubble" style={{ marginBottom: '20px', alignSelf: 'flex-start' }}>
                                            Hello! I'm Mint, your personal AI assistant ✨<br />
                                            Is there anything I can help you with? You can ask me to write code, manage workspaces, or run commands!
                                        </div>
                                        <div className="initial-suggestions">
                                            <div className="suggestion-card" onClick={() => handleSuggestionClick('Create a simple React website')}>
                                                <span className="suggestion-icon" aria-hidden="true">🚀</span>
                                                <span className="suggestion-text">Create React App</span>
                                            </div>
                                            <div className="suggestion-card" onClick={() => handleSuggestionClick('Get a summary of the current repository')}>
                                                <span className="suggestion-icon" aria-hidden="true">📊</span>
                                                <span className="suggestion-text">Repo Summary</span>
                                            </div>
                                            <div className="suggestion-card" onClick={() => handleSuggestionClick('Search code for TODO')}>
                                                <span className="suggestion-icon" aria-hidden="true">🔍</span>
                                                <span className="suggestion-text">Search Code</span>
                                            </div>
                                            <div className="suggestion-card" onClick={() => handleSuggestionClick('Show system statistics')}>
                                                <span className="suggestion-icon" aria-hidden="true">⚡</span>
                                                <span className="suggestion-text">System Stats</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            )}
                            {messages.map((m, idx) => (
                                <div key={idx} className={`message ${m.sender === 'user' ? 'user-message' : 'ai-message'}`}>
                                    <div className="bubble-wrapper">
                                        <div className="message-bubble">
                                            {m.isTyping ? (
                                                <div className="loader-dots">
                                                    <span></span><span></span><span></span>
                                                </div>
                                            ) : (
                                                m.text
                                            )}
                                        </div>
                                    </div>
                                </div>
                            ))}
                        </main>

                        <footer className="input-area">
                            {imagePreview && (
                                <div id="image-preview-container" style={{ display: 'block', padding: '10px', background: 'rgba(0,0,0,0.2)', borderRadius: '8px 8px 0 0', position: 'relative' }}>
                                    <img id="image-preview" src={imagePreview} alt="upload preview" style={{ maxHeight: '80px', maxWidth: '100%', borderRadius: '4px' }} />
                                    <button id="remove-image-btn" type="button" onClick={handleImageRemove} style={{ position: 'absolute', top: '5px', right: '10px', background: 'red', color: 'white', border: 'none', borderRadius: '50%', width: '20px', height: '20px', cursor: 'pointer' }}>&times;</button>
                                </div>
                            )}

                            <div className="smart-context-bar">
                                <div className="smart-context-control">
                                    <label className="toggle-switch">
                                        <input type="checkbox" checked={smartContext} onChange={toggleSmartContext} />
                                        <span className="slider round"></span>
                                    </label>
                                    <span className="smart-context-label">Smart Context <span>(Auto-Screen)</span></span>
                                </div>
                                <div className="smart-context-control">
                                    <label className="toggle-switch">
                                        <input type="checkbox" checked={agentMode} onChange={toggleAgentMode} />
                                        <span className="slider round"></span>
                                    </label>
                                    <span className="smart-context-label">Agent Mode</span>
                                </div>
                            </div>

                            <form id="chat-form" onSubmit={(e) => { e.preventDefault(); handleSendMessage(); }}>
                                <button type="button" id="vision-btn" onClick={() => window.api.startVision()} aria-label="Screen Vision" title="Screen Vision">
                                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path>
                                        <circle cx="12" cy="12" r="3"></circle>
                                    </svg>
                                </button>
                                <select
                                    id="chat-provider-select"
                                    className="chat-provider-select"
                                    value={activeProvider}
                                    onChange={(e) => handleProviderChange(e.target.value)}
                                >
                                    {providers.map((p) => (
                                        <option key={p} value={p}>{p.toUpperCase()}</option>
                                    ))}
                                </select>
                                <input
                                    type="text"
                                    id="chat-input"
                                    value={inputText}
                                    onChange={(e) => setInputText(e.target.value)}
                                    placeholder="Ask anything, @ to mention, / for actions"
                                    autoComplete="off"
                                />
                                <button type="button" id="mic-btn" className={isListening ? 'listening' : ''} onClick={handleMicClick} aria-label="Microphone">
                                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none"
                                        stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                        <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z"></path>
                                        <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
                                        <line x1="12" y1="19" x2="12" y2="23"></line>
                                        <line x1="8" y1="23" x2="16" y2="23"></line>
                                    </svg>
                                </button>
                                <button type="submit" id="send-btn">
                                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none"
                                        stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                        <line x1="22" y1="2" x2="11" y2="13"></line>
                                        <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
                                    </svg>
                                </button>
                            </form>
                        </footer>
                    </div>
                </div>

                {activeTab === 'pictures' && (
                    <section className="pictures-library" id="pictures-library">
                        <div className="pictures-header">
                            <div>
                                <p className="pictures-kicker">Local Library</p>
                                <h2>Pictures</h2>
                            </div>
                            <button className="pictures-close-btn" onClick={() => setActiveTab('chat')}>Back to Chat</button>
                        </div>
                        {pictures.length === 0 ? (
                            <div className="pictures-empty">
                                <p>No saved pictures yet.</p>
                            </div>
                        ) : (
                            <div className="pictures-grid">
                                {pictures.map((pic) => (
                                    <article className="picture-card" key={pic.id}>
                                        <img src={pic.url} alt={pic.filename} />
                                        <div className="picture-card-meta">{pic.message || pic.filename}</div>
                                    </article>
                                ))}
                            </div>
                        )}
                    </section>
                )}
            </div>
        </div>
    )
}
