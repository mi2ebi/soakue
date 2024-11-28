use chrono::Utc;
use itertools::Itertools;
use regex::bytes::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::{fs, sync::LazyLock, time::Duration};
use unicode_normalization::UnicodeNormalization;

fn main() {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let query = r#"{"action": "search", "query": ["and"]}"#.to_string();
    let res = client
        .post("https://toadua.uakci.space/api")
        .body(query.clone())
        .send();
    let text = res.unwrap().text().unwrap();
    let dict = dictify(&text);
    let dict_str = to_string(&dict).unwrap();
    fs::write("data/toakue.js", format!("const dict = {dict_str};")).unwrap();
    fs::write(
        "data/all.txt",
        dict.iter()
            .map(|toa| toa.head.clone())
            .collect_vec()
            .join("\r\n"),
    )
    .unwrap();
    fs::write("data/time.txt", format!("{:?}", Utc::now())).unwrap();
}

static DOT_TONE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("\u{0323}([\u{0301}\u{0302}\u{0308}])").unwrap());
static PALATAL: LazyLock<Regex> = LazyLock::new(|| Regex::new("([ncsNCS])[hH]").unwrap());
const TONES: &str = "\u{0300}\u{0301}\u{0308}\u{0302}";
const CONSONANTS_STR: &str = "[bcdfghjklmnpqrstvz'ʰBCDFGHJKLMNPQRSTVZ]";
const VOWELS_STR: &str = "[aeiouAEIOU]";
// static CONSONANTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(CONSONANTS_STR).unwrap());
static VOWELS: LazyLock<Regex> = LazyLock::new(|| Regex::new(VOWELS_STR).unwrap());
static FIND_STEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("\u{0323}({CONSONANTS_STR}*{VOWELS_STR})")).unwrap());

static MADE_OF_RAKU: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^((^|[mpbfntdczsrljvkg'h ]|[ncs]ʰ)[aeiou]?([aeo]i|ao|[aeiou][qm]?)|[ .,?!()])+$")
        .unwrap()
});

fn dictify(the: &str) -> Vec<Toa> {
    let out = from_str::<Toadua>(the)
        .unwrap()
        .results
        .into_iter()
        .filter(|toa| toa.score >= -2)
        .map(|toa| {
            let tone_info = tones(&toa.head);
            (
                (
                    tone_info.0,
                    tone_info.1,
                    tone_info.2,
                    -toa.score,
                    toa.clone().scope,
                    toa.clone().date,
                    toa.clone().body,
                ),
                toa,
            )
        })
        .map(|(info, toa)| {
            (
                info.clone(),
                Toa {
                    warn: ([
                        "ae", "au", "ou", "nhi", "vi", "vu", "aiq", "aoq", "eiq", "oiq",
                    ]
                    .iter()
                    .any(|v| info.0.contains(v))
                        || !MADE_OF_RAKU.is_match(info.0.as_bytes())
                        || toa.head.chars().any(|c| {
                            !"aáäâạbcdeéëêẹfghıíïîịjklmnoóöôọpqrstuúüûụꝡz'\
                              AÁÄÂẠBCDEÉËÊẸFGHIÍÏÎỊJKLMNOÓÖÔỌPQRSTUÚÜÛỤꝠZ \
                              .,?!-\u{0323}()«»‹›\u{0301}\u{0308}\u{0302}"
                                .contains(c)
                        })
                        || toa.user.starts_with("old"))
                        && !toa.body.contains("textspeak")
                        && !toa.notes.iter().any(|n| n.content.contains("textspeak")),
                    ..toa
                },
            )
        })
        .sorted_by_key(|(info, _)| info.clone())
        .map(|(_, toa)| toa)
        .collect_vec();
    out
}

fn tones(head: &str) -> (String, Vec<usize>, Vec<usize>) {
    let head = String::from_utf8(
        PALATAL
            .replace_all(
                &DOT_TONE.replace(
                    head.nfd()
                        .to_string()
                        .to_lowercase()
                        .trim_start_matches(|c| "*-@., ".contains(c))
                        .chars()
                        .map(|c| match c {
                            'ı' => 'i',
                            'ꝡ' => 'v',
                            x => x,
                        })
                        .filter(|c| !"()".contains(*c))
                        .collect::<String>()
                        .as_bytes(),
                    "$1\u{0323}".as_bytes(),
                ),
                "$1ʰ".as_bytes(),
            )
            .to_vec(),
    )
    .unwrap();
    let mut tones = vec![];
    let mut nat_indices = vec![];
    let mut moved = vec![];
    for word in head.split_whitespace() {
        let mut tone = 1;
        if !word.contains(|c| format!("\u{0323}{TONES}").contains(c)) {
            moved.push(
                String::from_utf8(
                    VOWELS
                        .replace(word.as_bytes(), "$0\u{0300}".as_bytes())
                        .to_vec(),
                )
                .unwrap(),
            );
        }
        for c in word.chars() {
            if c == '\u{0323}' {
                moved.push(
                    String::from_utf8(
                        FIND_STEM
                            .replace(word.as_bytes(), format!("$1{}", n2t(tone)).as_bytes())
                            .to_vec(),
                    )
                    .unwrap(),
                );
            }
            if TONES.contains(c) {
                tone = t2n(c);
                moved.push(word.to_string());
            }
        }
    }
    let moved = moved.join(" ");
    for (i, c) in moved.chars().enumerate() {
        if TONES.contains(c) {
            nat_indices.push(i - 1 - tones.len());
            tones.push(t2n(c));
        }
    }
    if moved.ends_with('-') {
        *tones.iter_mut().last().unwrap() = 9;
    }
    let head = moved
        .replace(|c| TONES.contains(c), "")
        .trim_end_matches('-')
        .to_string();
    (head, tones, nat_indices)
}

fn t2n(c: char) -> usize {
    TONES.chars().position(|t| t == c).unwrap() + 1
}
fn n2t(n: usize) -> char {
    TONES.chars().nth(n - 1).unwrap()
}

#[derive(Deserialize, Serialize)]
struct Toadua {
    results: Vec<Toa>,
}
#[derive(Deserialize, Serialize, Clone)]
struct Toa {
    id: String,
    date: String,
    head: String,
    body: String,
    user: String,
    notes: Vec<Note>,
    score: i32,
    scope: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    warn: bool,
}
#[derive(Deserialize, Serialize, Clone)]
struct Note {
    date: String,
    user: String,
    content: String,
}
