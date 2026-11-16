//! Hand-rolled BM25 over `(id, text)` documents. No stemming, no stop list.

const K1: f32 = 1.2;
const B: f32 = 0.75;

pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= 2)
        .map(|word| word.to_lowercase())
        .collect()
}

pub struct Index {
    ids: Vec<String>,
    doc_len: Vec<usize>,
    avg_doc_len: f32,
    term_freqs: Vec<std::collections::HashMap<String, usize>>,
    doc_freq: std::collections::HashMap<String, usize>,
    n: usize,
}

impl Index {
    pub fn build(docs: &[(String, String)]) -> Index {
        let mut ids = Vec::with_capacity(docs.len());
        let mut doc_len = Vec::with_capacity(docs.len());
        let mut term_freqs = Vec::with_capacity(docs.len());
        let mut doc_freq = std::collections::HashMap::new();

        for (id, text) in docs {
            let tokens = tokenize(text);
            ids.push(id.clone());
            doc_len.push(tokens.len());

            let mut freqs = std::collections::HashMap::new();
            for token in &tokens {
                *freqs.entry(token.clone()).or_insert(0usize) += 1;
            }
            for term in freqs.keys() {
                *doc_freq.entry(term.clone()).or_insert(0usize) += 1;
            }
            term_freqs.push(freqs);
        }

        let n = ids.len();
        let avg_doc_len = if n == 0 {
            0.0
        } else {
            doc_len.iter().sum::<usize>() as f32 / n as f32
        };

        Index {
            ids,
            doc_len,
            avg_doc_len,
            term_freqs,
            doc_freq,
            n,
        }
    }

    fn idf(&self, term: &str) -> f32 {
        let df = *self.doc_freq.get(term).unwrap_or(&0) as f32;
        let n = self.n as f32;
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, f32)> {
        let query_tokens = tokenize(query);
        let mut scores: Vec<(usize, f32)> = Vec::with_capacity(self.n);

        for doc_idx in 0..self.n {
            let doc_len = self.doc_len[doc_idx] as f32;
            let mut score = 0.0f32;
            for term in &query_tokens {
                let Some(tf) = self.term_freqs[doc_idx].get(term) else {
                    continue;
                };
                let tf = *tf as f32;
                let idf = self.idf(term);
                let denom = tf + K1 * (1.0 - B + B * (doc_len / self.avg_doc_len.max(f32::EPSILON)));
                score += idf * (tf * (K1 + 1.0)) / denom;
            }
            if score > 0.0 {
                scores.push((doc_idx, score));
            }
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores
            .into_iter()
            .take(limit)
            .map(|(idx, score)| (self.ids[idx].clone(), score))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_lowercases_and_splits_on_non_alphanumeric() {
        let tokens = tokenize("Sky-Blue, Grass_Green! a1B2 x");
        assert_eq!(tokens, vec!["sky", "blue", "grass", "green", "a1b2"]);
    }

    #[test]
    fn tokenizer_drops_short_tokens() {
        let tokens = tokenize("a I to be sky");
        assert_eq!(tokens, vec!["to", "be", "sky"]);
    }

    #[test]
    fn two_document_score_ordering_matches_hand_computation() {
        let docs = vec![
            ("short".to_string(), "cat dog".to_string()),
            (
                "long".to_string(),
                "cat cat cat dog dog dog bird bird bird".to_string(),
            ),
        ];
        let index = Index::build(&docs);
        let results = index.search("cat", 10);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "long");
        assert_eq!(results[1].0, "short");

        let long_score = results.iter().find(|(id, _)| id == "long").unwrap().1;
        let short_score = results.iter().find(|(id, _)| id == "short").unwrap().1;

        assert!(
            (long_score - 0.252).abs() < 0.001,
            "long score {long_score} not within 0.001 of 0.252"
        );
        assert!(
            (short_score - 0.246).abs() < 0.001,
            "short score {short_score} not within 0.001 of 0.246"
        );
    }
}
