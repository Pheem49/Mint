//! 2D mouse/transcript selection, auto-scroll, composer hit-testing, and clipboard copy.
use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ScreenPoint {
    pub(super) row: u16,
    pub(super) column: u16,
}

impl ScreenPoint {
    pub(super) fn from_mouse(mouse: event::MouseEvent) -> Self {
        Self {
            row: mouse.row,
            column: mouse.column,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct MouseSelection {
    pub(super) anchor: Option<ScreenPoint>,
    pub(super) head: Option<ScreenPoint>,
    pub(super) dragging: bool,
    pub(super) had_drag: bool,
}

impl MouseSelection {
    pub(super) fn start(&mut self, point: ScreenPoint) {
        self.anchor = Some(point);
        self.head = Some(point);
        self.dragging = true;
        self.had_drag = false;
    }

    pub(super) fn update(&mut self, point: ScreenPoint) {
        if self.dragging {
            self.head = Some(point);
            self.had_drag = true;
        }
    }

    pub(super) fn finish(&mut self, point: ScreenPoint) -> bool {
        if !self.dragging {
            return false;
        }
        self.head = Some(point);
        self.dragging = false;
        if self.had_drag {
            true
        } else {
            self.clear();
            false
        }
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn range(&self) -> Option<(ScreenPoint, ScreenPoint)> {
        let anchor = self.anchor?;
        let head = self.head?;
        Some(if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        })
    }

    pub(super) fn endpoints(&self) -> Option<(ScreenPoint, ScreenPoint)> {
        Some((self.anchor?, self.head?))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct TranscriptPoint {
    pub(super) row: usize,
    pub(super) column: u16,
}

pub(super) struct TranscriptSelection {
    pub(super) layout: TranscriptLayout,
    pub(super) area: Rect,
    pub(super) offset: usize,
    pub(super) pointer: ScreenPoint,
    pub(super) anchor: TranscriptPoint,
    pub(super) head: TranscriptPoint,
    pub(super) dragging: bool,
    pub(super) had_drag: bool,
}

impl TranscriptSelection {
    pub(super) fn from_text(
        text: Text<'static>,
        area: Rect,
        offset: usize,
        pointer: ScreenPoint,
    ) -> Self {
        let layout = TranscriptLayout::new(text, area.width);
        let anchor = TranscriptPoint {
            row: offset
                .saturating_add(pointer.row.saturating_sub(area.y) as usize)
                .min(layout.height.saturating_sub(1)),
            column: pointer.column.saturating_sub(area.x),
        };
        Self {
            layout,
            area,
            offset,
            pointer,
            anchor,
            head: anchor,
            dragging: true,
            had_drag: false,
        }
    }
    pub(super) fn update(&mut self, pointer: ScreenPoint) {
        if !self.dragging {
            return;
        }
        self.pointer = pointer;
        self.had_drag = true;
        self.head = TranscriptPoint {
            row: self
                .offset
                .saturating_add(
                    (pointer.row.clamp(self.area.y, self.area.bottom() - 1) - self.area.y) as usize,
                )
                .min(self.layout.height.saturating_sub(1)),
            column: pointer.column.clamp(self.area.x, self.area.right() - 1) - self.area.x,
        };
    }
    pub(super) fn range(&self) -> (TranscriptPoint, TranscriptPoint) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
    pub(super) fn scroll(&mut self, scroll: &mut usize) -> bool {
        if !self.dragging || !self.had_drag {
            return false;
        }
        let max = self.layout.height.saturating_sub(self.area.height as usize);
        let old = self.offset;
        if self.pointer.row <= self.area.y {
            self.offset = self.offset.saturating_sub(1);
        } else if self.pointer.row >= self.area.bottom().saturating_sub(1) {
            self.offset = self.offset.saturating_add(1).min(max);
        }
        if old == self.offset {
            return false;
        }
        *scroll = max.saturating_sub(self.offset);
        self.update(self.pointer);
        true
    }
    pub(super) fn paint(&self, buffer: &mut Buffer, back_button: Option<Rect>) {
        use ratatui::widgets::Widget;
        let mut snapshot = Buffer::empty(self.area);
        self.layout
            .paragraph_at(self.offset)
            .render(self.area, &mut snapshot);
        let (start, end) = self.range();
        for y in self.area.y..self.area.bottom() {
            let row = self.offset + (y - self.area.y) as usize;
            for x in self.area.x..self.area.right() {
                let position = Position::new(x, y);
                if back_button.is_some_and(|area| area.contains(position))
                    || (self.layout.height > self.area.height as usize
                        && x == self.area.right() - 1)
                {
                    continue;
                }
                if let (Some(target), Some(source)) =
                    (buffer.cell_mut(position), snapshot.cell(position))
                {
                    *target = source.clone();
                    let column = x - self.area.x;
                    if row >= start.row
                        && row <= end.row
                        && (row != start.row || column >= start.column)
                        && (row != end.row || column <= end.column)
                    {
                        target.set_style(
                            Style::default()
                                .bg(crate::terminal_theme::BLUE)
                                .fg(crate::terminal_theme::TEXT),
                        );
                    }
                }
            }
        }
    }
    pub(super) fn text(&self) -> String {
        use ratatui::widgets::Widget;
        let (start, end) = self.range();
        let mut output = String::new();
        let mut row = start.row;
        while row <= end.row {
            let height = (end.row - row + 1).min(64) as u16;
            let area = Rect::new(0, 0, self.layout.width, height);
            let mut buffer = Buffer::empty(area);
            self.layout.paragraph_at(row).render(area, &mut buffer);
            let selection = MouseSelection {
                anchor: Some(ScreenPoint {
                    row: 0,
                    column: if row == start.row { start.column } else { 0 },
                }),
                head: Some(ScreenPoint {
                    row: height - 1,
                    column: if row + height as usize - 1 == end.row {
                        end.column
                    } else {
                        self.layout.width - 1
                    },
                }),
                dragging: false,
                had_drag: true,
            };
            if !output.is_empty() || row > start.row {
                output.push('\n');
            }
            output.push_str(&selected_screen_text(&buffer, selection));
            if output.len() > COPY_LIMIT {
                break;
            }
            row += height as usize;
        }
        output
    }
}

pub(super) fn input_index_at(
    point: ScreenPoint,
    area: Rect,
    chars: &[char],
    row_width: usize,
    cursor_pos: usize,
    end_after_character: bool,
) -> Option<usize> {
    let first_text_row = area.y + 1;
    let visible_row = point.row.checked_sub(first_text_row)? as usize;
    let visible_rows = ((area.height as usize).saturating_sub(2)).min(6);
    if visible_row >= visible_rows {
        return None;
    }

    let (rows, cursor_row, _) = wrap_input_visual_into_rows(chars, row_width, cursor_pos);
    let scroll = cursor_row.saturating_add(1).saturating_sub(visible_rows);
    let row_index = visible_row + scroll;
    let row = rows.get(row_index)?;
    let start = input_row_start(chars, &rows, row_index);

    let text_x = area.x.saturating_add(3);
    let target_column = point.column.saturating_sub(text_x) as usize;
    let row_chars: Vec<char> = row.chars().collect();
    let mut visual_column = 0usize;
    for (index, character) in row_chars.iter().enumerate() {
        let width = UnicodeWidthStr::width(character.to_string().as_str()).max(1);
        if target_column < visual_column + width {
            return Some(start + index + usize::from(end_after_character));
        }
        visual_column += width;
    }
    Some(start + row_chars.len())
}

#[derive(Default)]
pub(super) struct TextClipboard {
    pub(super) native: Option<Box<dyn NativeClipboard>>,
    pub(super) initialization_attempted: bool,
}

pub(super) trait NativeClipboard {
    fn set_text(&mut self, text: String) -> std::result::Result<(), String>;
}

impl NativeClipboard for arboard::Clipboard {
    fn set_text(&mut self, text: String) -> std::result::Result<(), String> {
        arboard::Clipboard::set_text(self, text).map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CopyMethod {
    Native,
    Osc52,
}

impl TextClipboard {
    pub(super) fn copy<W: Write>(&mut self, text: &str, writer: &mut W) -> io::Result<CopyMethod> {
        if !self.initialization_attempted {
            self.initialization_attempted = true;
            self.native = arboard::Clipboard::new()
                .ok()
                .map(|clipboard| Box::new(clipboard) as Box<dyn NativeClipboard>);
        }
        if let Some(clipboard) = self.native.as_mut()
            && clipboard.set_text(text.to_owned()).is_ok()
        {
            return Ok(CopyMethod::Native);
        }

        write!(writer, "\x1b]52;c;{}\x07", BASE64.encode(text.as_bytes()))?;
        writer.flush()?;
        Ok(CopyMethod::Osc52)
    }
}

pub(super) fn apply_mouse_selection(buffer: &mut Buffer, selection: MouseSelection) {
    for selected in selected_rows(buffer.area, selection) {
        for column in selected.left..=selected.right {
            if let Some(cell) = buffer.cell_mut(Position::new(column, selected.row)) {
                cell.modifier.insert(Modifier::REVERSED);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectedRow {
    pub(super) row: u16,
    left: u16,
    right: u16,
}

pub(super) fn selected_rows(area: Rect, selection: MouseSelection) -> Vec<SelectedRow> {
    let Some((start, end)) = selection.range() else {
        return Vec::new();
    };
    if area.is_empty() {
        return Vec::new();
    }
    let top = start.row.max(area.top());
    let bottom = end.row.min(area.bottom().saturating_sub(1));
    if top > bottom {
        return Vec::new();
    }

    (top..=bottom)
        .filter_map(|row| {
            let left = if row == start.row {
                start.column
            } else {
                area.left()
            }
            .max(area.left());
            let right = if row == end.row {
                end.column
            } else {
                area.right().saturating_sub(1)
            }
            .min(area.right().saturating_sub(1));
            (left <= right).then_some(SelectedRow { row, left, right })
        })
        .collect()
}

pub(super) fn selected_screen_text(buffer: &Buffer, selection: MouseSelection) -> String {
    let mut lines = Vec::new();
    for selected in selected_rows(buffer.area, selection) {
        let mut line = String::new();
        let mut column = buffer.area.left();
        while column < buffer.area.right() {
            let Some(cell) = buffer.cell(Position::new(column, selected.row)) else {
                break;
            };
            let symbol = cell.symbol();
            let width = UnicodeWidthStr::width(symbol).max(1) as u16;
            let symbol_right = column.saturating_add(width.saturating_sub(1));
            if symbol_right >= selected.left && column <= selected.right {
                line.push_str(symbol);
            }
            column = column.saturating_add(width);
        }
        lines.push(line.trim_end_matches(' ').to_owned());
    }
    lines.join("\n")
}

pub(super) fn selected_cells_match(
    previous: &Buffer,
    current: &Buffer,
    selection: MouseSelection,
) -> bool {
    if previous.area != current.area {
        return false;
    }
    let rows = selected_rows(current.area, selection);
    if selection.range().is_some() && rows.is_empty() {
        return false;
    }
    for selected in rows {
        for column in selected.left..=selected.right {
            let position = Position::new(column, selected.row);
            if previous.cell(position).map(|cell| cell.symbol())
                != current.cell(position).map(|cell| cell.symbol())
            {
                return false;
            }
        }
    }
    true
}
