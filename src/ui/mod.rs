pub mod candidate_pane;
pub mod definition_pane;
pub mod source_pane;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

pub fn build_layout(area: Rect) -> (Rect, Rect, Rect, Rect) {
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main[0]);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(vertical[1]);
    (vertical[0], horizontal[0], horizontal[1], main[1])
}

pub fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let total = state.subtitles.len();
    let current = state.subtitle_cursor.saturating_add(1);
    let text = Line::from(vec![
        Span::styled(
            &state.source_file,
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled(
            format!("[{}/{}]", current, total),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            format!("deck: {}", state.deck_count),
            Style::default().fg(Color::Green),
        ),
    ]);
    let bar = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_areas_fill_total_width() {
        let total = Rect::new(0, 0, 100, 100);
        let (source, _candidates, _definition, _status) = build_layout(total);
        assert_eq!(source.width, total.width);
    }
}
