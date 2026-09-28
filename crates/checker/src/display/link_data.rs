use anyhow::Context;
use ratatui::layout::Rect;
use ratatui::Frame;

use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};

use crate::display::tui_app::TuiApp;
pub use shared::{Link, LinkStatus};

pub enum ColumnType {
    Slug,
    Average,
    Status,
}

pub enum ColumnStatus {
    Visible,
    Hidden,
}

pub fn render(f: &mut Frame<'_>, app: TuiApp, rect: Rect) {}

fn render_status_cell(link: &Link) -> Cell<'static> {
    let status = link.status;
    match status {
        Some(LinkStatus::Orange) => Cell::from("🟠"),
        Some(LinkStatus::Red) => Cell::from("🔴"),
        Some(LinkStatus::White) => Cell::from("⚪️"),
        None => Cell::from("Not probed yet"),
    }
}
