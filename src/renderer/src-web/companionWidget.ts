// Resolves MintDashboard's companion-widget hook to a null implementation —
// web has no inline Live2D companion character or `window.api.onProactiveSuggestion`
// IPC event to react to. Desktop's copy of this file (src/companionWidget.ts)
// resolves the same `@/companionWidget` import to the real
// `useCompanionWidget` hook instead.
//
// Handlers are `undefined` (not no-ops) so DashboardSidebar's
// `{onToggleModel && (...)}`-style presence checks correctly hide the
// Live2D controls on web, rather than rendering inert buttons.
export function useCompanionWidget(_onError: (message: string) => void) {
  return {
    modelVisible: false,
    scale: 1,
    interactionEnabled: false,
    showInteractionGuide: false,
    isLocked: false,
    layoutPreset: 'chat-wide' as const,
    expressionIndex: 0,
    accessoryIndex: 0,
    // Never false on web — nothing here ever needs to "finish loading" —
    // so `startupReady = (dashboardDataReady && modelReady) || startupTimedOut`
    // never blocks startup on a Live2D model that will never exist.
    modelReady: true,
    proactiveSuggestion: null as any,
    setScale: undefined as ((scale: number) => void) | undefined,
    setIsLocked: undefined as ((locked: boolean) => void) | undefined,
    setExpressionIndex: undefined as ((index: number) => void) | undefined,
    setAccessoryIndex: undefined as ((index: number) => void) | undefined,
    setModelReady: undefined as ((ready: boolean) => void) | undefined,
    toggleModel: undefined as (() => void) | undefined,
    changeLayoutPreset: undefined as ((preset: 'chat-wide' | 'model-wide') => void) | undefined,
    updateInteractionEnabled: undefined as ((enabled: boolean) => void) | undefined,
    updateInteractionGuide: undefined as ((visible: boolean) => void) | undefined,
    dismissProactiveSuggestion: undefined as (() => void) | undefined,
    handleProactiveAction: undefined as ((action: any) => Promise<void>) | undefined,
  }
}
