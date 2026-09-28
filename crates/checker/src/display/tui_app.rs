use ratatui::widgets::TableState;

pub struct TuiApp {
    pub link_table_state: TableState,
    pub selected_link_idx: usize,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            link_table_state: TableState::default(),
            selected_link_idx: 0,
        }
    }
}
