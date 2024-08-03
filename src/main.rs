use itertools::Itertools;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, json, to_string, Value};
use std::{fs, time::Duration};

fn main() {
    let words = fs::read_to_string("dictionary-counter/toadua.txt")
        .unwrap()
        .lines()
        .map(String::from)
        .collect_vec();
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    println!("queryifying");
    let query = format!(
        r#"{{"action": "search", "query": ["and", ["scope", "en"], {}]}}"#,
        to_toadua_query(words)
    );
    println!("done");
    let mut old = r#"{"results":[]}"#.to_string();
    loop {
        let res = client
            .post("https://toadua.uakci.pl/api")
            .body(query.clone())
            .send();
        if res.is_err() || !res.as_ref().unwrap().status().is_success() {
            println!("{res:?}");
            fs::write("toakue.json", dictify(old)).unwrap();
            break;
        }
        old = res.unwrap().text().unwrap();
    }
}

// kıjı hóm laqme :3
fn to_toadua_query(words: Vec<String>) -> Value {
    if words.is_empty() {
        json!(["or"])
    } else {
        let mut query = vec!["or".into()];
        query.extend(words.iter().map(|word| json!(["head", word])));
        json!(query)
    }
}

fn dictify(the: String) -> String {
    to_string(&from_str::<Toadua>(&the).unwrap().results).unwrap()
}
#[derive(Deserialize, Serialize)]
struct Toadua {
    results: Vec<Toa>,
}
#[derive(Deserialize, Serialize)]
struct Toa {
    id: String,
    date: String,
    head: String,
    body: String,
    user: String,
    notes: Vec<Note>,
    score: i32,
}
#[derive(Deserialize, Serialize)]
struct Note {
    date: String,
    user: String,
    content: String,
}
