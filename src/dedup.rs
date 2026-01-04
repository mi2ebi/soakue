use indexmap::IndexMap;
use itertools::Itertools as _;
use serde_json::from_str;

use crate::toadua::{Toa, Toadua};

#[derive(Hash, Eq, PartialEq)]
pub struct EntryKey {
    head: String,
    body: String,
    scope: String,
    user: String,
}
impl EntryKey {
    pub fn from_toa(toa: &Toa) -> Self {
        Self {
            head: toa.head.clone(),
            body: toa.body.clone(),
            scope: toa.scope.clone(),
            user: toa.user.clone(),
        }
    }
}

pub fn dictify(the: &str) -> Vec<Toa> {
    let entries: Vec<Toa> = from_str::<Toadua>(the)
        .unwrap_or_else(|_| panic!("toadua should be json, but its actual content is:\n{the}"))
        .results
        .into_iter()
        .filter(|toa| toa.score > -2 && !toa.date.starts_with("2025-09-21T1"))
        .map(|mut toa| {
            toa.scope = toa.scope.strip_suffix("-arch").unwrap_or(&toa.scope).to_string();
            toa
        })
        .update(Toa::set_warning)
        .sorted_by(Toa::cmp)
        .collect();
    println!("deduplicating");
    let mut groups: IndexMap<EntryKey, Vec<usize>> = IndexMap::new();
    for (i, entry) in entries.iter().enumerate() {
        groups.entry(EntryKey::from_toa(entry)).or_default().push(i);
    }
    groups
        .values()
        .map(|indices| {
            let keeper_idx =
                if indices.len() == 1 { indices[0] } else { choose_keeper(&entries, indices) };
            entries[keeper_idx].clone()
        })
        .sorted_by(Toa::cmp)
        .collect()
}

pub fn choose_keeper(entries: &[Toa], duplicates: &[usize]) -> usize {
    let non_duplicate_notes: Vec<usize> = duplicates
        .iter()
        .filter(|&&idx| {
            !entries[idx].notes.iter().any(|n| n.content.to_lowercase().contains("duplicate"))
        })
        .copied()
        .collect();
    let candidates = if non_duplicate_notes.is_empty() { duplicates } else { &non_duplicate_notes };
    let empty_notes: Vec<usize> =
        candidates.iter().filter(|&&i| entries[i].notes.is_empty()).copied().collect();
    if empty_notes.len() > 1 {
        let best_score = empty_notes.iter().map(|&i| entries[i].score).max().unwrap();
        let same_score: Vec<usize> =
            empty_notes.iter().filter(|&&i| entries[i].score == best_score).copied().collect();
        if same_score.len() > 1 {
            return *same_score.iter().max_by_key(|&&i| &entries[i].date).unwrap();
        }
        return same_score[0];
    }
    candidates[0]
}
