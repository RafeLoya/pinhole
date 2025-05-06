use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout,  Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use std::io::Stdout;
use ratatui::widgets::Padding;
use crate::app::App;

pub fn render_stats(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
) {
    // Split area into status + stats content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Status bar
            Constraint::Min(10),    // Stats area
        ])
        .split(area);

    // Status bar at top
    let status = Paragraph::new(
        Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::White)),
            Span::styled("Viewing Stats", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(" to go back"),
        ])
    )
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
        );

    f.render_widget(status, chunks[0]);

    // Stats container
    let stats_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Blue))
        .title(Span::styled(
            " Network Statistics ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(2, 2, 1, 1));

    f.render_widget(&stats_block, chunks[1]);

    // Stats area (inner)
    let stats_area = stats_block.inner(chunks[1]);

    // Display network info
    let stats_text = Text::from(vec![
        Line::from(vec![
            Span::styled("Local IP Address: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&app.network_info.ip_address, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Available UDP Port: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(app.network_info.udp_port.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Connection String: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}:{}", app.network_info.ip_address, app.network_info.udp_port),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
    ]);

    let stats = Paragraph::new(stats_text)
        .block(Block::default())
        .alignment(Alignment::Left);

    f.render_widget(stats, stats_area);
}

