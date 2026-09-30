use anyhow::Context;
use ratatui::Frame;

use ratatui::layout::{Constraint, Rect};
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

impl Columns {
    pub fn columns(&self) -> impl Iterator<Item = &Column> {
        self.0
            .iter()
            .filter(|c| matches!(c.status, ColumnStatus::Visible))
    }

    pub fn all_columns(&self) -> impl Iterator<Item = &Column> {
        self.0.iter()
    }
}
/*
fn get_avg_ms(link: &Link) -> Option<f64> {
    let db =
    let slug = link.slug;


}
*/

pub fn render(f: &mut Frame<'_>, app: TuiApp, rect: Rect) {
    //let config = &app.config;
    todo!()
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
    match link.avg_latency_ms {
        Some(ms) => Cell::from(format!("{ms} ms")),
        None => Cell::from("-"),
    }
}

fn render_slug_cell(link: &Link) -> Cell<'static> {
    let slug = &link.slug;
    Cell::from(format!("{}", slug))
}

fn constraint_len_calculator(items: &[Link]) -> (u16, u16, u16) {
    /*
        let slug_len = items
            .iter()
            .map(Link::slug)
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
        let avg_ms_len = items
            .iter()
            .map(Data::address)
            .flat_map(str::lines)
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
        let status_len = items
            .iter()
            .map(Data::email)
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
    */
    todo!()
}
