pub mod main_menu;
pub mod user_list;
pub mod stats;

mod utils;

use crate::app::{App, AppState, UserAction};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub fn run_ui() -> Result<UserAction, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ✅ Cleanup guard to restore terminal always
    struct CleanupGuard;
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = execute!(
                std::io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture
            );
            let _ = Terminal::new(CrosstermBackend::new(std::io::stdout()))
                .and_then(|mut term| term.show_cursor());
        }
    }
    let _cleanup_guard = CleanupGuard;

    // Create app state
    let mut app = App::new();

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

            // Route to correct UI module based on app state
            match &app.app_state {
                AppState::MainMenu => {
                    main_menu::render_main_menu(f, &mut app, inner_area);
                },
                AppState::UserList => {
                    user_list::render_user_list(f, &mut app, inner_area);
                },
                AppState::ViewStats => {
                    stats::render_stats(f, &mut app, inner_area);
                }
            }
        })?;

        // Handle key events
        if event::poll(std::time::Duration::from_millis(200))? {
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
                                        return Ok(UserAction::Quit);  // 👈 Clean exit
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
