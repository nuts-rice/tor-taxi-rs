use crate::display::link_data::Columns;
use color_eyre::config::Theme;
use ratatui::style::palette::tailwind;
use ratatui::style::{self, Color, Modifier, Style, Stylize};
use std::time::Duration;

pub const PALETTES: [tailwind::Palette; 4] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
];
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

pub struct TableColors {
    pub buffer_bg: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub row_fg: Color,
    pub selected_row_style_fg: Color,
    pub selected_column_style_fg: Color,
    pub selected_cell_style_fg: Color,
    pub normal_row_color: Color,
    pub footer_border_color: Color,
}

impl TableColors {
    pub const fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_style_fg: color.c400,
            selected_column_style_fg: color.c400,
            selected_cell_style_fg: color.c600,
            normal_row_color: tailwind::SLATE.c950,
            footer_border_color: color.c400,
        }
    }
}
