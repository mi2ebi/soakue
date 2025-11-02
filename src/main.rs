mod letters;
mod old_main;
#[cfg(test)] mod tests;
mod toadua;

use std::{fs, time::Duration};

use itertools::Itertools as _;
use reqwest::blocking::Client;
use serde_json::{from_str, to_string};

use crate::toadua::{Toa, Toadua};

const UNDERDOT: char = '\u{0323}';

#[allow(clippy::missing_panics_doc)]
pub fn main() {
    let client =
        Client::builder().timeout(Duration::from_secs(60)).build().expect("Building client failed");

    let query = r#"{"action": "search", "query": ["and"]}"#.to_string();

    let res = client
        .post("https://toadua.uakci.space/api")
        .body(query)
        .send()
        .expect("Couldn't receive toadua's response");

    let text = res.text().expect("Couldn't convert toadua's response to a string");

    let dict = dictify(&text);
    let dict_str = to_string(&dict).expect("Couldn't convert dictionary data to a string");

    fs::write("data/toakue.js", format!("const dict = {dict_str};")).unwrap();

    fs::write("data/all.txt", dict.iter().map(|toa| toa.head.clone()).collect_vec().join("\n"))
        .unwrap();

    fs::write("data/readable.txt", dict.iter().map(ToString::to_string).join("\n\n")).unwrap();
}

fn dictify(the: &str) -> Vec<Toa> {
    let entries = from_str::<Toadua>(the)
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
        .collect_vec();
    let mut result = vec![];
    let mut used = vec![false; entries.len()];
    for i in 0..entries.len() {
        if used[i] {
            continue;
        }
        let mut duplicates = vec![i];
        let current = &entries[i];
        for (j, entry) in entries.iter().enumerate().skip(i + 1) {
            if used[j] {
                continue;
            }
            let other = entry;
            if current.head == other.head
                && current.body == other.body
                && current.scope == other.scope
                && current.user == other.user
            {
                duplicates.push(j);
                used[j] = true;
            }
        }
        if duplicates.len() == 1 {
            result.push(entries[i].clone());
        } else {
            let keeper = choose_keeper(&entries, &duplicates);
            result.push(entries[keeper].clone());
        }
        used[i] = true;
    }
    result
}
fn choose_keeper(entries: &[Toa], duplicates: &[usize]) -> usize {
    let non_duplicate_notes: Vec<usize> = duplicates
        .iter()
        .filter(|&&idx| {
            !entries[idx].notes.iter().any(|n| n.content.to_lowercase().contains("duplicate"))
        })
        .copied()
        .collect();
    if non_duplicate_notes.len() == 1 {
        return non_duplicate_notes[0];
    }
    let candidates = if non_duplicate_notes.is_empty() { duplicates } else { &non_duplicate_notes };
    let empty_notes =
        candidates.iter().filter(|&&i| entries[i].notes.is_empty()).copied().collect_vec();
    if empty_notes.len() > 1 {
        let highest_score_i =
            empty_notes.iter().max_by_key(|&&i| entries[i].score).copied().unwrap();
        let same_score = empty_notes
            .iter()
            .filter(|&&i| entries[i].score == entries[highest_score_i].score)
            .copied()
            .collect_vec();
        if same_score.len() > 1 {
            return same_score
                .iter()
                .max_by(|&&a, &&b| entries[a].date.cmp(&entries[b].date))
                .copied()
                .unwrap();
        }
        return highest_score_i;
    }
    candidates[0]
}
