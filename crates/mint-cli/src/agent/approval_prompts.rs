use super::*;

/// Holds raw mode on for [`wait_for_escape_interrupt`]'s continuous-hold
/// window (see that function's docs). A plain `bool` local isn't enough:
/// `tokio::select!` can *drop* that whole async fn mid-poll — the instant
/// its sibling branch (`agent_loop`) resolves first — which runs none of
/// its ordinary code, only `Drop` impls of locals still alive at that
/// point. Without an RAII guard, that's a terminal stuck in raw mode: every
/// plain `println!` after the `select!` (the verification line, the badge,
/// eventually the next prompt) would print without `\n` → `\r\n`
/// translation until something else happened to re-enable cooked mode.
pub(super) struct RawModeGuard(bool);

impl RawModeGuard {
    fn set(&mut self, want_raw: bool) {
        if want_raw == self.0 {
            return;
        }
        if want_raw {
            let _ = crossterm::terminal::enable_raw_mode();
        } else {
            let _ = crossterm::terminal::disable_raw_mode();
        }
        self.0 = want_raw;
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.0 {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}

/// Watches stdin for the rest of the turn: Esc still interrupts (its
/// original job), and — when `LiveStatus::queue_enabled` — every other
/// keypress edits the follow-up draft shown in the box `render_live_status`
/// pins under the live region. Enter queues the draft (see
/// `LiveStatus::queued`) rather than sending it; the caller dispatches
/// queued entries once this turn returns.
///
/// While the queueing box is live and accepting input, this holds raw mode
/// *continuously* via [`RawModeGuard`] instead of toggling it every tick —
/// an earlier version enabled raw mode only for the brief poll/read each
/// tick and disabled it again before sleeping, on the theory that a longer
/// sleep interval or a queue-draining read loop would keep the cooked-mode
/// gap small enough not to matter. Measured against real typing (including
/// tmux-delivered bursts, which land in a single pty write almost
/// instantly), that measurement was wrong: since the cooked window was the
/// *sleep* and the raw window was a near-instant poll, a keystroke was
/// always far more likely to land during cooked mode than raw, regardless
/// of how short the tick was made — every character got locally echoed by
/// the tty driver on top of whatever `render_live_status` had drawn (e.g.
/// "ก่ำ" appearing once as raw echoed text with the terminal's own cursor,
/// once correctly inside the box). Holding raw mode continuously inverts
/// that ratio: cooked mode now only happens for the brief, event-driven
/// windows where something is actually about to print — `approve_cb`,
/// `on_chunk`, and `insert_permanent_lines`'s fallback branch each disable
/// raw mode themselves, synchronously, the instant they're about to print,
/// rather than waiting for this function to notice on its next tick.
pub(super) async fn wait_for_escape_interrupt(
    approval_active: Arc<AtomicBool>,
    live_status: Arc<Mutex<LiveStatus>>,
) {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers};

    // Fixed for the whole turn (set once when it starts), so it's safe to
    // snapshot instead of re-locking every tick.
    let queueing = live_status
        .lock()
        .map(|status| status.queue_enabled)
        .unwrap_or(false);

    let mut raw_mode = RawModeGuard(false);

    loop {
        let blocked =
            approval_active.load(Ordering::Relaxed) || CURSOR_QUERY_ACTIVE.load(Ordering::Relaxed);
        let accepting = !blocked
            && queueing
            && live_status
                .lock()
                .map(|status| status.accepting_input)
                .unwrap_or(false);

        raw_mode.set(accepting);

        if blocked {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }

        if !accepting {
            // Esc-only path: the queueing box isn't live for this turn (or
            // isn't accepting input right now), so there's nothing to type
            // into — fall back to a brief per-tick raw-mode window just for
            // the Esc read, same as before the box existed.
            let _ = crossterm::terminal::enable_raw_mode();
            let escaped = matches!(event::poll(Duration::from_millis(0)), Ok(true))
                && matches!(
                    event::read(),
                    Ok(Event::Key(key_event))
                        if key_event.kind == event::KeyEventKind::Press
                            && key_event.code == KeyCode::Esc
                );
            let _ = crossterm::terminal::disable_raw_mode();
            if escaped {
                break;
            }
            tokio::time::sleep(Duration::from_millis(80)).await;
            continue;
        }

        // `accepting`: `raw_mode` above already holds raw mode, so this can
        // poll/read without any toggling of its own. Drains everything
        // already queued (not just one event) so a fast burst can't leave
        // a backlog for a stray tick boundary to mishandle.
        let mut key_events = Vec::new();
        while matches!(event::poll(Duration::from_millis(0)), Ok(true)) {
            match event::read() {
                Ok(Event::Key(key_event)) if key_event.kind == event::KeyEventKind::Press => {
                    key_events.push(key_event);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        if !key_events.is_empty() {
            let mut escaped = false;
            let mut changed = false;
            if let Ok(mut status) = live_status.lock() {
                for key_event in key_events {
                    if key_event.code == KeyCode::Esc {
                        escaped = true;
                        break;
                    }
                    match key_event.code {
                        KeyCode::Char(c)
                            if !key_event
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            if status.draft.len() < 4000 {
                                status.draft.push(c);
                            }
                            changed = true;
                        }
                        KeyCode::Backspace => {
                            status.draft.pop();
                            changed = true;
                        }
                        KeyCode::Enter => {
                            if !status.draft.is_empty() {
                                let text: String = status.draft.drain(..).collect();
                                status.queued.push(text);
                            }
                            changed = true;
                        }
                        _ => {}
                    }
                }
                if changed {
                    render_live_status(&mut status);
                }
            }
            if escaped {
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(15)).await;
    }
}

/// Interactive picker for `ask_user` options: ↑/↓ + Enter to choose, digits for a quick
/// jump, any other character drops into free-text mode, Esc declines. Falls back to a
/// plain numbered prompt when raw mode isn't available (e.g. piped/non-interactive stdin).
/// Arrow-key Yes/No picker for the plan-mode approval cards (entering and
/// exiting), built on top of [`run_option_picker`] so plan mode gets the same
/// picker UX as every other approval card (`WriteFile`, `ApplyPatch`,
/// `AskUser` with options, …) instead of a plain `y/N` text prompt. Selecting
/// `yes_label`/`no_label` maps back to `Approved`/`Denied`; typing free text
/// or pressing Esc still falls through to `Intercepted`/`Denied` untouched,
/// since `run_option_picker` already supports those directly.
pub(super) fn plan_mode_option_picker(
    yes_label: &str,
    no_label: &str,
) -> Result<ApprovalOutcome, String> {
    let options = vec![yes_label.to_string(), no_label.to_string()];
    match run_option_picker(&options)? {
        ApprovalOutcome::Intercepted(text) if text == yes_label => Ok(ApprovalOutcome::Approved),
        ApprovalOutcome::Intercepted(text) if text == no_label => Ok(ApprovalOutcome::Denied),
        other => Ok(other),
    }
}

pub(super) fn run_option_picker(options: &[String]) -> Result<ApprovalOutcome, String> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    if enable_raw_mode().is_err() {
        return ask_numbered_fallback(options);
    }

    println!(
        "  {DIM}\u{2191}/\u{2193} + Enter to choose  \u{00b7}  type to answer freely  \u{00b7}  Esc to decline{RESET}"
    );
    let mut selected = 0usize;
    render_options(options, selected, true);

    let result = loop {
        let event = match event::read() {
            Ok(ev) => ev,
            Err(e) => break Err(e.to_string()),
        };
        let Event::Key(key) = event else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Up => {
                selected = selected.saturating_sub(1);
                render_options(options, selected, false);
            }
            KeyCode::Down => {
                selected = (selected + 1).min(options.len().saturating_sub(1));
                render_options(options, selected, false);
            }
            KeyCode::Enter => break Ok(ApprovalOutcome::Intercepted(options[selected].clone())),
            KeyCode::Esc => break Ok(ApprovalOutcome::Denied),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = disable_raw_mode();
                println!();
                std::process::exit(130);
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(idx) = (c as usize)
                    .checked_sub('1' as usize)
                    .filter(|idx| *idx < options.len())
                {
                    break Ok(ApprovalOutcome::Intercepted(options[idx].clone()));
                }
            }
            KeyCode::Char(c) => {
                let _ = disable_raw_mode();
                print!("\n  Answer: {c}");
                let _ = io::stdout().flush();
                let mut rest = String::new();
                let _ = std::io::stdin().read_line(&mut rest);
                let full = format!("{c}{}", rest.trim_end_matches(['\n', '\r']));
                let trimmed = full.trim();
                return Ok(if trimmed.is_empty() {
                    ApprovalOutcome::Denied
                } else {
                    ApprovalOutcome::Intercepted(trimmed.to_owned())
                });
            }
            _ => {}
        }
    };

    let _ = disable_raw_mode();
    println!();
    result
}

pub(super) fn render_options(options: &[String], selected: usize, first: bool) {
    use crossterm::terminal::{Clear, ClearType};
    use crossterm::{cursor, execute};

    let mut out = io::stdout();
    if !first {
        let _ = execute!(out, cursor::MoveUp(options.len() as u16));
    }
    for (i, opt) in options.iter().enumerate() {
        let _ = execute!(out, cursor::MoveToColumn(0), Clear(ClearType::CurrentLine));
        if i == selected {
            println!("  {CYAN}\u{276f}{RESET} {CYAN}{}) {}{RESET}", i + 1, opt);
        } else {
            println!("    {DIM}{}) {}{RESET}", i + 1, opt);
        }
    }
    let _ = out.flush();
}

pub(super) fn ask_numbered_fallback(options: &[String]) -> Result<ApprovalOutcome, String> {
    for (i, opt) in options.iter().enumerate() {
        println!("    {}) {}", i + 1, opt);
    }
    print!(
        "  Answer (type a number 1-{}, your own text, or leave empty to decline): ",
        options.len()
    );
    let _ = io::stdout().flush();
    let mut answer = String::new();
    match std::io::stdin().read_line(&mut answer) {
        Ok(_) => {
            let trimmed = answer.trim();
            if trimmed.is_empty() {
                Ok(ApprovalOutcome::Denied)
            } else if let Some(choice) = trimmed
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|idx| options.get(idx))
            {
                Ok(ApprovalOutcome::Intercepted(choice.clone()))
            } else {
                Ok(ApprovalOutcome::Intercepted(trimmed.to_owned()))
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn print_approval_card(title: &str, fields: &[(&str, &str)]) {
    let top_bar = format!("  {DIM}{}┬{}{RESET}", "─".repeat(11), "─".repeat(56));
    let bot_bar = format!("  {DIM}{}┴{}{RESET}", "─".repeat(11), "─".repeat(56));

    println!();
    println!(
        "  {BRIGHT}APPROVAL REQUIRED{RESET} {DIM}•{RESET} {BLUE}{}{RESET}",
        title
    );
    println!("{}", top_bar);
    for (label, val) in fields {
        let val_lines: Vec<&str> = val.lines().collect();
        if val_lines.is_empty() {
            println!("  {BRIGHT}{:<10}{RESET} {DIM}│{RESET}", label);
        } else {
            for (idx, line) in val_lines.iter().enumerate() {
                if idx == 0 {
                    println!("  {BRIGHT}{:<10}{RESET} {DIM}│{RESET} {}", label, line);
                } else {
                    println!("             {DIM}│{RESET} {}", line);
                }
            }
        }
    }
    println!("{}", bot_bar);
}

/// Approval prompt for the persistable `AgentApproval` variants (`run_shell`,
/// `write_file`, `apply_patch`, `note_write`, `run_plugin`, `mcp_tool`).
///
/// Checks `permission_rules` (in-memory rules from this run, seeded from
/// disk) first — if `tool`/`subject` already has a saved decision, returns
/// immediately without prompting or calling `render_card`. Otherwise calls
/// `render_card` (each approval type prints its own card format — a diff for
/// file edits, a plain field list for everything else) and shows a 3-option
/// prompt (Yes / Yes-and-don't-ask-again-this-session / No); the middle
/// choice appends the new rule to `permission_rules` for the rest of this
/// run only — deliberately *not* persisted via `save_config`, so it doesn't
/// outlive the process. Rules already on disk from before this change (or
/// added via `mint safety permissions`) are still honored by the lookup
/// above; this prompt just no longer creates new durable ones.
pub(super) fn confirm_with_persistence(
    tool: &str,
    subject: &str,
    root: &Path,
    permission_rules: &mut Vec<PermissionRule>,
    approval_active: &AtomicBool,
    render_card: impl FnOnce(),
) -> Result<ApprovalOutcome, String> {
    if let Some(decision) = permission_decision_for(permission_rules, tool, subject, root) {
        return Ok(match decision {
            PermissionDecision::Allow => {
                println!(
                    "  {DIM}Auto-approved by saved permission rule ({tool}: {subject}){RESET}"
                );
                ApprovalOutcome::Approved
            }
            PermissionDecision::Deny => {
                println!("  {DIM}Auto-denied by saved permission rule ({tool}: {subject}){RESET}");
                ApprovalOutcome::Denied
            }
        });
    }

    render_card();
    approval_active.store(true, Ordering::Relaxed);
    let choice = prompt_persistent_approval(subject);
    approval_active.store(false, Ordering::Relaxed);

    match choice {
        0 => Ok(ApprovalOutcome::Approved),
        1 => {
            let rule = PermissionRule {
                tool: tool.to_string(),
                pattern: subject.to_string(),
                decision: PermissionDecision::Allow,
                project_root: None,
            };
            permission_rules.push(rule);
            Ok(ApprovalOutcome::Approved)
        }
        _ => Ok(ApprovalOutcome::Denied),
    }
}

/// Renders the 3-option approval choice and returns its index (0 = Yes,
/// 1 = Yes and don't ask again this session, 2 = No). Falls back to a plain
/// stdin line read (matching `crate::confirm`'s non-TTY behavior, e.g.
/// piped/CI invocations) when not running in a real terminal, rather than
/// silently defaulting to deny like the underlying `prompt_interactive_select`
/// does on its own.
pub(super) fn prompt_persistent_approval(subject: &str) -> usize {
    use crossterm::tty::IsTty;

    // Kept short so the option still fits on one line in the picker.
    let truncated: String = if subject.chars().count() > 60 {
        subject.chars().take(60).collect::<String>() + "…"
    } else {
        subject.to_string()
    };

    let options = [
        "Yes".to_string(),
        format!("Yes, and don't ask again for: {truncated}"),
        "No".to_string(),
    ];

    if !io::stdout().is_tty() {
        print!("  Approve? [y]es / [d]on't ask again this session / [N]o: ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_err() {
            return 2;
        }
        return match answer.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => 0,
            "d" | "dont" | "don't" => 1,
            _ => 2,
        };
    }

    match crate::interactive::prompt_interactive_select(
        "Approve this action?",
        &options,
        &options[0],
    ) {
        Ok(Some(choice)) => options.iter().position(|o| *o == choice).unwrap_or(2),
        _ => 2,
    }
}

pub(super) fn confirm_pausing_interrupt(prompt: &str, approval_active: &AtomicBool) -> bool {
    approval_active.store(true, Ordering::Relaxed);
    let approved = crate::confirm_security(prompt).unwrap_or(false);
    approval_active.store(false, Ordering::Relaxed);
    approved
}

/// Dim-cyan used for the code block gutter/border — same hue as `CYAN` but
/// at reduced intensity so it doesn't compete with actual code content.
pub(super) const CODE_BORDER: &str = "\x1b[2m\x1b[38;2;56;189;248m";
