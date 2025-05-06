mod app;
mod network;
mod ui;

use ui::run_ui;
use app::UserAction;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let action = run_ui()?;

    match action {
        UserAction::ViewUsers => {
            println!("Viewed online users.");
            // No action needed; handled in UI.
        },
        UserAction::ViewStats => {
            println!("Viewed stats.");
            // No action needed; handled in UI.
        },
        UserAction::Quit => {
            println!("Exiting...");
        },
        UserAction::None => {
            // Nothing to do
        },
        // These are no longer needed but kept in case they're still part of UserAction:
        _ => {}
    }

    Ok(())
}
