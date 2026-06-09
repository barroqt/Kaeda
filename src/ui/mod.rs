pub mod candidate_pane;
pub mod definition_pane;
pub mod menu;
pub mod source_pane;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::AppState;

pub fn build_layout(area: Rect) -> (Rect, Rect, Rect, Rect, Rect) {
    let width = area.width;

    // Wider terminals give more space to source (45-60%) and definition (55-70%) panes
    let w = (width as f64).min(220.0) / 220.0;
    let source_pct = (45.0 + w * 15.0) as u32;
    let def_pct = (55.0 + w * 15.0) as u32;

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);
    let bottom = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(2)])
        .split(main[1]);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(source_pct, 100),
            Constraint::Ratio(100 - source_pct, 100),
        ])
        .split(main[0]);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(100 - def_pct, 100),
            Constraint::Ratio(def_pct, 100),
        ])
        .split(vertical[1]);
    (
        vertical[0],
        horizontal[0],
        horizontal[1],
        bottom[0],
        bottom[1],
    )
}

pub fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let total = state.subtitles.len();
    let current = state.subtitle_cursor.saturating_add(1);
    let text = Line::from(vec![
        Span::styled(&state.source_file, Style::default().fg(Color::Cyan)),
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

pub fn render_help_bar(f: &mut Frame, area: Rect) {
    let text = Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(" candidate  "),
        Span::styled("←→", Style::default().fg(Color::Cyan)),
        Span::raw(" subtitle  "),
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::raw(" pane  "),
        Span::styled("a", Style::default().fg(Color::Green)),
        Span::raw(" add  "),
        Span::styled("k", Style::default().fg(Color::Green)),
        Span::raw(" known  "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(" skip  "),
        Span::styled("q", Style::default().fg(Color::Red)),
        Span::raw(" quit"),
    ]);
    let bar = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_areas_fill_total_width() {
        let total = Rect::new(0, 0, 100, 100);
        let (source, _candidates, _definition, _status, _help) = build_layout(total);
        assert_eq!(source.width, total.width);
    }
}
