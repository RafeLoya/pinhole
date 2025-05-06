use ratatui::widgets::ListState;
use crate::network::NetworkInfo;

// App states
#[derive(Debug)]
pub enum AppState {
    MainMenu,
    UserList,
    ViewStats,
}

// Actions the user can take
#[derive(Debug)]
pub enum UserAction {
    ViewStats,
    Quit,
    None,
    ViewUsers,
}

// Simple struct representing a user
pub struct MockUser {
    pub username: String,
    pub status: String,
}

// Main app state
pub struct App {
    pub menu_state: ListState,
    pub users_state: ListState,
    pub app_state: AppState,
    pub online_users: Vec<MockUser>,
    pub last_action: Option<UserAction>,
    pub network_info: NetworkInfo,
}

impl App {
    pub fn new() -> App {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        let mut users_state = ListState::default();
        users_state.select(Some(0));

        // Mock users list
        let online_users = vec![
            MockUser { username: "Alice".to_string(), status: "Available".to_string() },
            MockUser { username: "Bob".to_string(), status: "Busy".to_string() },
            MockUser { username: "Charlie".to_string(), status: "Available".to_string() },
            MockUser { username: "David".to_string(), status: "Busy".to_string() },
            MockUser { username: "Eve".to_string(), status: "Available".to_string() },
        ];

        App {
            menu_state,
            users_state,
            app_state: AppState::MainMenu,
            online_users,
            last_action: None,
            network_info: NetworkInfo::new(),
        }
    }

    // Navigate menu (MainMenu)
    pub fn next_menu_item(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => if i >= 2 { 0 } else { i + 1 },
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    pub fn previous_menu_item(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => if i == 0 { 2 } else { i - 1 },
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    // Navigate users (UserList)
    pub fn next_user(&mut self) {
        let i = match self.users_state.selected() {
            Some(i) => if i >= self.online_users.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.users_state.select(Some(i));
    }

    pub fn previous_user(&mut self) {
        let i = match self.users_state.selected() {
            Some(i) => if i == 0 { self.online_users.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.users_state.select(Some(i));
    }

    // Go back to main menu
    pub fn back_to_main_menu(&mut self) {
        self.app_state = AppState::MainMenu;
    }

    // Switch to viewing stats
    pub fn view_stats(&mut self) {
        let _ = self.network_info.get_network_info(); // refresh stats
        self.app_state = AppState::ViewStats;
    }

    // Go back from stats to main
    pub fn back_from_stats(&mut self) {
        self.app_state = AppState::MainMenu;
    }
}
