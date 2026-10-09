import { useEffect, useState } from 'react'

export function useProactiveSuggestions(onError: (message: string) => void) {
  const [proactiveSuggestion, setProactiveSuggestion] = useState<any>(null)

  useEffect(() => {
    const unlistenProactive = window.api.onProactiveSuggestion?.((suggestion: any) => {
      setProactiveSuggestion(suggestion)
    })
    return () => {
      unlistenProactive?.then?.((unlisten) => unlisten?.())
    }
  }, [])

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

  return { proactiveSuggestion, dismissProactiveSuggestion, handleProactiveAction }
}
