mod main_menu;
mod stats;
mod user_list;
mod utils;

use crossterm::{
    execute,
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType},
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io::{self, stdout, Write};
use std::time::Duration;
use crate::app::{App, AppState, UserAction};

pub fn run_ui() -> Result<UserAction, io::Error> {
    // ✅ Step 1: Prompt for username before starting the TUI
    println!("Enter your username (leave blank for 'Anonymous'):");
    let mut username_input = String::new();
    io::stdin().read_line(&mut username_input)?;
    let username = username_input.trim();
    let final_username = if username.is_empty() {
        "Anonymous".to_string()
    } else {
        username.to_string()
    };
    while event::poll(Duration::from_millis(0))? {
        let _ = event::read();
    }

    // ✅ Step 2: Setup TUI
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
    execute!(stdout, Clear(ClearType::All))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = execute!(
                std::io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                Show
            );
        }
    }
    let _cleanup = Cleanup;

    // ✅ Step 3: Create app with the username
    let mut app = App::new();
    app.username = Some(final_username);  // set username here
    app.app_state = AppState::MainMenu;   // no EnterUsername state needed anymore

    // ✅ Step 4: Main TUI loop
    loop {
        terminal.draw(|f| {
            let size = f.size();

            let background = ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray))
                .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black));

            let inner_area = background.inner(size);
            f.render_widget(background, size);

            match &app.app_state {
                AppState::MainMenu => {
                    main_menu::render_main_menu(f, &mut app, inner_area);
                }
                AppState::UserList => {
                    user_list::render_user_list(f, &mut app, inner_area);
                }
                AppState::ViewStats => {
                    stats::render_stats(f, &mut app, inner_area);
                }
            }
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match app.app_state {
                    AppState::MainMenu => match key.code {
                        KeyCode::Up => app.previous_menu_item(),
                        KeyCode::Down => app.next_menu_item(),
                        KeyCode::Enter => {
                            if let Some(selected) = app.menu_state.selected() {
                                match selected {
                                    0 => {
                                        app.app_state = AppState::UserList;
                                        app.last_action = Some(UserAction::ViewUsers);
                                    }
                                    1 => {
                                        app.view_stats();
                                        app.last_action = Some(UserAction::ViewStats);
                                    }
                                    2 => {
                                        app.last_action = Some(UserAction::Quit);
                                        return Ok(UserAction::Quit);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    },
                    AppState::UserList => match key.code {
                        KeyCode::Up => app.previous_user(),
                        KeyCode::Down => app.next_user(),
                        KeyCode::Esc => app.back_to_main_menu(),
                        _ => {}
                    },
                    AppState::ViewStats => match key.code {
                        KeyCode::Esc => app.back_from_stats(),
                        _ => {}
                    },
                }
            }
        }
    }
}
