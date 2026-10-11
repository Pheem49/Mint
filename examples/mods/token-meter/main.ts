export function register(mint: any) {
  const render = (value: number | null) => mint.ui.set('above_composer', {
    type: 'text', text: value == null ? 'Usage unavailable' : `${value.toLocaleString()} tokens this turn`
  });
  render(null);
  mint.on('turn_start', () => render(null));
  mint.on('usage_update', (event: any) => render(event.totalTokens));
}
