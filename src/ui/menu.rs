use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::MenuState;

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

pub fn render_menu(f: &mut Frame, area: Rect, state: &MenuState) {
    let menu_area = centered_rect(40, 40, area);

    let items: Vec<ListItem> = state
        .options
        .iter()
        .map(|opt| ListItem::new(*opt))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Lantern ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected));

    f.render_stateful_widget(list, menu_area, &mut list_state);

    let hint = Paragraph::new("↑↓ navigate   Enter select   q quit")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, Rect::new(menu_area.x, menu_area.bottom(), menu_area.width, 1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn menu_renders_without_panic() {
        let menu = MenuState::new();
        let backend = TestBackend::new(40, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_menu(f, f.area(), &menu))
            .unwrap();
    }
}
