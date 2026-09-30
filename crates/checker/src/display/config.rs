use crate::display::link_data::Columns;
use color_eyre::config::Theme;
use std::time::Duration;
#[derive(Debug)]
pub struct TuiConfig {
    pub refresh_rate: Duration,
    pub theme: Theme,
    pub tui_columns: Columns,
}

impl TuiConfig {
    pub fn new(refresh_rate: Duration, theme: Theme, tui_columns: Columns) -> Self {
        Self {
            refresh_rate,
            theme,
            tui_columns,
        }
    }
}
