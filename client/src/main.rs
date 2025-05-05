mod frontend;

use frontend::{run_ui, UserAction};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let action = run_ui()?;

    match action {
        UserAction::Connect(Some(username)) => {
            println!("Connecting to {}...", username);
            // In a real application, you would handle the connection to the user here
        },
        UserAction::Connect(None) => {
            println!("No user selected for connection");
        },
        UserAction::ViewStats => println!("Stats..."),
        UserAction::Quit => println!("Exiting..."),
        UserAction::None => {}
        _ => {}
    }

    Ok(())
}
