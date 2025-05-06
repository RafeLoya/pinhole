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
use crate::ui::utils::status_color;

pub fn render_user_list(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
) {
    // Split area into status + content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Status bar
            Constraint::Min(10),    // User list area
        ])
        .split(area);

    // Status bar at top
    let status = Paragraph::new(
        Line::from(vec![
            Span::styled(" ↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" to navigate | "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(" to return to Main Menu"),
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

    // User list container
    let users_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Blue))
        .title(Span::styled(
            " Online Users ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(2, 2, 1, 1));

    f.render_widget(&users_block, chunks[1]);

    // User list area
    let users_area = users_block.inner(chunks[1]);

    // Build the list of user items (no Back option)
    let user_items: Vec<ListItem> = app.online_users
        .iter()
        .map(|user| {
            let status_line = Line::from(vec![
                Span::styled(&user.username, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw(" - "),
                Span::styled(&user.status, Style::default().fg(status_color(&user.status))),
            ]);
            ListItem::new(Text::from(vec![status_line]))
        })
        .collect();

    let users_list = List::new(user_items)
        .block(Block::default())
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" > ");

    f.render_stateful_widget(users_list, users_area, &mut app.users_state);
}
