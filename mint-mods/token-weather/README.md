# Token Weather

A live forecast of your context window, drawn on one line in the band above the prompt.

## What it shows

```
☂ Showers  67%  134.4k / 200k  ▁▂▃▅▆▇█  ▲ +98.3k last turn
```

| Segment | Meaning |
| --- | --- |
| `☀ Clear` (yellow) | under 25% of the planning window used |
| `☁ Cloudy` (cyan) | 25–49% |
| `☂ Showers` (blue) | 50–74% |
| `☇ Storm` (magenta) | 75–89% |
| `↯ Compact soon` (red) | 90% and up |
| `67%` | share of the planning window used |
| `134.4k / 200k` | tokens used out of the window |
| `▁▂▃▅▆▇█` | last 12 turns, one block per turn (context fill) |
| `▲ +98.3k last turn` | how much the last turn added (`▼` when it shrank) |

## How it updates

- Redraws on `usage_update`, `turn_start`, `turn_end`, `session_start` and `context_compacted`.
- Appends exactly one sparkline point per completed turn (`turn_end`), keeping the last 12 in `mint.state`.
- A turn with no measurement still advances the chart, drawn as `·`.

## Notes

- The window is Mint's planning limit from provider defaults/configuration (`contextWindowSource: mint_config`), not discovered model capacity.
- Occupancy is the latest measured response, not a sum of requests.
- Successful compaction clears the sparkline and shows the fresh baseline until the next measured response.
- Reading is `usage.context` (input over the latest response), including its delta.
- Missing measurements show `☁ No reading` instead of invented numbers.
- No permissions required; no tools or filesystem access.
