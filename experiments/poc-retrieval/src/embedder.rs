use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const OLLAMA_URL: &str = "http://localhost:11434/api/embeddings";
const MODEL: &str = "qwen3-embedding:0.6b";
const QUERY_INSTRUCT: &str =
    "Instruct: Given a natural language code search query, retrieve the most relevant code symbol from a TypeScript codebase\nQuery: ";

#[derive(Copy, Clone)]
pub enum Role {
    Query,
    Passage,
}

#[derive(Serialize)]
struct Req<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct Resp {
    embedding: Vec<f32>,
}

pub struct Embedder {
    client: reqwest::blocking::Client,
}

impl Embedder {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
        }
    }

    pub fn embed(&self, text: &str, role: Role) -> Result<Vec<f32>> {
        let prompt: String = match role {
            Role::Query => format!("{}{}", QUERY_INSTRUCT, text),
            Role::Passage => text.to_string(),
        };
        let r: Resp = self
            .client
            .post(OLLAMA_URL)
            .json(&Req {
                model: MODEL,
                prompt: &prompt,
            })
            .send()
            .context("ollama http")?
            .error_for_status()
            .context("ollama status")?
            .json()
            .context("ollama json")?;
        Ok(r.embedding)
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = (na.sqrt() * nb.sqrt()).max(1e-12);
    dot / denom
}
