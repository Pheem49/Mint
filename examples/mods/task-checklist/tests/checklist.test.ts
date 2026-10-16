import type { TestRegistrar } from '@mint/mods'

export function registerTests(test: TestRegistrar) {
  test('adds a trimmed task and redraws the count', async ({ command, state, ui, equal }) => {
    await command('add', '  Review changes  ')
    equal(state('tasks'), ['Review changes'])
    equal(ui('side_panel')?.children?.[0].text, '1 tasks')
  })
  test('clear removes tasks from state and the table', async ({ command, state, ui, equal }) => {
    await command('add', 'Review changes')
    await command('clear')
    equal(state('tasks'), [])
    equal(ui('side_panel')?.children?.[1].rows, [])
  })
}
