use std::{cmp::Ordering, fmt::Display, sync::LazyLock};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::letters::{filter, GraphResult, GraphsIter, Tone};

static MADE_OF_RAKU: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        "^(((^|[mpbfntdczꝡsrljvkg'wh \
         ]|[ncs]h)([aeiouáéíóúâêîôûäëïẹịöụüıạọ]([aeo](ı)|ao|[aeiıou])?\
         |[aeiouáéíóúâêîôûäëïẹịöụüıạọ])[qm]?)[ .,?!()]?)+$",
    )
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
}

impl Toa {
    pub fn set_warning(&mut self) {
        self.warn = ([
            "ae", "au", "ou", "nhi", "vi", "vu", "aiq", "aoq", "eiq", "oiq", "ꝡi", "ꝡu", "ae",
            "au", "ou", "nhı", "vı", "vu", "aıq", "aoq", "eıq", "oiq", "ꝡı", "ꝡu",
        ]
        .iter()
        .any(|v| self.head.contains(v))
            || {
                !MADE_OF_RAKU.is_match(
                    &self
                        .head
                        .to_lowercase()
                        .replace(|x| !filter(x) && !x.is_whitespace(), ""),
                )
            }
            || self.head.chars().any(|c| {
                !"aáäâạbcdeéëêẹfghıíïîịjklmnoóöôọpqrstuúüûụꝡz'\
                  AÁÄÂẠBCDEÉËÊẸFGHIÍÏÎỊJKLMNOÓÖÔỌPQRSTUÚÜÛỤꝠZ \
                  .,?!-\u{0323}()«»‹›\u{0301}\u{0308}\u{0302}"
                    .contains(c)
            })
            || self.user.starts_with("old"))
            && !self.body.contains("textspeak")
            && !self.notes.iter().any(|n| n.content.contains("textspeak"));
    }
}

impl Display for Toa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{} {} `{}` @{} #{}\n{}",
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
            self.body,
        )
    }
}

impl PartialOrd for Toa {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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
        } else if other.head.contains(' ') && !self.head.contains(' ') {
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
                    } else if other.head.ends_with('-') && !self.head.ends_with('-') {
                        return Ordering::Greater;
                    }
                    return self_highest_tone.cmp(&other_highest_tone);
                }
                (GraphResult::Err(_), GraphResult::Err(_)) => {
                    return self_highest_tone.cmp(&other_highest_tone)
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
                            } else if self_fails {
                                return Ordering::Greater;
                            } else if other_fails {
                                return Ordering::Less;
                            }
                            return ordering;
                        }
                    }
                }

                // Move failures to the end of the list
                (GraphResult::Err(_), _) | (GraphResult::Ok(_), GraphResult::Finished) => {
                    return Ordering::Greater
                }
                (_, GraphResult::Err(_)) | (GraphResult::Finished, GraphResult::Ok(_)) => {
                    return Ordering::Less
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
