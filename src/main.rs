#![allow(clippy::cast_precision_loss)]

mod dedup;
mod guess_metadata;
mod letters;
mod old_main;
#[cfg(test)]
mod tests;
mod toadua;
mod toakao;

use std::{fs, time::Duration};

use itertools::Itertools as _;
use reqwest::blocking::Client;
use serde_json::to_string;

use crate::dedup::dictify;

const UNDERDOT: char = '\u{0323}';

#[allow(clippy::missing_panics_doc)]
pub fn main() {
    let client = Client::builder()
        .timeout(Duration::from_mins(2))
        .build()
        .expect("Building client failed");

    println!("getting stuff from toadua");
    let toadua_text = client
        .post("https://toadua.uakci.space/api")
        .body(r#"{"action": "search", "query": ["and"]}"#)
        .send()
        .expect("Couldn't receive toadua's response")
        .text()
        .expect("Couldn't convert toadua's response to a string");

    println!("getting stuff from toakao");
    let toakao_text = client
        .get("https://raw.githubusercontent.com/toaq/toakao/refs/heads/master/toakao_extended.json")
        .send()
        .expect("Couldn't receive toakao's response")
        .text()
        .expect("Couldn't convert toakao's response to a string");

    println!("jsonifying");
    let mut dict = dictify(&toadua_text);

    println!("tagging");
    let tags = toakao::tag_map(&toakao_text);
    for toa in &mut dict {
        toa.tags = tags.get(&toa.head).cloned();
    }

    let dict_str = to_string(&dict).expect("Couldn't convert dictionary data to a string");

    println!(
        "{:.02}% of entries have fancy metadata!",
        dict.iter().filter(|t| t.has_metadata()).count() as f64 / dict.len() as f64 * 100.
    );
    println!(
        "{:.02}% of entries have tags!",
        dict.iter().filter(|t| t.tags.is_some()).count() as f64 / dict.len() as f64 * 100.
    );

    println!("writing");
    fs::write("data/toakue.js", format!("const dict = {dict_str};")).unwrap();

    fs::write(
        "data/all.txt",
        dict.iter()
            .map(|toa| toa.head.clone())
            .collect_vec()
            .join("\n"),
    )
    .unwrap();

    fs::write(
        "data/readable.txt",
        dict.iter().map(ToString::to_string).join("\n\n"),
    )
    .unwrap();

    // just for fun i fed claude data/readable.txt and asked it to write this function to try annotating stuff with metadata
    guess_metadata::run(&dict).unwrap();
}
