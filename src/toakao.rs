use std::collections::HashMap;

use serde::Deserialize;
use serde_json::from_str;

#[derive(Deserialize)]
struct ToakaoEntry {
    lemma: String,
    tags: Option<String>,
}

pub fn tag_map(toakao_str: &str) -> HashMap<String, String> {
    from_str::<Vec<ToakaoEntry>>(toakao_str)
        .expect("toakao should be json")
        .into_iter()
        .filter_map(|e| match e.tags {
            Some(t) if !t.is_empty() => Some((e.lemma, t)),
            _ => None,
        })
        .collect()
}
