use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::app::AppState;

pub fn render_candidate_pane(f: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .candidates
        .iter()
        .map(|t| {
            ListItem::new(format!("{} ({})", t.surface, t.lemma))
                .style(Style::default().fg(Color::White))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.candidate_cursor));
    let list = List::new(items)
        .block(
            Block::default()
                .title(" Candidates ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(list, area, &mut list_state);
}
