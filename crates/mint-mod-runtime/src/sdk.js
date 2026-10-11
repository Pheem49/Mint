const __handlers = Object.create(null);
const __commands = Object.create(null);
let __phase = '';
let __view = { ui: {}, commands: [], state: {}, logs: [], requests: [] };
const defineMod = fn => fn;
const __events = Object.freeze(['session_start','session_end','turn_start','turn_end','pre_tool_use','post_tool_use','usage_update','context_compacted','ui_resize','tool_result']);
function __readHost(kind) {
  const result = JSON.parse(__host(JSON.stringify({kind})));
  if (result.error) throw Error(result.error);
  return result.value;
}
const mint = Object.freeze({
  session: Object.freeze({ usage() { return __readHost('usage'); } }),
  on(event, handler) {
    if (!__events.includes(event) || typeof handler !== 'function') throw Error('Invalid event');
    (__handlers[event] ||= []).push(handler);
  },
  ui: Object.freeze({ viewport() { return __readHost('viewport'); }, set(slot, tree) {
    if (!['above_composer','side_panel'].includes(slot)) throw Error('Invalid UI slot');
    __view.ui[slot] = JSON.parse(JSON.stringify(tree));
  }}),
  commands: Object.freeze({ register(name, handler, description = '') {
    if (!/^[a-z][a-z0-9-]{0,47}$/.test(name) || typeof handler !== 'function' || __commands[name]) throw Error('Invalid or duplicate command');
    __commands[name] = handler; __view.commands.push({name, description});
  }}),
  state: Object.freeze({ get(key) { return __view.state[key]; }, set(key, value) {
    if (typeof key !== 'string' || ['__proto__','constructor','prototype'].includes(key)) throw Error('Invalid state key');
    __view.state[key] = JSON.parse(JSON.stringify(value));
  }}),
  workspace: Object.freeze({ read(path) {
    const result = JSON.parse(__host(JSON.stringify({kind:'read', path})));
    if (result.error) throw Error(result.error); return result.value;
  }}),
  tools: Object.freeze({ request(action, input = {}) {
    if (__phase === 'pre_tool_use') throw Error('Tools cannot be requested from pre-tool hooks');
    const result = JSON.parse(__host(JSON.stringify({kind:'tool', action, input})));
    if (result.error) throw Error(result.error);
    return result.value;
  }}),
  log(message) { __view.logs.push(String(message).slice(0,4000)); __view.logs = __view.logs.slice(-100); }
});
async function __dispatch(kind, payload) {
  __phase = kind;
  try {
    if (kind === 'action') {
      const handler = __commands[payload.action];
      if (!handler) throw Error('Unknown action or command');
      return {value: await handler(payload.value)};
    }
    for (const handler of __handlers[kind] || []) {
      const result = await handler(payload);
      if (kind === 'pre_tool_use' && result?.block) return {block: String(result.block)};
    }
    return {};
  } finally { __phase = ''; }
}
