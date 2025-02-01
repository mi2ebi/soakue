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

#[derive(Deserialize, Serialize, Clone)]
pub struct Note {
    pub date: String,
    pub user: String,
    pub content: String,
}
