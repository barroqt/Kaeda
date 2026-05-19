pub mod candidate_pane;
pub mod definition_pane;
pub mod source_pane;

use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn build_layout(area: Rect) -> (Rect, Rect, Rect) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(vertical[1]);
    (vertical[0], horizontal[0], horizontal[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_areas_fill_total_width() {
        let total = Rect::new(0, 0, 100, 100);
        let (source, _candidates, _definition) = build_layout(total);
        assert_eq!(source.width, total.width);
    }
}
