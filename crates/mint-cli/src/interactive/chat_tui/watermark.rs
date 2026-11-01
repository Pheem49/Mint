//! 3D rotating medallion watermark for the empty TUI transcript.
use super::*;

pub(super) const WATERMARK_FRAME_COUNT: u8 = 200;
pub(super) const WATERMARK_TICK: std::time::Duration = std::time::Duration::from_millis(30);
pub(super) const WATERMARK_HOLD: std::time::Duration = std::time::Duration::from_secs(1);
pub(super) const WATERMARK_WIDTH: u16 = 39;
pub(super) const WATERMARK_HEIGHT: u16 = 17;

pub(super) fn watermark_animation_at(
    elapsed: std::time::Duration,
) -> (Option<u8>, Option<std::time::Duration>) {
    let half_turn_frames = WATERMARK_FRAME_COUNT / 2;
    let half_turn = WATERMARK_TICK * half_turn_frames as u32;
    let hold_end = half_turn + WATERMARK_HOLD;
    let full_turn_end = hold_end + half_turn;
    // Ease each 3-second half turn into and out of its resting face.
    let eased_frame = |step: u8| {
        let progress = step as f32 / half_turn_frames as f32;
        let eased = (1.0 - (progress * std::f32::consts::PI).cos()) * 0.5;
        (eased * half_turn_frames as f32).round() as u8
    };
    if elapsed < half_turn {
        let step = (elapsed.as_millis() / WATERMARK_TICK.as_millis()) as u8;
        (
            Some(eased_frame(step)),
            Some(WATERMARK_TICK * (step as u32 + 1)),
        )
    } else if elapsed < hold_end {
        (Some(half_turn_frames), Some(hold_end))
    } else if elapsed < full_turn_end {
        let step = ((elapsed - hold_end).as_millis() / WATERMARK_TICK.as_millis()) as u8;
        (
            Some((half_turn_frames + eased_frame(step)) % WATERMARK_FRAME_COUNT),
            Some(hold_end + WATERMARK_TICK * (step as u32 + 1)),
        )
    } else {
        (None, None)
    }
}

pub(super) fn mint_m_watermark(frame: Option<u8>, height: usize) -> Text<'static> {
    const WIDTH: usize = WATERMARK_WIDTH as usize;
    const DESIGN_HEIGHT: usize = WATERMARK_HEIGHT as usize;
    #[derive(Clone, Copy)]
    enum Part {
        Void,
        Rim,
        Band,
        Orbit,
        Mark,
    }

    let height = height.clamp(1, DESIGN_HEIGHT);
    let angle = frame.unwrap_or(0) as f32 * std::f32::consts::TAU / WATERMARK_FRAME_COUNT as f32;
    let (sin, cos) = angle.sin_cos();
    let mut pixels = vec![vec![None::<(f32, Part)>; WIDTH]; height];

    // A dotted medallion with an M on its front and a leaf on its back. The
    // two faces and the rim share one depth buffer as they turn in 3D.
    for y in 0..DESIGN_HEIGHT {
        let world_y = y as f32 - 8.0;
        let projected_y = y * height.saturating_sub(1) / (DESIGN_HEIGHT - 1);
        for source_x in 0..WIDTH {
            let world_x = (source_x as f32 - 19.0) / 2.0;
            let radius = world_x.hypot(world_y);
            if radius > 8.15 {
                continue;
            }

            let stem = (world_x.abs() - 3.7).abs() < 0.5 && world_y.abs() <= 4.2;
            let diagonal_y = world_y + 4.2;
            let diagonal = (-4.2..=2.2).contains(&world_y)
                && [-3.7 + diagonal_y * 3.7 / 6.4, 3.7 - diagonal_y * 3.7 / 6.4]
                    .into_iter()
                    .any(|line_x| (world_x - line_x).abs() < 0.5);
            let front_mark = stem || diagonal;

            let leaf_x = world_x * 0.8 + world_y * 0.6;
            let leaf_y = -world_x * 0.6 + world_y * 0.8;
            let leaf_radius = (leaf_x / 4.2).powi(2) + (leaf_y / 2.5).powi(2);
            let back_mark =
                (leaf_radius - 1.0).abs() < 0.22 || (leaf_y.abs() < 0.32 && leaf_x.abs() < 3.8);

            for (z, mark) in [(-1.2, back_mark), (0.0, false), (1.2, front_mark)] {
                let part = if radius > 7.25 {
                    Part::Rim
                } else if radius > 6.2 {
                    Part::Band
                } else if mark {
                    Part::Mark
                } else if radius > 5.75 && radius < 6.05 && (source_x + y) % 3 != 0 {
                    Part::Orbit
                } else {
                    Part::Void
                };
                let projected_x = (19.0 + (world_x * cos + z * sin) * 2.0).round() as isize;
                if !(0..WIDTH as isize).contains(&projected_x) {
                    continue;
                }
                let depth = -world_x * sin + z * cos;
                let cell = &mut pixels[projected_y][projected_x as usize];
                if cell.is_none_or(|(existing_depth, _)| depth > existing_depth) {
                    *cell = Some((depth, part));
                }
            }
        }
    }

    let lines = pixels
        .into_iter()
        .map(|row| {
            let spans = row
                .into_iter()
                .map(|pixel| match pixel {
                    Some((_, Part::Void)) | None => Span::raw(" "),
                    Some((depth, part)) => {
                        let lit = frame.is_some() && depth > 0.5;
                        let (glyph, color) = match part {
                            Part::Mark => (
                                "⣿",
                                if lit {
                                    crate::terminal_theme::ACCENT
                                } else {
                                    crate::terminal_theme::WATERMARK_MARK
                                },
                            ),
                            Part::Rim => (
                                "⠿",
                                if lit {
                                    crate::terminal_theme::BLUE
                                } else {
                                    crate::terminal_theme::WATERMARK_RIM
                                },
                            ),
                            Part::Band => ("⠿", crate::terminal_theme::WATERMARK_BAND),
                            Part::Orbit => ("⠂", crate::terminal_theme::WATERMARK_ORBIT),
                            Part::Void => unreachable!(),
                        };
                        Span::styled(glyph, Style::default().fg(color))
                    }
                })
                .collect::<Vec<_>>();
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}
