#[cfg(test)]
mod tests;

mod letters;
mod toadua;

use crate::toadua::Toa;
use crate::toadua::Toadua;
use itertools::Itertools;
use reqwest::blocking::Client;
use serde_json::{from_str, to_string};
use std::{fs, time::Duration};

const UNDERDOT: char = '\u{0323}';

pub fn main() {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .expect("Building client failed");

    let query = r#"{"action": "search", "query": ["and"]}"#.to_string();

    let res = client
        .post("https://toadua.uakci.space/api")
        .body(query)
        .send()
        .expect("Couldn't receive toadua's response");

    let text = res
        .text()
        .expect("Couldn't convert toadua's response to a string");

    let dict = dictify(&text);
    let dict_str = to_string(&dict).expect("Couldn't convert dictionary data to a string");

    fs::write("data/toakue.js", format!("const dict = {dict_str};")).unwrap();

    fs::write(
        "data/all.txt",
        dict.iter()
            .map(|toa| toa.head.clone())
            .collect_vec()
            .join("\r\n"),
    )
    .unwrap();

    fs::write(
        "data/readable.txt",
        dict.iter().map(ToString::to_string).join("\r\n\r\n"),
    )
    .unwrap();
}

fn dictify(the: &str) -> Vec<Toa> {
    from_str::<Toadua>(the)
        .unwrap()
        .results
        .into_iter()
        .filter(|toa| toa.score >= -2)
        .sorted_by(Toa::cmp)
        .collect_vec()
}
