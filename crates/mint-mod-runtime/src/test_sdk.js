const __tests = [];
function __test(name, run, fixtures = {}) {
  if (typeof name !== 'string' || !name || name.length > 120 || typeof run !== 'function' || __tests.length >= 64 || __tests.some(t => t.name === name)) throw Error('Invalid or duplicate test');
  __tests.push({name, run, fixtures: JSON.parse(JSON.stringify(fixtures))});
}
function __normal(value) {
  if (value == null) return null;
  if (Array.isArray(value)) return value.map(__normal);
  if (typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(k => [k, __normal(value[k])]));
  return value;
}
async function __runTest(index) {
  function checkSnapshot() {
    const response=JSON.parse(__host(JSON.stringify({kind:'test_snapshot', value:__view})));
    if (response.error) throw Error(response.error);
  }
  const context = Object.freeze({
    mint,
    async emit(kind, payload = {}) {
      if (!__events.includes(kind)) throw Error('Invalid test event');
      if (['usage_update','context_compacted','turn_end'].includes(kind)) __host(JSON.stringify({kind:'test_usage', value:payload}));
      const result=await __dispatch(kind, payload); checkSnapshot(); return result;
    },
    async command(name, value) { const result=await __dispatch('action', {action:name, value}); checkSnapshot(); return result; },
    ui(slot) { return JSON.parse(JSON.stringify(__view.ui[slot] ?? null)); },
    state(key) { return JSON.parse(JSON.stringify(__view.state[key] ?? null)); },
    equal(actual, expected, message = 'Values differ') {
      if (JSON.stringify(__normal(actual)) !== JSON.stringify(__normal(expected))) throw Error(`${message}: expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
    }
  });
  await __tests[index].run(context);
}
