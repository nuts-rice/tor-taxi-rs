use anyhow::Context;
use ratatui::layout::Rect;
use ratatui::Frame;

use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};

use crate::display::tui_app::TuiApp;
pub use shared::{Link, LinkStatus};

pub const READ_AVG_MS_UP_AND_TOTAL_SQL: &str = "SELECT slug,
       avg(latency_ms)   AS avg_latency_ms,
       count(latency_ms) AS up_samples,
       count(*)          AS total_samples
FROM probes
WHERE checked_at >= ?1
GROUP BY slug
";

pub const READ_AVG_MS_SQL: &str = "
SELECT slug, avg(latency_ms) AS avg_latency_ms FROM probes GROUP BY slug
";


pub enum ColumnType {
    Slug,
    Average,
    Status,
}

pub enum ColumnStatus {
    Visible,
    Hidden,
}

fn get_avg_ms( ) -> Option<f64> {
    todo!()

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
