//! One job: turn text into vectors. Recall, decisions_query and grounding never call this.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::bm25;

pub trait Embedder {
    fn model(&self) -> &str;
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
}

#[derive(Debug, Clone)]
pub struct EmbedError {
    pub detail: String,
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl std::error::Error for EmbedError {}

pub struct OllamaEmbedder {
    client: reqwest::blocking::Client,
    url: String,
    model: String,
}

impl OllamaEmbedder {
    pub fn new(url: &str, model: &str) -> OllamaEmbedder {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("the reqwest client always builds");
        OllamaEmbedder {
            client,
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }
}

impl Embedder for OllamaEmbedder {
    fn model(&self) -> &str {
        &self.model
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let body = serde_json::json!({ "model": self.model, "input": texts });
        let response = self
            .client
            .post(format!("{}/api/embed", self.url))
            .json(&body)
            .send()
            .map_err(|source| EmbedError {
                detail: source.to_string(),
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(EmbedError {
                detail: format!("ollama embed returned status {status}"),
            });
        }
        let payload: serde_json::Value = response.json().map_err(|source| EmbedError {
            detail: source.to_string(),
        })?;
        let embeddings = payload
            .get("embeddings")
            .and_then(|value| value.as_array())
            .ok_or_else(|| EmbedError {
                detail: "ollama embed response is missing an 'embeddings' array".to_string(),
            })?;
        if embeddings.len() != texts.len() {
            return Err(EmbedError {
                detail: format!(
                    "ollama embed returned {} vectors for {} inputs",
                    embeddings.len(),
                    texts.len()
                ),
            });
        }
        let mut vectors = Vec::with_capacity(embeddings.len());
        for row in embeddings {
            let values = row.as_array().ok_or_else(|| EmbedError {
                detail: "ollama embed returned a non-array vector".to_string(),
            })?;
            vectors.push(values.iter().map(|value| value.as_f64().unwrap_or(0.0) as f32).collect());
        }
        Ok(vectors)
    }
}

const HASH_DIMS: usize = 64;

fn hash_bucket(token: &str) -> usize {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in token.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (hash % HASH_DIMS as u64) as usize
}

#[doc(hidden)]
pub struct HashEmbedder {
    calls: AtomicUsize,
}

impl HashEmbedder {
    pub fn new() -> HashEmbedder {
        HashEmbedder {
            calls: AtomicUsize::new(0),
        }
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Default for HashEmbedder {
    fn default() -> Self {
        HashEmbedder::new()
    }
}

impl Embedder for HashEmbedder {
    fn model(&self) -> &str {
        "hash-64"
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut vectors = Vec::with_capacity(texts.len());
        for text in texts {
            let mut buckets = vec![0f32; HASH_DIMS];
            for token in bm25::tokenize(text) {
                buckets[hash_bucket(&token)] += 1.0;
            }
            let norm = buckets.iter().map(|value| value * value).sum::<f32>().sqrt();
            if norm > 0.0 {
                for value in buckets.iter_mut() {
                    *value /= norm;
                }
            }
            vectors.push(buckets);
        }
        Ok(vectors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embedder_is_deterministic_and_unit_length() {
        let embedder = HashEmbedder::new();
        let texts = vec!["kestrel dispatcher retries".to_string()];
        let first = embedder.embed(&texts).unwrap();
        let second = embedder.embed(&texts).unwrap();
        assert_eq!(first, second);

        let magnitude: f32 = first[0].iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.0001, "magnitude {magnitude} not unit length");
        assert_eq!(embedder.calls(), 2);
    }
}
