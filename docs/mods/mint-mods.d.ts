declare module '@mint/mods' {
  export type Slot = 'above_composer' | 'side_panel'
  export type Color = 'default' | 'yellow' | 'cyan' | 'blue' | 'magenta' | 'red'
  export type Node = { type: 'text' | 'stack' | 'badge' | 'button' | 'input' | 'progress' | 'table'; color?: Color; direction?: 'row' | 'column'; text?: string; label?: string; action?: string; value?: string | number; max?: number; columns?: string[]; rows?: (string | number | null)[][]; children?: Node[] }
  export type Event = 'session_start' | 'session_end' | 'turn_start' | 'turn_end' | 'pre_tool_use' | 'post_tool_use' | 'usage_update' | 'context_compacted' | 'ui_resize' | 'tool_result'
  export interface Viewport {
    surface: 'terminal' | 'desktop' | 'web' | null
    /** Exact text columns in CLI; estimated from pixels on graphical surfaces. */
    columns: number | null
    rows: number | null
    widthPx: number | null
    heightPx: number | null
    sidePanelOpen: boolean
  }
  export interface ContextReading {
    tokens: number | null
    window: number | null
    percent: number | null
    basis: 'input' | 'input_output'
    lastTurnDelta: number | null
  }
  export interface UsageEvent {
    turnId: number
    /** Accumulated API usage for this turn, not context occupancy. */
    totalTokens: number | null
    /** Latest provider-reported input + output; null after compaction or missing usage. */
    contextUsedTokens: number | null
    contextInputTokens: number | null
    contextOutputTokens: number | null
    /** Mint's configured/conservative planning window, not a provider capacity discovery. */
    contextWindowTokens: number | null
    contextWindowSource: 'mint_config'
    contextPercent: number | null
    contextStatus: 'measured' | 'unavailable' | 'compacted'
    provider: string
    model: string
    /** Signed difference from previous completed turn; provided only on turn_end. */
    lastTurnDelta: number | null
    compacted: boolean
    /** Last response's input context, matching the Claude-style reading. */
    context: ContextReading & { basis: 'input' }
    contextIncludingOutput: ContextReading & { basis: 'input_output' }
  }
  export type EventPayload<E extends Event> = E extends 'turn_end'
    ? UsageEvent & { success: boolean; cancelled: boolean }
    : E extends 'usage_update' | 'context_compacted' ? UsageEvent : E extends 'ui_resize' ? Viewport : any
  export interface Mint {
    session: { usage(): UsageEvent }
    on<E extends Event>(event: E, handler: (event: EventPayload<E>) => void | { block: string } | Promise<void | { block: string }>): void
    ui: { set(slot: Slot, tree: Node): void; viewport(): Viewport }
    commands: { register(name: string, handler: (value?: string) => unknown, description?: string): void }
    state: { get(key: string): any; set(key: string, value: unknown): void }
    workspace: { read(path: string): string }
    tools: { request(action: string, input?: Record<string, unknown>): string }
    log(message: unknown): void
  }
  export interface TestContext {
    mint: Mint
    emit<E extends Event>(event: E, payload?: Partial<EventPayload<E>>): Promise<any>
    command(name: string, value?: unknown): Promise<any>
    ui(slot: Slot): Node | null
    state(key: string): any
    equal(actual: unknown, expected: unknown, message?: string): void
  }
  export type TestRegistrar = (name: string, run: (context: TestContext) => void | Promise<void>, fixtures?: { usage?: Partial<UsageEvent>; viewport?: Partial<Viewport>; files?: Record<string, string>; state?: Record<string, unknown> }) => void
}
