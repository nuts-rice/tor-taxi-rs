use anyhow::Context;
use ratatui::layout::Rect;
use ratatui::Frame;

use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};
use std::fmt::{Debug, Display, Formatter};

use crate::display::{config::TuiConfig, tui_app::TuiApp};
pub use shared::{Link, LinkStatus};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ColumnType {
    Slug,
    Average,
    Status,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ColumnStatus {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Column {
    pub typ: ColumnType,
    pub status: ColumnStatus,
}
impl Column {
    pub const fn new_shown(typ: ColumnType) -> Self {
        Self {
            typ,
            status: ColumnStatus::Visible,
        }
    }
    pub const fn new_hidden(typ: ColumnType) -> Self {
        Self {
            typ,
            status: ColumnStatus::Hidden,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Columns(Vec<Column>);

/*
fn get_avg_ms(link: &Link) -> Option<f64> {
    let db =
    let slug = link.slug;


}
*/

pub fn render(f: &mut Frame<'_>, app: TuiApp, rect: Rect) {
    let config = &app.config;
}

fn render_table_row(app: TuiApp, config: &TuiConfig) -> Row<'static> {
    todo!()
}

fn render_status_cell(link: &Link) -> Cell<'static> {
    let status = link.status;
    match status {
        Some(LinkStatus::Orange) => Cell::from("🟠"),
        Some(LinkStatus::Red) => Cell::from("🔴"),
        Some(LinkStatus::White) => Cell::from("⚪️"),
        None => Cell::from("Not probed yet"),
    }
}

fn render_avg_ms(link: &Link) -> Cell<'static> {
    todo!()
}

fn render_slug_cell(link: &Link) -> Cell<'static> {
    let slug = &link.slug;
    Cell::from(format!("{}", slug))
}
