use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const URL: &str = "https://api.jina.ai/v1/rerank";
const MODEL: &str = "jina-reranker-v2-base-multilingual";

#[derive(Serialize)]
struct Req<'a> {
    model: &'a str,
    query: &'a str,
    documents: Vec<&'a str>,
    top_n: usize,
}

#[derive(Deserialize)]
struct Resp {
    results: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    index: usize,
    relevance_score: f32,
}

pub struct Reranker {
    client: reqwest::blocking::Client,
    api_key: String,
}

impl Reranker {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("JINA_API_KEY").context(
            "JINA_API_KEY env var not set — pass inline: JINA_API_KEY=... cargo run -- ...",
        )?;
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
            api_key,
        })
    }

    /// Rerank `documents` against `query`, return up to `top_n` (index, score) pairs sorted desc.
    pub fn rerank(
        &self,
        query: &str,
        documents: Vec<&str>,
        top_n: usize,
    ) -> Result<Vec<(usize, f32)>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let r: Resp = self
            .client
            .post(URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&Req {
                model: MODEL,
                query,
                documents,
                top_n,
            })
            .send()
            .context("jina http")?
            .error_for_status()
            .context("jina status")?
            .json()
            .context("jina json")?;
        Ok(r.results.into_iter().map(|x| (x.index, x.relevance_score)).collect())
    }
}
