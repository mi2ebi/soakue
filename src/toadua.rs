use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Toadua {
    pub results: Vec<Toa>,
}
#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Deserialize, Serialize, Clone)]
pub struct Note {
    pub date: String,
    pub user: String,
    pub content: String,
}
