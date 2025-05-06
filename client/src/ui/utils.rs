use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
};

// Helper: Create a centered rectangle (for popups, etc.)
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Helper: Get color based on user status
pub fn status_color(status: &str) -> Color {
    match status {
        "Available" => Color::Green,
        "Busy" => Color::Red,
        _ => Color::Gray,
    }
}
