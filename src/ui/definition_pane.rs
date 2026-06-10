use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use kaeda_core::dictionary::db::DictEntry;

pub fn render_definition_pane(
    f: &mut Frame,
    area: Rect,
    lemma: Option<&str>,
    entry: Option<&DictEntry>,
) {
    let block = Block::default()
        .title(" Definition ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = match (lemma, entry) {
        (Some(_), None) => Paragraph::new(Line::from(Span::styled(
            "Loading...",
            Style::default().fg(Color::Yellow),
        )))
        .block(block),
        (_, Some(e)) => {
            let mut lines = vec![
                Line::from(vec![
                    Span::raw("Meaning: "),
                    Span::styled(&e.meaning, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::raw("POS: "),
                    Span::styled(&e.pos, Style::default().fg(Color::Cyan)),
                ]),
            ];
            if let Some(example) = e.examples.first() {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::raw("Ex: "),
                    Span::styled(example.clone(), Style::default().fg(Color::Yellow)),
                ]));
            }
            Paragraph::new(lines).block(block)
        }
        _ => Paragraph::new(Line::from(Span::styled(
            "No definition found",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block),
    };

    f.render_widget(paragraph, area);
}
