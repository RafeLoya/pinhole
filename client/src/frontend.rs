use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment, Rect},
    style::{Color, Style, Modifier},
    text::{Span, Line, Text},
    widgets::{Block, Borders, Paragraph, BorderType, Padding, List, ListItem, ListState},
    symbols,
    Terminal,
};
use std::io;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::time::{Duration, Instant};
use local_ip_address::local_ip;
use std::net::UdpSocket;

// Application states
enum AppState {
    MainMenu,
    UserList,
    Connected(String),
    ViewStats,
}

pub enum UserAction {
    Connect(Option<String>),
    ViewStats,
    Quit,
    EndCall,
    None,
}

// Mock data for online users - in a real app, this would come from a server
struct MockUser {
    username: String,
    status: String,
}

// Network information structure
struct NetworkInfo {
    ip_address: String,
    udp_port: u16,
}

impl NetworkInfo {
    fn new() -> Self {
        // Default values
        NetworkInfo {
            ip_address: "Unknown".to_string(),
            udp_port: 0,
        }
    }

    // Get the local IP address and bind to a random UDP port to discover what's available
    fn get_network_info(&mut self) -> Result<(), String> {
        // Get the local IP address
        match local_ip() {
            Ok(ip) => {
                self.ip_address = ip.to_string();

                // Create a UDP socket and bind to a random port (0)
                match UdpSocket::bind("0.0.0.0:0") {
                    Ok(socket) => {
                        match socket.local_addr() {
                            Ok(addr) => {
                                self.udp_port = addr.port();
                                Ok(())
                            },
                            Err(e) => Err(format!("Failed to get local address: {}", e))
                        }
                    },
                    Err(e) => Err(format!("Failed to bind UDP socket: {}", e))
                }
            },
            Err(e) => Err(format!("Failed to get local IP: {}", e))
        }
    }
}

// App state to track UI state
struct App {
    menu_state: ListState,
    users_state: ListState,
    app_state: AppState,
    online_users: Vec<MockUser>,
    last_action: Option<UserAction>,
    selected_username: Option<String>,
    network_info: NetworkInfo,
}

impl App {
    fn new() -> App {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        let mut users_state = ListState::default();
        users_state.select(Some(0));

        // Mock data with only Available or Busy status
        let online_users = vec![
            MockUser { username: "Alice".to_string(), status: "Available".to_string() },
            MockUser { username: "Bob".to_string(), status: "Busy".to_string() },
            MockUser { username: "Charlie".to_string(), status: "Available".to_string() },
            MockUser { username: "David".to_string(), status: "Busy".to_string() },
            MockUser { username: "Eve".to_string(), status: "Available".to_string() },
            MockUser { username: "Back".to_string(), status: "".to_string() }, // Add Back option
        ];

        App {
            menu_state,
            users_state,
            app_state: AppState::MainMenu,
            online_users,
            last_action: None,
            selected_username: None,
            network_info: NetworkInfo::new(),
        }
    }

    fn next_menu_item(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i >= 2 {  // 3 menu items (0-2)
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    fn previous_menu_item(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i == 0 {
                    2  // 3 menu items (0-2)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    fn next_user(&mut self) {
        let i = match self.users_state.selected() {
            Some(i) => {
                if i >= self.online_users.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.users_state.select(Some(i));
    }

    fn previous_user(&mut self) {
        let i = match self.users_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.online_users.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.users_state.select(Some(i));
    }

    fn connect_to_selected_user(&mut self) -> Option<String> {
        if let Some(selected) = self.users_state.selected() {
            if selected < self.online_users.len() {
                let username = self.online_users[selected].username.clone();
                self.app_state = AppState::Connected(username.clone());
                self.selected_username = Some(username.clone());
                return Some(username);
            }
        }
        None
    }

    fn back_to_main_menu(&mut self) {
        self.app_state = AppState::MainMenu;
    }

    fn end_call(&mut self) {
        self.app_state = AppState::MainMenu;
    }

    fn view_stats(&mut self) {
        // Get network information
        let _ = self.network_info.get_network_info();
        self.app_state = AppState::ViewStats;
    }

    fn back_from_stats(&mut self) {
        self.app_state = AppState::MainMenu;
    }
}

// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

// Helper function to get status color
fn status_color(status: &str) -> Color {
    match status {
        "Available" => Color::Green,
        "Busy" => Color::Red,
        _ => Color::Gray,
    }
}

pub fn run_ui() -> Result<UserAction, io::Error> {
    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Main loop
    loop {
        terminal.draw(|f| {
            // Create the base layout
            let size = f.size();

            // Create a background with a border
            let background = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .style(Style::default().bg(Color::Black));

            // Calculate the inner area before rendering (which consumes the block)
            let inner_area = background.inner(size);

            // Now render the background
            f.render_widget(background, size);

            // Main vertical layout - now with status bar at top
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Status bar at top
                    Constraint::Min(10),    // Content area
                ])
                .split(inner_area);

            // Content area changes based on app state
            match &app.app_state {
                AppState::MainMenu => {
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
                        ]))
                        .alignment(Alignment::Left)
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)));

                    f.render_widget(status, chunks[0]);

                    // Menu container
                    let menu_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Blue))
                        .title(Span::styled(" Menu Options ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
                        .padding(Padding::new(2, 2, 1, 1));

                    f.render_widget(&menu_block, chunks[1]);

                    // Menu items as a selectable list
                    let menu_area = menu_block.inner(chunks[1]);

                    let menu_items = vec![
                        ListItem::new(Text::from(vec![
                            Line::from(vec![
                                Span::styled("View Connections", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
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
                },
                AppState::UserList => {
                    // Status bar at top for user list
                    let status = Paragraph::new(
                        Line::from(vec![
                            Span::styled(" ↑↓", Style::default().fg(Color::Yellow)),
                            Span::raw(" to navigate | "),
                            Span::styled("Enter", Style::default().fg(Color::Green)),
                            Span::raw(" to select Back | "),
                            Span::styled("Connections view only", Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC)),
                        ]))
                        .alignment(Alignment::Left)
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)));

                    f.render_widget(status, chunks[0]);

                    // User list container
                    let users_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Blue))
                        .title(Span::styled(" Available Users (View Only) ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
                        .padding(Padding::new(2, 2, 1, 1));

                    f.render_widget(&users_block, chunks[1]);

                    // User list area
                    let users_area = users_block.inner(chunks[1]);

                    // Create user list items
                    let user_items: Vec<ListItem> = app.online_users
                        .iter()
                        .map(|user| {
                            if user.username == "Back" {
                                // Special rendering for the Back option
                                let back_line = Line::from(vec![
                                    Span::styled("< Back to Main Menu >", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                                ]);
                                ListItem::new(Text::from(vec![back_line]))
                            } else {
                                // Normal rendering for users
                                let status_line = Line::from(vec![
                                    Span::styled(&user.username, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                                    Span::raw(" - "),
                                    Span::styled(&user.status, Style::default().fg(status_color(&user.status))),
                                ]);
                                ListItem::new(Text::from(vec![status_line]))
                            }
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
                },
                AppState::Connected(username) => {
                    // Status bar at top for chat
                    let status = Paragraph::new(
                        Line::from(vec![
                            Span::styled(" Status: ", Style::default().fg(Color::White)),
                            Span::styled("Chatting", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                            Span::raw(" | "),
                            Span::styled("Esc", Style::default().fg(Color::Red)),
                            Span::raw(" to end call"),
                        ]))
                        .alignment(Alignment::Left)
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)));

                    f.render_widget(status, chunks[0]);

                    // Chat container
                    let chat_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Green))
                        .title(Span::styled(
                            format!(" Connected with {} ", username),
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        ))
                        .padding(Padding::new(1, 1, 0, 0));

                    f.render_widget(&chat_block, chunks[1]);

                    // Split the chat area into message history and input box
                    let chat_area = chat_block.inner(chunks[1]);
                    let chat_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Min(3),       // Message history
                            Constraint::Length(3),    // Input box
                        ])
                        .split(chat_area);

                    // Message history (placeholder in this demo)
                    let history_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Gray))
                        .title(Span::styled(" Chat History ", Style::default().fg(Color::White)));

                    // Demo message - in reality, this would display actual message history
                    let history_text = Text::from(vec![
                        Line::from(vec![
                            Span::styled("System: ", Style::default().fg(Color::Yellow)),
                            Span::raw("Connected to chat with "),
                            Span::styled(username, Style::default().fg(Color::Cyan)),
                        ]),
                        Line::from(vec![
                            Span::styled("System: ", Style::default().fg(Color::Yellow)),
                            Span::raw("Type your message and press Enter to send"),
                        ]),
                    ]);

                    let history = Paragraph::new(history_text)
                        .block(history_block)
                        .wrap(ratatui::widgets::Wrap { trim: true });

                    f.render_widget(history, chat_chunks[0]);

                    // Input box
                    let input_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Blue))
                        .title(Span::styled(" Input ", Style::default().fg(Color::White)));

                    // Placeholder for text input - in reality, this would be user's input
                    let input = Paragraph::new("Type your message here...")
                        .style(Style::default().fg(Color::Gray))
                        .block(input_block);

                    f.render_widget(input, chat_chunks[1]);
                },
                AppState::ViewStats => {
                    // Status bar at top for stats view
                    let status = Paragraph::new(
                        Line::from(vec![
                            Span::styled(" Status: ", Style::default().fg(Color::White)),
                            Span::styled("Viewing Stats", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            Span::raw(" | "),
                            Span::styled("Esc", Style::default().fg(Color::Yellow)),
                            Span::raw(" to go back"),
                        ]))
                        .alignment(Alignment::Left)
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::DarkGray)));

                    f.render_widget(status, chunks[0]);

                    // Stats container
                    let stats_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Blue))
                        .title(Span::styled(" Network Statistics ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
                        .padding(Padding::new(2, 2, 1, 1));

                    f.render_widget(&stats_block, chunks[1]);

                    // Stats area
                    let stats_area = stats_block.inner(chunks[1]);

                    // Display network information
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
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                            ),
                        ]),
                    ]);

                    let stats = Paragraph::new(stats_text)
                        .block(Block::default())
                        .alignment(Alignment::Left);

                    f.render_widget(stats, stats_area);
                }
            }
        })?;

        // Handle key events
        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match app.app_state {
                    AppState::MainMenu => {
                        match key.code {
                            KeyCode::Up => {
                                app.previous_menu_item();
                            },
                            KeyCode::Down => {
                                app.next_menu_item();
                            },
                            KeyCode::Enter => {
                                if let Some(selected) = app.menu_state.selected() {
                                    match selected {
                                        0 => {
                                            // View Connections
                                            app.app_state = AppState::UserList;
                                            app.last_action = Some(UserAction::Connect(None));
                                        },
                                        1 => {
                                            // View Stats
                                            app.view_stats();
                                            app.last_action = Some(UserAction::ViewStats);
                                        },
                                        2 => {
                                            // Quit Application
                                            app.last_action = Some(UserAction::Quit);
                                            break Ok(UserAction::Quit);
                                        },
                                        _ => {}
                                    }
                                }
                            },
                            _ => {}
                        }
                    },
                    AppState::UserList => {
                        match key.code {
                            KeyCode::Up => {
                                app.previous_user();
                            },
                            KeyCode::Down => {
                                app.next_user();
                            },
                            KeyCode::Enter => {
                                // Check if "Back" option is selected
                                if let Some(selected) = app.users_state.selected() {
                                    if selected == app.online_users.len() - 1 {
                                        // Back option selected - return to previous page
                                        app.back_to_main_menu();
                                    }
                                    // Do nothing for other selections (users)
                                }
                            },
                            KeyCode::Esc => {
                                app.back_to_main_menu();
                            },
                            _ => {}
                        }
                    },
                    AppState::Connected(_) => {
                        match key.code {
                            KeyCode::Esc => {
                                app.end_call();
                                app.last_action = Some(UserAction::EndCall);
                            },
                            _ => {}
                        }
                    },
                    AppState::ViewStats => {
                        match key.code {
                            KeyCode::Esc => {
                                app.back_from_stats();
                            },
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check if we should return an action
        if let Some(action) = &app.last_action {
            match action {
                UserAction::Quit => {
                    // Cleanup
                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    // Return the action
                    return Ok(UserAction::Quit);
                },
                UserAction::Connect(Some(username)) => {
                    // User selected someone to connect with
                    // In a real app, this would trigger the connection
                    // For now, keep showing the chat screen

                    // If we want to return to main menu after this function completes:
                    if matches!(app.app_state, AppState::MainMenu) {
                        // Cleanup
                        disable_raw_mode()?;
                        execute!(
                            terminal.backend_mut(),
                            LeaveAlternateScreen,
                            DisableMouseCapture
                        )?;
                        terminal.show_cursor()?;

                        return Ok(UserAction::Connect(Some(username.clone())));
                    }
                },
                _ => {} // Other actions don't trigger UI exits
            }
        }
    }
}