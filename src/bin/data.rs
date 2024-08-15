use humantime::format_duration;
use itertools::Itertools;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::{
    fs,
    time::{Duration, Instant},
};

fn main() {
    let start = Instant::now();
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let query = r#"{"action": "search", "query": ["and"]}"#.to_string();
    let mut old = r#"{"results":[]}"#.to_string();
    loop {
        let res = client
            .post("https://toadua.uakci.pl/api")
            .body(query.clone())
            .send();
        if (res.is_err()
            || !res.as_ref().unwrap().status().is_success()
            || start.elapsed() > Duration::from_secs(10))
            && !old.starts_with("<")
            && dictify(&old) != "[]"
        {
            println!("end");
            fs::write(
                "data/toakue.js",
                "const dict = ".to_string() + &dictify(&old) + ";",
            )
            .unwrap();
            break;
        }
        old = res.unwrap().text().unwrap();
        println!("{}", format_duration(start.elapsed()));
    }
}

fn dictify(the: &str) -> String {
    to_string(
        &from_str::<Toadua>(the)
            .unwrap()
            .results
            .into_iter()
            .filter(|toa| toa.score >= -2)
            .collect_vec(),
    )
    .unwrap()
}
#[derive(Deserialize, Serialize)]
struct Toadua {
    results: Vec<Toa>,
}
#[derive(Deserialize, Serialize)]
struct Toa {
    id: String,//d
    date: String,//5
    head: String,//1
    body: String,
    user: String,//2
    notes: Vec<Note>,
    score: i32,//3
    scope: String,//4
}
#[derive(Deserialize, Serialize)]
struct Note {
    date: String,
    user: String,
    content: String,
}
