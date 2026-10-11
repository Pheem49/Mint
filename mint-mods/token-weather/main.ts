// Token Weather - a live forecast of the context window, drawn above the prompt.
const BLOCKS = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const HISTORY_KEY = 'history';
const MAX_POINTS = 12;

type Weather = { icon: string; word: string; color: string };

function bucket(percent: number): Weather {
  if (percent < 25) return { icon: '☀', word: 'Clear', color: 'yellow' };
  if (percent < 50) return { icon: '☁', word: 'Cloudy', color: 'cyan' };
  if (percent < 75) return { icon: '☂', word: 'Showers', color: 'blue' };
  if (percent < 90) return { icon: '☇', word: 'Storm', color: 'magenta' };
  return { icon: '↯', word: 'Compact soon', color: 'red' };
}

function formatTokens(value: number | null): string {
  if (value == null) return '—';
  if (value < 1000) return String(Math.round(value));
  const k = Math.round((value / 1000) * 10) / 10;
  return (Number.isInteger(k) ? String(k) : k.toFixed(1)) + 'k';
}

function sparkline(points: (number | null)[]): string {
  let out = '';
  for (let i = 0; i < points.length; i++) {
    const p = points[i];
    if (p == null) {
      out += '·';
      continue;
    }
    const level = Math.round((p / 100) * (BLOCKS.length - 1));
    out += BLOCKS[Math.max(0, Math.min(BLOCKS.length - 1, level))];
  }
  return out;
}

function deltaText(delta: number | null | undefined): string {
  if (delta == null) return '— last turn';
  if (delta === 0) return '— no change last turn';
  const up = delta > 0;
  return (up ? '▲ +' : '▼ -') + formatTokens(Math.abs(delta)) + ' last turn';
}

function loadHistory(mint: any): (number | null)[] {
  const raw = mint.state.get(HISTORY_KEY);
  return Array.isArray(raw) ? raw.slice(-MAX_POINTS) : [];
}

export function register(mint: any) {
  let history: (number | null)[] = loadHistory(mint);

  const draw = (usage: any) => {
    const ctx = usage && usage.context ? usage.context : {};
    const percent = typeof ctx.percent === 'number' ? ctx.percent : null;
    const tokens = typeof ctx.tokens === 'number' ? ctx.tokens : null;
    const windowTokens = typeof ctx.window === 'number' ? ctx.window : null;
    const spark = sparkline(history);
    const children: any[] = [];

    if (percent != null && tokens != null) {
      const shown = Math.round(percent);
      const w = bucket(shown);
      children.push({ type: 'text', text: w.icon + ' ' + w.word, color: w.color });
      children.push({ type: 'text', text: shown + '%' });
      children.push({ type: 'text', text: formatTokens(tokens) + ' / ' + formatTokens(windowTokens) });
      if (spark) children.push({ type: 'text', text: spark });
      children.push({ type: 'text', text: deltaText(ctx.lastTurnDelta) });
    } else {
      children.push({ type: 'text', text: '☁ No reading', color: 'default' });
      if (windowTokens != null) {
        children.push({ type: 'text', text: '— / ' + formatTokens(windowTokens) });
      }
      if (spark) children.push({ type: 'text', text: spark });
    }

    mint.ui.set('above_composer', { type: 'stack', direction: 'row', children });
  };

  // Initialize immediately so a newly published mod shows something before the next turn.
  draw(mint.session.usage());

  mint.on('session_start', () => {
    history = loadHistory(mint);
    draw(mint.session.usage());
  });

  mint.on('turn_start', () => draw(mint.session.usage()));
  mint.on('usage_update', (event: any) => draw(event));

  mint.on('context_compacted', (event: any) => {
    // Compaction resets occupancy; start the sparkline from a fresh baseline.
    history = [];
    mint.state.set(HISTORY_KEY, history);
    draw(event);
  });

  mint.on('turn_end', (event: any) => {
    const ctx = event && event.context ? event.context : {};
    const point = typeof ctx.percent === 'number' ? ctx.percent : null;
    history = history.concat([point]).slice(-MAX_POINTS);
    mint.state.set(HISTORY_KEY, history);
    draw(event);
  });
}
