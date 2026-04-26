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
                embedding BLOB
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS symbols_fts USING fts5(
                name, snippet, kind, content='symbols', content_rowid='id'
            );
            CREATE TRIGGER IF NOT EXISTS symbols_ai AFTER INSERT ON symbols BEGIN
              INSERT INTO symbols_fts(rowid, name, snippet, kind)
              VALUES (new.id, new.name, new.snippet, new.kind);
            END;
            "#,
        )
        .context("schema")?;
        Ok(Self { conn })
    }

    pub fn clear(&self) -> Result<()> {
        self.conn.execute_batch("DELETE FROM symbols; DELETE FROM symbols_fts;")?;
        Ok(())
    }

    pub fn insert(&self, s: &Symbol, emb: &[f32]) -> Result<i64> {
        let blob: Vec<u8> = emb.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn.execute(
            "INSERT INTO symbols(kind,name,path,start_line,end_line,snippet,embedding) VALUES (?,?,?,?,?,?,?)",
            params![s.kind, s.name, s.path, s.start_line as i64, s.end_line as i64, s.snippet, blob],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn bm25(&self, query: &str, k: usize) -> Result<Vec<(i64, f32)>> {
        let mut st = self.conn.prepare(
            "SELECT rowid, bm25(symbols_fts) AS s FROM symbols_fts
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
