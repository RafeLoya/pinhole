use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};
use ratatui::widgets::Padding;
use crate::app::App;

pub fn render_main_menu(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
) {
    // Split into status + content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Status bar
            Constraint::Min(10),    // Content area
        ])
        .split(area);

    // Status bar at top
    let status = Paragraph::new(
        Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::White)),
            Span::styled("Ready", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" to navigate | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" to select"),
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

    // Menu container
    let menu_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Blue))
        .title(Span::styled(
            " Menu Options ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        ))
        .padding(Padding::new(2, 2, 1, 1));

    f.render_widget(&menu_block, chunks[1]);

    // Menu items as a selectable list
    let menu_area = menu_block.inner(chunks[1]);

    let menu_items = vec![
        ListItem::new(Text::from(vec![
            Line::from(vec![
                Span::styled("View Online Users", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
        ])),
        ListItem::new(Text::from(vec![
            Line::from(vec![
                Span::styled("View Stats", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
        ])),
        ListItem::new(Text::from(vec![
            Line::from(vec![
                Span::styled("Quit Application", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            ]),
        ])),
    ];

    let menu_list = List::new(menu_items)
        .block(Block::default())
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" > ");

    f.render_stateful_widget(menu_list, menu_area, &mut app.menu_state);
}
