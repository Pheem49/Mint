import { useEffect, useState } from 'react'

/**
 * Live2D companion widget + proactive-suggestion state — desktop only.
 * Extracted out of MintDashboard.tsx as prep work for merging that
 * component with the web renderer's copy, which has neither feature
 * (no `ModelPanel`, no `window.api.onProactiveSuggestion`). Pure
 * extraction: every value/handler here behaves identically to when it
 * lived inline in MintDashboard.tsx.
 *
 * `handleModelInteraction` (turns a model poke into a chat message) and
 * `toastMessage`/`showToast` stay in MintDashboard.tsx itself — both
 * reach into dashboard-wide state (the chat-sending setters, the error
 * banner, `DashboardSidebar`'s toast trigger) that isn't part of the
 * widget's own concern.
 */
export function useCompanionWidget(onError: (message: string) => void) {
  const [modelVisible, setModelVisible] = useState(() => window.localStorage.getItem('mint:model-visible') !== 'false')
  const [scale, setScale] = useState(1.00)
  const [interactionEnabled, setInteractionEnabled] = useState(() => window.localStorage.getItem('mint:interaction-enabled') !== 'false')
  const [showInteractionGuide, setShowInteractionGuide] = useState(() => window.localStorage.getItem('mint:interaction-guide-visible') !== 'false')
  const [isLocked, setIsLocked] = useState(false)
  const [layoutPreset, setLayoutPreset] = useState<'chat-wide' | 'model-wide'>(() => (window.localStorage.getItem('mint:layout-preset') as 'chat-wide' | 'model-wide') || 'chat-wide')
  const [expressionIndex, setExpressionIndex] = useState(0)
  const [accessoryIndex, setAccessoryIndex] = useState(0)
  const [modelReady, setModelReady] = useState(false)
  const [proactiveSuggestion, setProactiveSuggestion] = useState<any>(null)

  useEffect(() => {
    const unlistenProactive = window.api.onProactiveSuggestion?.((suggestion: any) => {
      setProactiveSuggestion(suggestion)
    })
    return () => {
      unlistenProactive?.then?.((unlisten) => unlisten?.())
    }
  }, [])

  const toggleModel = () => {
    const next = !modelVisible
    window.localStorage.setItem('mint:model-visible', String(next))
    setModelVisible(next)
  }

  const changeLayoutPreset = (preset: 'chat-wide' | 'model-wide') => {
    window.localStorage.setItem('mint:layout-preset', preset)
    setLayoutPreset(preset)
  }

  const updateInteractionEnabled = (enabled: boolean) => {
    window.localStorage.setItem('mint:interaction-enabled', String(enabled))
    setInteractionEnabled(enabled)
  }

  const updateInteractionGuide = (visible: boolean) => {
    window.localStorage.setItem('mint:interaction-guide-visible', String(visible))
    setShowInteractionGuide(visible)
  }

  const dismissProactiveSuggestion = () => setProactiveSuggestion(null)

  const handleProactiveAction = async (action: any) => {
    setProactiveSuggestion(null)
    if (action && action.type !== 'none') {
      try {
        await window.api.executeProactiveAction(action)
      } catch (err) {
        onError(String(err))
      }
    }
  }

  return {
    modelVisible,
    scale,
    interactionEnabled,
    showInteractionGuide,
    isLocked,
    layoutPreset,
    expressionIndex,
    accessoryIndex,
    modelReady,
    proactiveSuggestion,
    setScale,
    setIsLocked,
    setExpressionIndex,
    setAccessoryIndex,
    setModelReady,
    toggleModel,
    changeLayoutPreset,
    updateInteractionEnabled,
    updateInteractionGuide,
    dismissProactiveSuggestion,
    handleProactiveAction,
  }
}
