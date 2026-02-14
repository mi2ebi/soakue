#![allow(clippy::cast_precision_loss)]

mod dedup;
mod letters;
mod old_main;
#[cfg(test)] mod tests;
mod toadua;

use std::{fs, time::Duration};

use itertools::Itertools as _;
use reqwest::blocking::Client;
use serde_json::to_string;

use crate::dedup::dictify;

const UNDERDOT: char = '\u{0323}';

#[allow(clippy::missing_panics_doc)]
pub fn main() {
    let client =
        Client::builder().timeout(Duration::from_mins(1)).build().expect("Building client failed");

    let query = r#"{"action": "search", "query": ["and"]}"#.to_string();

    println!("getting stuff from toadua");
    let res = client
        .post("https://toadua.uakci.space/api")
        .body(query)
        .send()
        .expect("Couldn't receive toadua's response");

    let text = res.text().expect("Couldn't convert toadua's response to a string");

    println!("jsonifying");
    let dict = dictify(&text);
    let dict_str = to_string(&dict).expect("Couldn't convert dictionary data to a string");

    println!(
        "{:.02}% of entries have fancy metadata!",
        dict.iter().filter(|t| t.has_metadata()).count() as f64 / dict.len() as f64 * 100.
    );

    println!("writing");
    fs::write("data/toakue.js", format!("const dict = {dict_str};")).unwrap();

    fs::write("data/all.txt", dict.iter().map(|toa| toa.head.clone()).collect_vec().join("\n"))
        .unwrap();

    fs::write("data/readable.txt", dict.iter().map(ToString::to_string).join("\n\n")).unwrap();
}
