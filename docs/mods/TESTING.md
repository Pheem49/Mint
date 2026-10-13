# Testing Mint mods

Keep tests beside the mod in `.mint-mods/<id>/tests/*.test.ts` or `*.test.js`.
Existing mods in `mint-mods/<id>/` use the same `tests/` layout.
Export `registerTests(test)`; each named case runs in a fresh QuickJS instance
using the actual mod source and SDK. Tests never enable a mod or change its
saved state. Type imports from `@mint/mods` work as in main.ts.

```ts
import type { TestRegistrar } from '@mint/mods'

export function registerTests(test: TestRegistrar) {
  test('adds an item', async ({ command, state, equal }) => {
    await command('add', 'Review changes')
    equal(state('task'), 'Review changes')
  })

  test('usage updates the band', async ({ emit, ui, equal }) => {
    await emit('usage_update', {
      context: { tokens: 134400, window: 200000, percent: 67.2,
        basis: 'input', lastTurnDelta: null },
    })
    equal(ui('above_composer')?.type, 'stack')
  }, {
    usage: { context: { tokens: 36100, window: 200000, percent: 18.05,
      basis: 'input', lastTurnDelta: null } },
    viewport: { surface: 'terminal', columns: 120, rows: 35 },
  })
}
```

Choose assertions that match your mod; the examples assume commands/state/UI
that the author implements. `emit` dispatches a supported lifecycle event and
returns its result (including pre-tool block decisions). For usage_update,
context_compacted and turn_end it first updates what session.usage returns.
`command` invokes a registered mod command. `ui(slot)` and `state(key)` return
copies, or null when absent. `equal` checks deep JSON equality and accepts an
optional failure message.

The third test argument supplies initial `usage`, `viewport`, `state` and
`files` fixtures. File fixtures map relative paths to text and require the
manifest's workspace.read permission. There is no access to real files,
network, model calls or tools; simulate tool_result events when needed.
Include all usage fields your mod reads. Defaults explicitly mark usage and
viewport as unavailable.

Run `mint mods test <id>`, `/mods test <id>`, or Run tests in the Desktop/Web
Mods panel. CLI returns a nonzero exit status if any case fails. Results name
each case and file and report assertion/runtime errors. Compilation or missing
tests are errors rather than successful empty runs. Tests use current workspace
source, independently of its active revision. Keep tests in the repository;
the compact .mintmod export format contains only the entry, manifest and README.

Limits: 32 test files, 64 uniquely named cases per file, 512 KiB per source,
64 MiB memory per instance and 250 ms CPU per registration/case. Host fixtures
perform no external I/O. Infinite loops and invalid UI fail the case.
