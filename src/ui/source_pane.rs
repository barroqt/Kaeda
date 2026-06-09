use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::AppState;

pub fn render_source_pane(f: &mut Frame, area: Rect, state: &AppState) {
    let candidates_len = state.candidates.len();
    let selected_surface = if state.candidate_cursor < candidates_len {
        Some(state.candidates[state.candidate_cursor].surface.as_str())
    } else {
        None
    };

    let start = state.subtitle_cursor.saturating_sub(2);
    let end = (state.subtitle_cursor + 3).min(state.subtitles.len());
    let mut lines: Vec<Line> = Vec::new();
    for i in start..end {
        let sub = &state.subtitles[i];
        let is_current = i == state.subtitle_cursor;
        let prefix = if is_current { "▸ " } else { "  " };
        let styled_prefix = Span::styled(prefix, Style::default().fg(Color::Cyan));
        if is_current {
            let mut spans = vec![styled_prefix];
            let text = &sub.text;
            if let Some(surface) = selected_surface {
                push_highlighted_text(&mut spans, text, surface);
            } else {
                spans.push(Span::raw(text.clone()));
            }
            lines.push(
                Line::from(spans).style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        } else {
            lines.push(
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::raw(&sub.text),
                ])
                .style(Style::default().fg(Color::DarkGray)),
            );
        }
    }

    let block = Block::default()
        .title(" Context ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn push_highlighted_text<'a>(spans: &mut Vec<Span<'a>>, text: &str, highlight: &str) {
    let mut remaining = text;
    while let Some(pos) = remaining.find(highlight) {
        if pos > 0 {
            spans.push(Span::raw(remaining[..pos].to_string()));
        }
        spans.push(Span::styled(
            highlight.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        remaining = &remaining[pos + highlight.len()..];
    }
    if !remaining.is_empty() {
        spans.push(Span::raw(remaining.to_string()));
    }
}
