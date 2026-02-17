use std::{cmp::Ordering, fmt::Display, sync::LazyLock};

use itertools::Itertools as _;
use jiff::fmt::rfc2822;
use regex::{
    Regex,
    bytes::{Regex as Bregex, RegexBuilder as BregexBuilder},
};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization as _;

use crate::letters::{GraphResult, GraphsIter, Tone, filter};

static ONE_RAKU: LazyLock<Bregex> = LazyLock::new(|| {
    Bregex::new(r"(^|[\ mpbfntdczꝡsrljkg'h]|[ncs]h)([aeiıou])?([aeo][iı]|ao|[aeiıou][qm]?)")
        .unwrap()
});
static MANY_RAKU: LazyLock<Bregex> = LazyLock::new(|| {
    BregexBuilder::new(
        r"^((
            (^|[\ mpbfntdczꝡsrljkg'h]|[ncs]h)
            ([aeiıou])? 
            ([aeo][iı]|ao|[aeiıou][qm]?)
        )[ .,?!()]?)+$",
    )
    .ignore_whitespace(true)
    .build()
    .unwrap()
});

#[derive(Deserialize, Serialize)]
pub struct Toadua {
    pub results: Vec<Toa>,
}
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Toa {
    pub id: String,
    pub date: String,
    pub head: String,
    pub body: String,
    pub user: String,
    pub notes: Vec<Note>,
    pub score: i32,
    pub scope: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub warn: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    #[serde(
        rename(deserialize = "pronominal_class", serialize = "animacy"),
        skip_serializing_if = "Option::is_none"
    )]
    pub animacy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

fn normalize_for_validation(text: &str) -> String {
    text.nfd()
        .to_string()
        .to_lowercase()
        .replace(" a", "'a")
        .replace(" e", "'e")
        .replace(" i", "'i")
        .replace(" ı", "'i")
        .replace(" o", "'o")
        .replace(" u", "'u")
        .replace(|x| !filter(x) || "\u{0301}\u{0302}\u{0308}\u{0323}".contains(x), "")
}
pub fn split_into_raku(word: &str) -> Option<Vec<String>> {
    let normalized = normalize_for_validation(word);
    if !MANY_RAKU.is_match(normalized.as_bytes()) {
        return None;
    }
    let rakus: Vec<String> = ONE_RAKU
        .find_iter(normalized.as_bytes())
        .map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned())
        .collect();
    if rakus.is_empty() { None } else { Some(rakus) }
}

impl Toa {
    pub fn set_warning(&mut self) {
        self.warn = (["ae", "au", "ou", "nhı", "ꝡı", "ꝡu", "aıq", "aoq", "eıq", "oıq"]
            .iter()
            .any(|v| self.head.contains(v))
            || !MANY_RAKU.is_match(normalize_for_validation(&self.head).as_bytes())
            || self.head.nfc().any(|c| {
                !"aáäâạbcdeéëêẹfghıíïîịjklmnoóöôọpqrstuúüûụꝡz'\
                  AÁÄÂẠBCDEÉËÊẸFGHIÍÏÎỊJKLMNOÓÖÔỌPQRSTUÚÜÛỤꝠZ \
                  .,?!-\u{0301}\u{0302}\u{0308}\u{0323}()«»‹›"
                    .contains(c)
            })
            || self.user.starts_with("old")
            || self.head.split_whitespace().any(|word| {
                word.nfd().contains(&'\u{0308}')
                    && split_into_raku(word).is_some_and(|rakus| rakus.len() > 1)
            }))
            && !self.body.contains("textspeak")
            && !self.notes.iter().any(|n| n.content.contains("textspeak"));
    }
    pub fn fix_note_dates(&mut self) {
        self.notes = self
            .notes
            .iter()
            .map(|n| Note {
                date: rfc2822::parse(&n.date)
                    .map_or_else(|_| n.date.clone(), |noniso| noniso.timestamp().to_string()),
                ..n.clone()
            })
            .collect_vec();
    }
    pub fn has_metadata(&self) -> bool {
        [self.frame.clone(), self.animacy.clone(), self.distribution.clone(), self.subject.clone()]
            .iter()
            .any(Option::is_some)
    }
    pub fn fixup_metadata(&mut self) {
        if !self.body.clone().contains("▯") {
            self.frame = None;
            self.animacy = None;
            self.distribution = None;
            self.subject = None;
        }
        if [
            self.frame.clone(),
            self.animacy.clone(),
            self.distribution.clone(),
            self.subject.clone(),
        ]
        .iter()
        .any(|m| *m == Some("undefined".to_string()))
        {
            println!("{} #{} has bad metadata", self.head, self.id);
        }
    }
}
/*
static FRAME_NOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^frame\s*:\s*(
            0 | 1x? | 2(?:xx)? | c\s*(?:
                0 | 1[ix]? | 2(?:ix|x[ix])? | c\s*(?:
                    0 | 1[ijx]? | 2(?:i[jx]|j[ix]|x[ijx])? | c
                )?
            )?
        )$",
    )
    .unwrap()
});
static DISTRIBUTION_NOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^distribution\s*:\s*(([nd]\s*){1,3})$").unwrap());
static SUBJECT_NOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^subject\s*:\s*(free|individual|predicate|event|agent|shape)$").unwrap()
});
*/

impl Display for Toa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{} {} `{}` @{} #{} {}\n{}{}",
            if self.warn { "⚠ " } else { "" },
            self.head,
            match self.score {
                0 => "±".to_string(),
                x if x > 0 => format!("+{}", self.score),
                _ => self.score.to_string(),
            },
            self.scope,
            self.user,
            self.id,
            self.date,
            self.body,
            if self.notes.is_empty() {
                String::new()
            } else {
                "\n".to_string()
                    + &self
                        .notes
                        .iter()
                        .map(|n| format!("{} ({}): {}", n.user, n.date, n.content))
                        .join("\n")
            }
        )
    }
}

impl PartialOrd for Toa {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for Toa {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut self_iter = GraphsIter::new(&self.head);
        let mut other_iter = GraphsIter::new(&other.head);

        // Move example phrases to the end of the list
        if self.head.contains(' ') && !other.head.contains(' ') {
            if other_iter.will_fail() && !self_iter.will_fail() {
                return Ordering::Less;
            }
            return Ordering::Greater;
        }
        if other.head.contains(' ') && !self.head.contains(' ') {
            if self_iter.will_fail() && !other_iter.will_fail() {
                return Ordering::Greater;
            }
            return Ordering::Less;
        }

        let mut self_highest_tone = (Tone::None, false);
        let mut other_highest_tone = (Tone::None, false);

        loop {
            let self_letter = self_iter.next();
            let other_letter = other_iter.next();

            match (self_letter, other_letter) {
                (GraphResult::Finished, GraphResult::Finished) => {
                    // If two strings reach this point, that means that their letters are identical,
                    // so the only way to differentiate is with the tone and whether one is a
                    // prefix.
                    if self.head.ends_with('-') && !other.head.ends_with('-') {
                        return Ordering::Less;
                    }
                    if other.head.ends_with('-') && !self.head.ends_with('-') {
                        return Ordering::Greater;
                    }
                    return self_highest_tone.cmp(&other_highest_tone);
                }
                (GraphResult::Err(_), GraphResult::Err(_)) => {
                    return self_highest_tone.cmp(&other_highest_tone);
                }
                (GraphResult::Ok(self_graph), GraphResult::Ok(other_graph)) => {
                    match self_graph.letter.cmp(&other_graph.letter) {
                        Ordering::Equal => {
                            self_highest_tone =
                                self_highest_tone.max((self_graph.tone, self_graph.underdot));
                            other_highest_tone =
                                other_highest_tone.max((other_graph.tone, other_graph.underdot));
                        }
                        ordering => {
                            let self_fails = self_iter.will_fail();
                            let other_fails = other_iter.will_fail();

                            if self_fails && other_fails {
                                return ordering;
                            }
                            if self_fails {
                                return Ordering::Greater;
                            }
                            if other_fails {
                                return Ordering::Less;
                            }
                            return ordering;
                        }
                    }
                }

                // Move failures to the end of the list
                (GraphResult::Err(_), _) | (GraphResult::Ok(_), GraphResult::Finished) => {
                    return Ordering::Greater;
                }
                (_, GraphResult::Err(_)) | (GraphResult::Finished, GraphResult::Ok(_)) => {
                    return Ordering::Less;
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Note {
    pub date: String,
    pub user: String,
    pub content: String,
}
