import type { BuiltinPluginDefinition } from '../constants/plugins'

/**
 * Native plugin names whose enable state is ALSO tracked by a
 * `plugin<Name>Enabled` boolean (the OAuth-backed ones). Mirrors
 * `native_plugin_enable_flag` in
 * `crates/mint-core/src/integrations/plugins.rs`. Plugins not listed here are
 * gated purely by `allowedNativePlugins`.
 */
const FLAG_BY_NATIVE_NAME: Record<string, string> = {
  gmail: 'pluginGmailEnabled',
  google_calendar: 'pluginCalendarEnabled',
  notion: 'pluginNotionEnabled',
  spotify: 'pluginSpotifyEnabled',
  github: 'pluginGithubEnabled',
}

function allowlist(config: any): string[] {
  const value = config?.allowedNativePlugins
  return Array.isArray(value)
    ? value.filter((x: unknown): x is string => typeof x === 'string')
    : []
}

/**
 * Whether the agent's `run_plugin` gate would accept this plugin —
 * `allowedNativePlugins` membership (or `*`) OR its flag boolean. Mirrors
 * `mint_core::native_plugin_enabled`.
 */
export function isNativePluginEnabled(config: any, def: BuiltinPluginDefinition): boolean {
  if (!def.nativeName) return Boolean(config?.[def.enabledField])
  const list = allowlist(config)
  if (list.includes('*') || list.includes(def.nativeName)) return true
  const flag = FLAG_BY_NATIVE_NAME[def.nativeName]
  return flag ? Boolean(config?.[flag]) : false
}

/**
 * The next config for toggling `def` — writes both the flag boolean (when the
 * plugin has one) and `allowedNativePlugins` membership, leaving a `*` wildcard
 * intact. Mirrors `mint_core::set_native_plugin_enabled_in`. Non-native entries
 * (Discord RPC) only get the display boolean.
 */
export function toggleNativePlugin(
  config: any,
  def: BuiltinPluginDefinition,
  enabled: boolean,
): Record<string, any> {
  const next: Record<string, any> = { ...config }
  if (!def.nativeName) {
    next[def.enabledField] = enabled
    return next
  }
  const flag = FLAG_BY_NATIVE_NAME[def.nativeName]
  if (flag) next[flag] = enabled

  const list = allowlist(config)
  if (list.includes('*')) {
    next.allowedNativePlugins = list
    return next
  }
  next.allowedNativePlugins = enabled
    ? list.includes(def.nativeName)
      ? list
      : [...list, def.nativeName]
    : list.filter((n) => n !== def.nativeName)
  return next
}

/**
 * Push a native-plugin toggle through `updateField` (the host persists). At most
 * two writes: the flag boolean (OAuth plugins) and `allowedNativePlugins`.
 */
export function applyNativePluginToggle(
  config: any,
  def: BuiltinPluginDefinition,
  enabled: boolean,
  updateField: (field: string, value: any) => void,
): void {
  if (!def.nativeName) {
    updateField(def.enabledField, enabled)
    return
  }
  const next = toggleNativePlugin(config, def, enabled)
  const flag = FLAG_BY_NATIVE_NAME[def.nativeName]
  if (flag) updateField(flag, next[flag])
  updateField('allowedNativePlugins', next.allowedNativePlugins)
}
