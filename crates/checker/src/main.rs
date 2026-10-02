//! Out-of-band onion link prober.
//!
//! Runs on a host with a Tor daemon, probes each link through the SOCKS port,
//! and writes the resulting status into D1. The Cloudflare Worker only ever
//! reads that table — the Workers runtime has no Tor and cannot reach .onion.

pub mod d1;
mod display;
mod links;
mod probe;
mod proxy;
mod status;

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::display::config::{TableColors, TuiConfig, PALETTES};
use crate::display::link_data::{render_table_row, Columns};
use crate::links::LinkSet;
use crate::probe::{Outcome, ProbePolicy, ProbeResult};
use crate::status::StatusPolicy;
use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Row, ScrollbarState, TableState};
use ratatui::{DefaultTerminal, Frame};
use shared::Link;
use tokio::sync::mpsc;

#[derive(Debug, Parser)]
#[command(about = "Probe onion links over Tor and write their status to D1")]
pub struct Args {
    /// Time between sweeps, e.g. `15m`, `90s`, `1h`.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "15m")]
    pub interval: Duration,

    /// The canonical link set.
    #[arg(long, default_value = "crates/checker/links.toml")]
    pub links: PathBuf,

    /// Tor's SOCKS port.
    #[arg(long, env = "TOR_SOCKS_ADDR", default_value = "127.0.0.1:9050")]
    pub socks_addr: String,

    /// Probe once and exit, instead of looping. Useful for a first run.
    #[arg(long)]
    pub once: bool,

    /// Probe and report, but do not write to D1. Needs no credentials, so it is
    /// the way to try a new link before committing it to links.toml.
    #[arg(long)]
    pub dry_run: bool,

    /// Never start the TUI, even on a terminal. It is already off whenever
    /// stdout is not a TTY (systemd, docker without -t, a pipe) and under --once.
    #[arg(long)]
    pub no_tui: bool,

    /// Where logs go while the TUI owns the terminal. Defaults to
    /// `$TMPDIR/tor-taxi-checker.log`.
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    #[command(flatten)]
    pub probe_policy: ProbePolicy,

    #[command(flatten)]
    pub status_policy: StatusPolicy,
}

impl Args {
    /// The TUI needs a terminal to draw on and a loop to watch. Without a TTY
    /// it would write escape codes into journald; under --once there is no
    /// second sweep, and the exit status is the result that matters.
    fn wants_tui(&self) -> bool {
        !self.no_tui && !self.once && std::io::stdout().is_terminal()
    }
}

/// What the sweep task tells the TUI.
enum SweepEvent {
    Started,
    Probed {
        links: LinkSet,
        results: Vec<ProbeResult>,
    },
    Failed(String),
}

/// Per-link history the TUI keeps across sweeps. D1 holds the real record;
/// this is enough to colour a row without reading it back, and it works
/// under --dry-run where there is no D1 at all.
#[derive(Default)]
struct Tally {
    consecutive_failures: u32,
    up_samples: u64,
    up_latency_sum_ms: u64,
}

const LINK_HEIGHT: usize = 4;
struct TuiApp {
    link_data: Vec<Link>,
    config: TuiConfig,
    status_policy: StatusPolicy,
    tallies: HashMap<String, Tally>,
    colors: TableColors,
    footer: String,
    color_index: usize,
    pub table_state: TableState,
    scroll_state: ScrollbarState,
}
impl TuiApp {
    pub fn new(tui_config: TuiConfig, status_policy: StatusPolicy, link_data: Vec<Link>) -> Self {
        let link_data_len = link_data.len();
        Self {
            link_data,
            config: tui_config,
            status_policy,
            tallies: HashMap::new(),
            footer: "waiting for the first sweep…".into(),
            table_state: TableState::default(),
            colors: TableColors::new(&PALETTES[0]),
            color_index: 0,
            scroll_state: ScrollbarState::new(link_data_len.saturating_sub(1) * LINK_HEIGHT),
        }
    }

    pub fn next_row(&mut self) {
        if self.link_data.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.link_data.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * LINK_HEIGHT);
    }
    pub fn previous_row(&mut self) {
        if self.link_data.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.link_data.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * LINK_HEIGHT);
    }

    pub fn next_column(&mut self) {
        self.table_state.select_next_column();
    }

    pub fn previous_column(&mut self) {
        self.table_state.select_previous_column();
    }

    pub const fn next_color(&mut self) {
        self.color_index = (self.color_index + 1) % PALETTES.len();
    }

    pub const fn previous_color(&mut self) {
        let count = PALETTES.len();
        self.color_index = (self.color_index + count - 1) % count;
    }

    pub const fn set_colors(&mut self) {
        self.colors = TableColors::new(&PALETTES[self.color_index]);
    }

    /// Folds one sweep into the tallies and rebuilds the rows from links.toml
    /// order, so a link added mid-run shows up on the next sweep.
    fn apply_sweep(&mut self, links: &LinkSet, results: &[ProbeResult]) {
        let by_slug: HashMap<&str, &ProbeResult> =
            results.iter().map(|r| (r.slug.as_str(), r)).collect();
        let mut up = 0;

        self.link_data = links
            .iter()
            .map(|(slug, entry)| {
                let tally = self.tallies.entry(slug.clone()).or_default();
                let result = by_slug.get(slug.as_str());
                let latency = result.and_then(|r| match r.outcome {
                    Outcome::Reachable { latency } => Some(latency),
                    Outcome::Unreachable { .. } => None,
                });

                match (result, latency) {
                    (Some(_), Some(l)) => {
                        up += 1;
                        tally.consecutive_failures = 0;
                        tally.up_samples += 1;
                        tally.up_latency_sum_ms += l.as_millis() as u64;
                    }
                    (Some(_), None) => tally.consecutive_failures += 1,
                    (None, _) => {}
                }

                Link {
                    slug: slug.clone(),
                    url: entry.url.clone(),
                    category: entry.category,
                    description: entry.description.clone(),
                    status: result.map(|_| {
                        self.status_policy
                            .classify(latency, tally.consecutive_failures)
                    }),
                    latency_ms: latency.map(|l| l.as_millis() as u64),
                    checked_ago_secs: None,
                    avg_latency_ms: tally.up_latency_sum_ms.checked_div(tally.up_samples),
                }
            })
            .collect();

        self.scroll_state = self
            .scroll_state
            .content_length(self.link_data.len().saturating_sub(1) * LINK_HEIGHT);
        if self.table_state.selected().is_none() && !self.link_data.is_empty() {
            self.table_state.select(Some(0));
        }
        self.footer = format!("{up} up / {} down", results.len() - up);
    }

    fn handle(&mut self, event: SweepEvent) {
        match event {
            SweepEvent::Started => self.footer = format!("{} · sweeping…", self.footer),
            SweepEvent::Probed { links, results } => self.apply_sweep(&links, &results),
            SweepEvent::Failed(e) => self.footer = format!("last sweep failed: {e}"),
        }
    }

    fn run(
        mut self,
        terminal: &mut DefaultTerminal,
        mut events: mpsc::UnboundedReceiver<SweepEvent>,
    ) -> Result<()> {
        loop {
            while let Ok(ev) = events.try_recv() {
                self.handle(ev);
            }
            terminal.draw(|frame| self.render(frame))?;

            // Poll rather than block, so a finished sweep redraws without
            // waiting for a keypress.
            if !event::poll(self.config.refresh_rate)? {
                continue;
            }
            if let event::Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    // Raw mode swallows SIGINT; Ctrl-C arrives as a key.
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(())
                    }
                    KeyCode::Down | KeyCode::Char('j') => self.next_row(),
                    KeyCode::Up | KeyCode::Char('k') => self.previous_row(),
                    KeyCode::Right | KeyCode::Char('l') => self.next_column(),
                    KeyCode::Left | KeyCode::Char('h') => self.previous_column(),
                    _ => {}
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]);
        let rects = frame.area().layout_vec(&layout);
        self.render_table(frame, rects[0]);
        self.render_footer(frame, rects[1]);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default().fg(Color::White).bg(Color::Blue);
        let header = Row::new(["slug", "avg", "status"]).style(header_style);
        let bar = " █ ";
        let rows = self
            .link_data
            .iter()
            .map(|link| render_table_row(link, &self.config))
            .collect::<Vec<_>>();
        let table = ratatui::widgets::Table::new(
            rows,
            [
                Constraint::Length(20),
                Constraint::Min(10),
                Constraint::Min(10),
            ],
        )
        .header(header)
        .highlight_symbol(Text::from(vec![
            "".into(),
            bar.into(),
            bar.into(),
            "".into(),
        ]))
        .style(
            Style::new()
                .fg(self.colors.row_fg)
                .bg(self.colors.normal_row_color),
        );
        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let text = format!(" {} · ↑↓ move · q quit", self.footer);
        frame.render_widget(Paragraph::new(text).fg(Color::Gray), area);
    }

    fn selected_link(&self) -> Option<&Link> {
        self.link_data.get(self.table_state.selected()?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let tui = args.wants_tui();
    init_tracing(&args, tui)?;

    let d1 = if args.dry_run {
        None
    } else {
        Some(d1::D1Client::from_env()?)
    };

    match proxy::verify_isolation(&args.socks_addr).await {
        Ok(true) => tracing::info!("circuit isolation confirmed"),
        Ok(false) => tracing::warn!("circuit isolation looks disabled"),
        Err(e) => tracing::warn!(error = ?e, "could not verify circuit isolation"),
    }

    if !tui {
        return sweep_loop(&args, d1.as_ref(), None).await;
    }

    // The sweeps run as a task; the TUI owns the terminal on a blocking
    // thread, because crossterm's event::poll would otherwise stall the
    // runtime the probes need.
    let (tx, rx) = mpsc::unbounded_channel();
    let config = TuiConfig::new(
        Duration::from_millis(250),
        color_eyre::config::Theme::default(),
        Columns::default(),
    );
    let app = TuiApp::new(config, args.status_policy, Vec::new());
    let ui = tokio::task::spawn_blocking(move || ratatui::run(|terminal| app.run(terminal, rx)));

    // Quitting the TUI ends the process; dropping the sweep mid-flight is fine,
    // since an unflushed sweep simply never reaches D1.
    tokio::select! {
        ui = ui => ui?,
        swept = sweep_loop(&args, d1.as_ref(), Some(tx)) => swept,
    }
}

/// Logs to stdout headless. Under the TUI, to a file instead: anything written
/// to the terminal would be drawn over the table.
fn init_tracing(args: &Args, tui: bool) -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "tor_taxi_checker=info".into());
    let builder = tracing_subscriber::fmt().with_env_filter(filter);

    if tui {
        let path = args
            .log_file
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("tor-taxi-checker.log"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        builder
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .init();
        eprintln!("logging to {}", path.display());
    } else {
        builder.init();
    }
    Ok(())
}

async fn sweep_loop(
    args: &Args,
    d1: Option<&d1::D1Client>,
    ui: Option<mpsc::UnboundedSender<SweepEvent>>,
) -> Result<()> {
    // A send only fails once the TUI has gone, and then nobody is listening.
    let notify = |ev| {
        if let Some(ui) = &ui {
            let _ = ui.send(ev);
        }
    };

    loop {
        notify(SweepEvent::Started);
        let swept = sweep(args, d1, |links, results| {
            notify(SweepEvent::Probed {
                links: links.clone(),
                results: results.to_vec(),
            })
        })
        .await;
        if let Err(e) = &swept {
            tracing::error!(error = ?e, "sweep failed");
            notify(SweepEvent::Failed(format!("{e:#}")));
        }

        // Propagated rather than discarded: deploy/pi/entrypoint.sh treats the
        // checker's exit status as the verdict of a --once verification run.
        if args.once {
            return swept;
        }
        tokio::time::sleep(args.interval).await;
    }
}

/// `on_probed` sees the results before the D1 write, so the TUI still shows
/// what the network said when the write itself fails.
async fn sweep(
    args: &Args,
    d1: Option<&d1::D1Client>,
    on_probed: impl FnOnce(&LinkSet, &[ProbeResult]),
) -> Result<()> {
    // Re-read every sweep so adding a link does not need a restart.
    let links = links::load(&args.links)?;
    tracing::info!(count = links.len(), "🔍 starting sweep");
    let now = Instant::now();

    let results = probe::probe_all(&links, &args.socks_addr, &args.probe_policy).await?;

    let mut up = 0;
    for result in &results {
        match &result.outcome {
            Outcome::Reachable { .. } => up += 1,
            Outcome::Unreachable { reason } => {
                tracing::warn!(slug = %result.slug, reason, "link down");
            }
        }
    }
    tracing::info!(up, down = results.len() - up, "sweep complete");
    let sweep_duration = now.elapsed();
    tracing::info!("✅ Sweep complete in {:2} s", sweep_duration.as_secs_f64());
    on_probed(&links, &results);

    match d1 {
        Some(d1) => d1.flush(&links, &results, &args.status_policy).await,

        None => {
            tracing::info!("dry run: skipping the D1 write");
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::d1::{
        build_batch, retire_sql, PROBE_RETENTION_SECS, PRUNE_PROBES_SQL, RECORD_PROBE_SQL,
        RECORD_SQL, RELEASE_URL_SQL, UPSERT_SQL,
    };
    use crate::links::LinkSet;
    use crate::probe::{Outcome, ProbeResult};
    use crate::status::StatusPolicy;
    use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
    use serde_json::Value;
    use shared::{LinkCategory, LinkEntry, SELECT_LINKS_SQL};
    use std::time::Duration;

    /// Every migration, in the order wrangler applies them. Listed rather than
    /// globbed because `include_str!` is a compile-time read: a new migration
    /// has to be added here, which is the moment to ask whether it breaks any
    /// of the statements below.
    const MIGRATIONS: &[&str] = &[
        include_str!("../../worker/migrations/0000_initial.sql"),
        include_str!("../../worker/migrations/0001_add_retired_at.sql"),
        include_str!("../../worker/migrations/0002_add_probes.sql"),
        include_str!("../../worker/migrations/0003_add_avg_to_probes.sql"),
    ];

    const POLICY: StatusPolicy = StatusPolicy {
        orange_after: Duration::from_secs(10),
        red_after: 3,
    };

    /// A database in the shape production is actually in.
    fn migrated() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        for (i, m) in MIGRATIONS.iter().enumerate() {
            conn.execute_batch(m)
                .unwrap_or_else(|e| panic!("migration {i:04} does not apply: {e}"));
        }
        conn
    }

    /// Runs one sweep by replaying what `build_batch` actually produced.
    ///
    /// Deliberately not a hand-written sequence of statements: the ordering is
    /// the thing under test, and a test that restates it would pass against a
    /// `flush` that had been reordered underneath it.
    ///
    /// `toml` is the link set as links.toml spells it; `probe` is what the
    /// network said about each one.
    fn sweep(
        conn: &Connection,
        toml: &[(&str, &str)],
        probe: &[(&str, bool, Option<u64>)],
        now: u64,
    ) {
        let links: LinkSet = toml
            .iter()
            .map(|(slug, url)| {
                (
                    slug.to_string(),
                    LinkEntry {
                        url: url.to_string(),
                        category: LinkCategory::Info,
                        description: "desc".into(),
                        expected_content: None,
                    },
                )
            })
            .collect();

        let results: Vec<ProbeResult> = probe
            .iter()
            .map(|(slug, up, latency)| ProbeResult {
                slug: slug.to_string(),
                outcome: if *up {
                    Outcome::Reachable {
                        latency: Duration::from_millis(latency.expect("an up probe has a latency")),
                    }
                } else {
                    Outcome::Unreachable {
                        reason: "test".into(),
                    }
                },
            })
            .collect();

        for stmt in build_batch(&links, &results, &POLICY, now) {
            let sql = stmt["sql"].as_str().unwrap();
            let bound: Vec<SqlValue> = stmt["params"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| match v {
                    Value::Null => SqlValue::Null,
                    Value::Number(n) => SqlValue::Integer(n.as_i64().unwrap()),
                    Value::String(s) => SqlValue::Text(s.clone()),
                    other => panic!("unexpected bound parameter {other}"),
                })
                .collect();
            conn.execute(sql, params_from_iter(bound))
                .unwrap_or_else(|e| panic!("{sql}\n  failed: {e}"));
        }
    }

    fn status_of(conn: &Connection, slug: &str) -> Option<String> {
        conn.query_row(
            "SELECT status FROM links WHERE slug = ?1",
            params![slug],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn schema_drift_is_guarded() {
        let conn = migrated();
        for (name, sql) in [
            ("RELEASE_URL_SQL", RELEASE_URL_SQL.to_string()),
            ("UPSERT_SQL", UPSERT_SQL.to_string()),
            ("RECORD_SQL", RECORD_SQL.to_string()),
            ("retire_sql", retire_sql(3)),
            ("RECORD_PROBE_SQL", RECORD_PROBE_SQL.to_string()),
            ("PRUNE_PROBES_SQL", PRUNE_PROBES_SQL.to_string()),
            // The Worker's read. It only compiles to wasm32 and cannot be
            // tested in place, so this is the one thing standing between a
            // malformed SELECT and a blank directory.
            ("SELECT_LINKS_SQL", SELECT_LINKS_SQL.to_string()),
        ] {
            conn.prepare(&sql)
                .unwrap_or_else(|e| panic!("{name} does not match the schema: {e}"));
        }
    }

    #[test]
    fn schema_rejects_an_unknown_category() {
        let conn = migrated();
        let err = conn.execute(
            UPSERT_SQL,
            params!["new", "http://x.onion/", "NewCategory", ""],
        );
        assert!(
            err.is_err(),
            "the CHECK constraint accepted a category no LinkCategory variant spells"
        );
    }

    /// The table in status.rs, as SQL.
    #[test]
    fn status_rule_is_matching() {
        let conn = migrated();
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];

        sweep(&conn, A, &[("a", true, Some(200))], 1_000);
        assert_eq!(
            status_of(&conn, "a").as_deref(),
            Some("White"),
            "reachable and fast"
        );

        sweep(&conn, A, &[("a", true, Some(25_000))], 2_000);
        assert_eq!(
            status_of(&conn, "a").as_deref(),
            Some("Orange"),
            "reachable but over orange_after"
        );

        // Failures below red_after are Orange: one missed sweep is not an outage.
        for (i, now) in [3_000, 4_000].iter().enumerate() {
            sweep(&conn, A, &[("a", false, None)], *now);
            assert_eq!(
                status_of(&conn, "a").as_deref(),
                Some("Orange"),
                "failure {}",
                i + 1
            );
        }

        sweep(&conn, A, &[("a", false, None)], 5_000);
        assert_eq!(
            status_of(&conn, "a").as_deref(),
            Some("Red"),
            "at red_after consecutive failures"
        );

        // last_good_at must still point at the last sweep that actually
        // succeeded -- it is the only evidence the service ever worked.
        let (good, failures): (i64, i64) = conn
            .query_row(
                "SELECT last_good_at, consecutive_failures FROM links WHERE slug = 'a'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(good, 2_000);
        assert_eq!(failures, 3);
    }

    /// Delisting: the row stops rendering but keeps everything it learned.
    #[test]
    fn retiring_hides_a_link_without_losing_its_history() {
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];
        const AB: &[(&str, &str)] = &[("a", "http://a.onion/"), ("b", "http://b.onion/")];
        let conn = migrated();
        sweep(
            &conn,
            AB,
            &[("a", true, Some(200)), ("b", false, None)],
            1_000,
        );

        // b drops out of links.toml; only a is swept.
        sweep(&conn, A, &[("a", true, Some(200))], 2_000);

        let visible: Vec<String> = conn
            .prepare(SELECT_LINKS_SQL)
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(visible, ["a"], "a retired link must not render");

        let (retired, failures): (Option<i64>, i64) = conn
            .query_row(
                "SELECT retired_at, consecutive_failures FROM links WHERE slug = 'b'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(retired, Some(2_000));
        assert_eq!(failures, 1, "the probe history must survive the delisting");

        // Stamped once: a later sweep must not keep moving the date.
        sweep(&conn, A, &[("a", true, Some(200))], 3_000);
        let retired: Option<i64> = conn
            .query_row("SELECT retired_at FROM links WHERE slug = 'b'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            retired,
            Some(2_000),
            "retired_at records when it went missing, not when we last looked"
        );
    }

    /// Re-listing is the reason this is a soft delete.
    #[test]
    fn relisting_restores_the_row_with_its_history() {
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];
        const AB: &[(&str, &str)] = &[("a", "http://a.onion/"), ("b", "http://b.onion/")];
        let conn = migrated();
        sweep(&conn, AB, &[("b", false, None)], 1_000);
        sweep(&conn, A, &[("a", true, Some(200))], 2_000);

        sweep(&conn, AB, &[("b", false, None)], 3_000);

        let (retired, failures): (Option<i64>, i64) = conn
            .query_row(
                "SELECT retired_at, consecutive_failures FROM links WHERE slug = 'b'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(retired, None, "the upsert must clear retired_at");
        assert_eq!(
            failures, 2,
            "a re-listed link keeps its record rather than starting clean"
        );
    }

    /// The bug this ordering exists to prevent: a slug rename arrives as a new
    /// row whose url the old row still holds, and UNIQUE(url) rejects the whole
    /// batch -- so no link's status lands, every sweep, until someone notices.
    #[test]
    fn a_slug_rename_does_not_break_the_batch() {
        let conn = migrated();
        sweep(
            &conn,
            &[("dread", "http://d.onion/")],
            &[("dread", true, Some(200))],
            1_000,
        );

        // links.toml now spells it dread_forum, at the same address.
        sweep(
            &conn,
            &[("dread_forum", "http://d.onion/")],
            &[("dread_forum", true, Some(200))],
            2_000,
        );

        let rows: Vec<(String, String)> = conn
            .prepare("SELECT slug, url FROM links ORDER BY slug")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            [("dread_forum".to_string(), "http://d.onion/".to_string())]
        );
    }

    /// Every sweep leaves one sample per link, down probes as NULL, and
    /// nothing older than the retention window survives the next sweep.
    #[test]
    fn probes_are_recorded_and_pruned() {
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];
        let conn = migrated();
        let t0 = 1_000_000;

        sweep(&conn, A, &[("a", true, Some(200))], t0);
        sweep(&conn, A, &[("a", false, None)], t0 + 60);

        let samples = |conn: &Connection| -> Vec<(i64, Option<i64>)> {
            conn.prepare(
                "SELECT checked_at, latency_ms FROM probes WHERE slug = 'a' ORDER BY checked_at",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
        };
        assert_eq!(
            samples(&conn),
            [(t0 as i64, Some(200)), (t0 as i64 + 60, None)],
            "a down probe is a NULL sample, not a missing one"
        );

        // One second past retention for t0, still inside it for t0 + 60.
        let later = t0 + PROBE_RETENTION_SECS + 1;
        sweep(&conn, A, &[("a", true, Some(300))], later);
        assert_eq!(
            samples(&conn),
            [(t0 as i64 + 60, None), (later as i64, Some(300))],
            "only samples older than the window are pruned"
        );
    }

    /// Each sample carries the mean of the up samples in the window at the
    /// time it was written; earlier samples keep the mean they were given.
    #[test]
    fn probes_carry_rolling_avg() {
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];
        let conn = migrated();
        let t0 = 1_000_000;

        sweep(&conn, A, &[("a", false, None)], t0);
        sweep(&conn, A, &[("a", true, Some(200))], t0 + 60);
        sweep(&conn, A, &[("a", true, Some(400))], t0 + 120);

        let avgs: Vec<Option<f64>> = conn
            .prepare("SELECT avg_latency_ms FROM probes WHERE slug = 'a' ORDER BY checked_at")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            avgs,
            [None, Some(200.0), Some(300.0)],
            "down probes are excluded from the mean, and history is not rewritten"
        );
    }

    /// A live row must never be deleted to make room. Two live entries sharing
    /// a url is an editorial mistake, caught in `links::load`; if it ever got
    /// this far it has to fail loudly rather than quietly drop a row.
    #[test]
    fn release_url_spares_a_live_row() {
        let conn = migrated();
        sweep(
            &conn,
            &[("a", "http://shared.onion/")],
            &[("a", true, Some(200))],
            1_000,
        );

        conn.execute(RELEASE_URL_SQL, params!["b", "http://shared.onion/"])
            .unwrap();
        let live: i64 = conn
            .query_row("SELECT count(*) FROM links WHERE slug = 'a'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(live, 1, "a live row was deleted to free its url");

        assert!(
            conn.execute(UPSERT_SQL, params!["b", "http://shared.onion/", "Info", ""])
                .is_err(),
            "a duplicate url must fail rather than silently rewrite the table"
        );
    }
}
