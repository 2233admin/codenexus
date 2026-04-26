use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::parser::Symbol;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path).context("open db")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS symbols (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT, name TEXT, path TEXT,
                start_line INT, end_line INT, snippet TEXT,
                search_blob TEXT,
                embedding BLOB
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS symbols_fts USING fts5(
                name, snippet, kind, search_blob, content='symbols', content_rowid='id'
            );
            CREATE TRIGGER IF NOT EXISTS symbols_ai AFTER INSERT ON symbols BEGIN
              INSERT INTO symbols_fts(rowid, name, snippet, kind, search_blob)
              VALUES (new.id, new.name, new.snippet, new.kind, new.search_blob);
            END;
            CREATE TABLE IF NOT EXISTS edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_id INTEGER NOT NULL REFERENCES symbols(id),
                to_id INTEGER NOT NULL REFERENCES symbols(id),
                kind TEXT NOT NULL CHECK (kind IN ('Calls','Imports','Implements','Extends')),
                confidence REAL NOT NULL DEFAULT 1.0,
                UNIQUE(from_id, to_id, kind)
            );
            CREATE INDEX IF NOT EXISTS edges_from ON edges(from_id, kind);
            CREATE INDEX IF NOT EXISTS edges_to   ON edges(to_id,   kind);
            "#,
        )
        .context("schema")?;
        Ok(Self { conn })
    }

    pub fn clear_edges(&self) -> Result<()> {
        self.conn.execute_batch("DELETE FROM edges;")?;
        Ok(())
    }

    pub fn insert_edge(&self, from: i64, to: i64, kind: &str) -> Result<()> {
        self.insert_edge_conf(from, to, kind, 1.0)
    }

    /// Insert with explicit confidence (per resolver step in graph_build.rs).
    /// 1.0 = same-file resolution, 0.9 = import-file resolution, 0.7 = global-unique fallback.
    pub fn insert_edge_conf(&self, from: i64, to: i64, kind: &str, confidence: f64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO edges(from_id,to_id,kind,confidence) VALUES (?,?,?,?)",
            params![from, to, kind, confidence],
        )?;
        Ok(())
    }

    /// Returns all edges of the given kinds with confidence ≥ min_conf, as
    /// `Vec<(from_id, to_id, confidence)>`. Used by graph_ppr for PPR matrix
    /// construction; ARCHITECTURE §9.7 confidence_min default = 0.5.
    pub fn edges_of_kinds(
        &self,
        kinds: &[&str],
        min_conf: f64,
    ) -> Result<Vec<(i64, i64, f64)>> {
        if kinds.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = std::iter::repeat("?")
            .take(kinds.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT from_id, to_id, confidence FROM edges \
             WHERE kind IN ({}) AND confidence >= ?",
            placeholders
        );
        let mut st = self.conn.prepare(&sql)?;
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = kinds
            .iter()
            .map(|k| k as &dyn rusqlite::ToSql)
            .collect();
        params_vec.push(&min_conf);
        let rows = st.query_map(params_vec.as_slice(), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Counts edges grouped by (kind, confidence_bucket) where bucket = round(confidence*10)/10.
    /// Returns Vec<(kind, confidence_bucket, count)> sorted by kind asc, conf desc.
    pub fn count_edges_by_kind_conf(&self) -> Result<Vec<(String, f64, i64)>> {
        let mut st = self.conn.prepare(
            "SELECT kind, ROUND(confidence, 1) as bucket, COUNT(*) FROM edges \
             GROUP BY kind, bucket ORDER BY kind ASC, bucket DESC",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_files(&self) -> Result<Vec<String>> {
        let mut st = self.conn.prepare("SELECT DISTINCT path FROM symbols")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Returns (id, name, start_line, end_line) for all symbols in `path`,
    /// ordered by start_line ascending.
    pub fn symbols_in_file_full(
        &self,
        path: &str,
    ) -> Result<Vec<(i64, String, usize, usize)>> {
        let mut st = self.conn.prepare(
            "SELECT id, name, start_line, end_line FROM symbols WHERE path=? ORDER BY start_line ASC",
        )?;
        let rows = st.query_map(params![path], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)? as usize,
                r.get::<_, i64>(3)? as usize,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn symbol_in_file_by_name(&self, path: &str, name: &str) -> Result<Option<i64>> {
        let mut st = self.conn.prepare(
            "SELECT id FROM symbols WHERE path=? AND name=? LIMIT 1",
        )?;
        let mut rows = st.query(params![path, name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get::<_, i64>(0)?))
        } else {
            Ok(None)
        }
    }

    /// Returns all symbol IDs matching `name` (used for graph entry-point lookup,
    /// supports multiple matches when the name is not globally unique).
    pub fn find_symbols_by_name(&self, name: &str) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare("SELECT id FROM symbols WHERE name=?")?;
        let rows = st.query_map(params![name], |r| r.get::<_, i64>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Look up a symbol by id; returns (path, name, kind) or None.
    pub fn symbol_by_id(&self, id: i64) -> Result<Option<(String, String, String)>> {
        let mut st = self
            .conn
            .prepare("SELECT path, name, kind FROM symbols WHERE id=? LIMIT 1")?;
        let mut rows = st.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            )))
        } else {
            Ok(None)
        }
    }

    pub fn find_global_unique(&self, name: &str) -> Result<Option<i64>> {
        let mut st = self.conn.prepare("SELECT id FROM symbols WHERE name=? LIMIT 2")?;
        let rows: Vec<i64> = st
            .query_map(params![name], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect();
        if rows.len() == 1 {
            Ok(Some(rows[0]))
        } else {
            Ok(None)
        }
    }

    /// Files imported by `from_file` (resolved target paths, derived via Imports edges).
    pub fn import_targets_for_file(&self, from_file: &str) -> Result<Vec<String>> {
        let mut st = self.conn.prepare(
            "SELECT DISTINCT s2.path
             FROM edges e
             JOIN symbols s1 ON e.from_id=s1.id
             JOIN symbols s2 ON e.to_id=s2.id
             WHERE s1.path = ?1 AND e.kind = 'Imports'",
        )?;
        let rows = st.query_map(params![from_file], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn count_edges_by_kind(&self) -> Result<Vec<(String, i64)>> {
        let mut st = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM edges GROUP BY kind ORDER BY kind")?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// JOIN edges with symbols, optionally filtered by kind. Returns
    /// (from_path, from_name, kind, to_path, to_name).
    pub fn dump_edges_join(
        &self,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, String, String, String, String)>> {
        let sql = "SELECT s1.path, s1.name, e.kind, s2.path, s2.name
                   FROM edges e
                   JOIN symbols s1 ON e.from_id=s1.id
                   JOIN symbols s2 ON e.to_id=s2.id
                   WHERE (?1 IS NULL OR e.kind = ?1)
                   LIMIT ?2";
        let mut st = self.conn.prepare(sql)?;
        let rows = st.query_map(params![kind, limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn clear(&self) -> Result<()> {
        self.conn.execute_batch("DELETE FROM symbols; DELETE FROM symbols_fts;")?;
        Ok(())
    }

    pub fn insert(&self, s: &Symbol, search_blob: &str, emb: &[f32]) -> Result<i64> {
        let blob: Vec<u8> = emb.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn.execute(
            "INSERT INTO symbols(kind,name,path,start_line,end_line,snippet,search_blob,embedding)
             VALUES (?,?,?,?,?,?,?,?)",
            params![s.kind, s.name, s.path, s.start_line as i64, s.end_line as i64, s.snippet, search_blob, blob],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn bm25(&self, query: &str, k: usize) -> Result<Vec<(i64, f32)>> {
        let mut st = self.conn.prepare(
            "SELECT rowid, bm25(symbols_fts, 10.0, 1.0, 1.0, 5.0) AS s FROM symbols_fts
             WHERE symbols_fts MATCH ?1 ORDER BY s LIMIT ?2",
        )?;
        let rows = st.query_map(params![query, k as i64], |r| {
            let id: i64 = r.get(0)?;
            let s: f64 = r.get(1)?;
            Ok((id, -s as f32))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn all_embeddings(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        let mut st = self.conn.prepare("SELECT id, embedding FROM symbols")?;
        let rows = st.query_map([], |r| {
            let id: i64 = r.get(0)?;
            let blob: Vec<u8> = r.get(1)?;
            let v: Vec<f32> = blob
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            Ok((id, v))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn fetch(&self, id: i64) -> Result<Symbol> {
        let s = self.conn.query_row(
            "SELECT kind,name,path,start_line,end_line,snippet FROM symbols WHERE id=?",
            params![id],
            |r| {
                Ok(Symbol {
                    kind: r.get(0)?,
                    name: r.get(1)?,
                    path: r.get(2)?,
                    start_line: r.get::<_, i64>(3)? as usize,
                    end_line: r.get::<_, i64>(4)? as usize,
                    snippet: r.get(5)?,
                })
            },
        )?;
        Ok(s)
    }
}
