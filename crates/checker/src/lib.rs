#[cfg(test)]
mod test {
    use shared::LinkStatus;
    use std::sync::LazyLock;

    pub const UPSERT_SQL: &str = "\
INSERT INTO links (slug, url, category, description) VALUES (?1, ?2, ?3, ?4) \
ON CONFLICT(slug) DO UPDATE SET \
  url = excluded.url, category = excluded.category, description = excluded.description";

    /// Applies the status rule. Built once at first use because the status literals
    /// come from `LinkStatus::as_sql()` rather than being typed in here.
    pub static RECORD_SQL: LazyLock<String> = LazyLock::new(|| {
        format!(
            "UPDATE links SET \
           consecutive_failures = CASE WHEN ?2 = 1 THEN 0 ELSE consecutive_failures + 1 END, \
           latency_ms = ?3, \
           last_good_at = CASE WHEN ?2 = 1 THEN ?4 ELSE last_good_at END, \
           last_checked_at = ?4, \
           status = CASE \
             WHEN ?2 = 1 AND ?3 >= ?5 THEN '{orange}' \
             WHEN ?2 = 1 THEN '{white}' \
             WHEN consecutive_failures + 1 >= ?6 THEN '{red}' \
             ELSE '{orange}' \
           END \
         WHERE slug = ?1",
            white = LinkStatus::White.as_sql(),
            orange = LinkStatus::Orange.as_sql(),
            red = LinkStatus::Red.as_sql(),
        )
    });

    #[test]
    fn status_rule_is_matching() {
        use std::time::Duration;
        let expected_orange_dur = Duration::from_secs(25);
        let expected_red_fails = 4;
        let now = std::time::Instant::now();
        //sleep for 25 and then check baseline known url?
    }
    #[test]
    fn schema_drift_is_guarded() {
        use rusqlite::*;
        use serde_json::{json, Value};
        let schema = include_str!("../../worker/schema.sql");
        let record_json = json!({
            "sql": RECORD_SQL.as_str(),
            "params": []
        });
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(UPSERT_SQL, []).unwrap();
        conn.execute(&RECORD_SQL, []).unwrap();
    }
}
