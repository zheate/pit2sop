use crate::models::{CaptureStatus, SearchResult};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::fs;
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open sqlite db {}", path.display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS capture_events (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                raw_text TEXT,
                obsidian_path TEXT,
                error TEXT
            );

            CREATE TABLE IF NOT EXISTS pits (
                id TEXT PRIMARY KEY,
                capture_id TEXT NOT NULL,
                title TEXT NOT NULL,
                scenario TEXT NOT NULL,
                risk TEXT NOT NULL,
                recurrence TEXT NOT NULL,
                sop_title TEXT,
                file_path TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sops (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                risk TEXT NOT NULL,
                version INTEGER NOT NULL,
                file_path TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS scenes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                risk TEXT NOT NULL,
                trigger_keywords TEXT NOT NULL,
                matched_sops TEXT NOT NULL,
                file_path TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS trigger_events (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                payload TEXT NOT NULL,
                detected_scene TEXT,
                matched_sop TEXT,
                confidence REAL,
                action TEXT,
                created_at TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
                doc_id,
                doc_type,
                title,
                path,
                body
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_capture(
        &self,
        id: &str,
        source_type: &str,
        status: CaptureStatus,
        raw_text: &str,
        created_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO capture_events (id, source_type, status, created_at, raw_text)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
              status = excluded.status,
              raw_text = excluded.raw_text
            "#,
            params![
                id,
                source_type,
                status_to_str(&status),
                created_at,
                raw_text
            ],
        )?;
        Ok(())
    }

    pub fn mark_capture(
        &self,
        id: &str,
        status: CaptureStatus,
        obsidian_path: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE capture_events
            SET status = ?2, obsidian_path = COALESCE(?3, obsidian_path), error = ?4
            WHERE id = ?1
            "#,
            params![id, status_to_str(&status), obsidian_path, error],
        )?;
        Ok(())
    }

    pub fn capture_status(&self, id: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT status FROM capture_events WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn upsert_pit(
        &self,
        id: &str,
        capture_id: &str,
        title: &str,
        scenario: &str,
        risk: &str,
        recurrence: &str,
        sop_title: Option<&str>,
        file_path: &str,
        created_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO pits (
              id, capture_id, title, scenario, risk, recurrence, sop_title, file_path, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              scenario = excluded.scenario,
              risk = excluded.risk,
              recurrence = excluded.recurrence,
              sop_title = excluded.sop_title,
              file_path = excluded.file_path
            "#,
            params![
                id, capture_id, title, scenario, risk, recurrence, sop_title, file_path, created_at
            ],
        )?;
        Ok(())
    }

    pub fn upsert_sop(
        &self,
        id: &str,
        title: &str,
        status: &str,
        risk: &str,
        version: i64,
        file_path: &str,
        updated_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO sops (id, title, status, risk, version, file_path, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              status = excluded.status,
              risk = excluded.risk,
              version = excluded.version,
              file_path = excluded.file_path,
              updated_at = excluded.updated_at
            "#,
            params![id, title, status, risk, version, file_path, updated_at],
        )?;
        Ok(())
    }

    pub fn clear_search_index(&self) -> Result<()> {
        self.conn.execute("DELETE FROM search_index", [])?;
        Ok(())
    }

    pub fn index_document(
        &self,
        doc_id: &str,
        doc_type: &str,
        title: &str,
        path: &str,
        body: &str,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM search_index WHERE doc_id = ?1",
            params![doc_id],
        )?;
        self.conn.execute(
            r#"
            INSERT INTO search_index (doc_id, doc_type, title, path, body)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![doc_id, doc_type, title, path, body],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let like = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT doc_type, title, path, body
            FROM search_index
            WHERE title LIKE ?1 OR body LIKE ?1 OR path LIKE ?1
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![like, limit as i64], |row| {
            let body: String = row.get(3)?;
            Ok(SearchResult {
                doc_type: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                snippet: make_snippet(&body, query),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn status_to_str(status: &CaptureStatus) -> &'static str {
    match status {
        CaptureStatus::Created => "created",
        CaptureStatus::Queued => "queued",
        CaptureStatus::Sending => "sending",
        CaptureStatus::Delivered => "delivered",
        CaptureStatus::Processing => "processing",
        CaptureStatus::Processed => "processed",
        CaptureStatus::Failed => "failed",
        CaptureStatus::NeedsReview => "needs_review",
    }
}

fn make_snippet(body: &str, query: &str) -> String {
    let max_len = 160;
    if let Some(pos) = body.to_ascii_lowercase().find(&query.to_ascii_lowercase()) {
        let chars = body.chars().collect::<Vec<_>>();
        let char_pos = body
            .char_indices()
            .take_while(|(idx, _)| *idx < pos)
            .count();
        let start = char_pos.saturating_sub(50);
        let end = (char_pos + max_len).min(chars.len());
        return chars[start..end]
            .iter()
            .collect::<String>()
            .replace('\n', " ");
    }

    body.chars()
        .take(max_len)
        .collect::<String>()
        .replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::make_snippet;

    #[test]
    fn snippet_does_not_split_utf8_chars() {
        let body = "在M 二八蓝光组装过程中，操作员将PBS装反，导致拨片未朝向大反方向。".repeat(8);
        let snippet = make_snippet(&body, "PBS");
        assert!(snippet.contains("PBS装反"));
    }
}
