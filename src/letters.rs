use std::{iter::Peekable, str::Chars};

use unicode_normalization::{Decompositions, UnicodeNormalization as _};

use crate::UNDERDOT;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NotToaqChar(char),
}

// The characters we want to sort
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Letter {
    Aomoi,
    A,
    B,
    C,
    Ch,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    Nh,
    O,
    P,
    Q,
    R,
    S,
    Sh,
    T,
    U,
    V,
    Z,
}

impl Letter {
    pub const fn build_h_digraph(chr: char) -> Option<Self> {
        match chr {
            'c' => Some(Self::Ch),
            'n' => Some(Self::Nh),
            's' => Some(Self::Sh),
            _ => None,
        }
    }
}

impl TryFrom<char> for Letter {
    type Error = Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value.to_lowercase().next().expect("Lowercase should be single character") {
            '\'' => Ok(Self::Aomoi),
            'a' => Ok(Self::A),
            'b' => Ok(Self::B),
            'c' => Ok(Self::C),
            'd' => Ok(Self::D),
            'e' => Ok(Self::E),
            'f' => Ok(Self::F),
            'g' => Ok(Self::G),
            'h' => Ok(Self::H),
            'i' | 'ı' => Ok(Self::I),
            'j' => Ok(Self::J),
            'k' => Ok(Self::K),
            'l' => Ok(Self::L),
            'm' => Ok(Self::M),
            'n' => Ok(Self::N),
            'o' => Ok(Self::O),
            'p' => Ok(Self::P),
            'q' => Ok(Self::Q),
            'r' => Ok(Self::R),
            's' => Ok(Self::S),
            't' => Ok(Self::T),
            'u' => Ok(Self::U),
            'v' | 'w' | 'ꝡ' => Ok(Self::V),
            'z' => Ok(Self::Z),
            _ => Err(Error::NotToaqChar(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tone {
    None,
    Verb,
    Noun,
    Clause,
    Adjunct,
}

/// A Toaq Grapheme
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Grapheme {
    pub tone: Tone,
    pub letter: Letter,
    pub underdot: bool,
}

/// Encodes the possibility of failure or success of a Graphemes Iterator
#[derive(Debug, PartialEq, Eq)]
pub enum GraphResult {
    Ok(Grapheme),
    Err(Error),
    Finished,
}

pub fn filter(x: char) -> bool {
    !x.is_whitespace() && !['*', '-', '.', ',', '!', '?', '«', '»', '"'].contains(&x)
}

/// An "iterator" over a string's `Graph`emes. Returns `MaybeGraph` to encode
/// the possibility of finishing with failure instead of succesful iteration.
pub struct GraphsIter<'a> {
    base: Peekable<Decompositions<Chars<'a>>>,
    lowest_tone: Tone,
}

impl<'a> GraphsIter<'a> {
    pub fn new(string: &'a str) -> Self {
        let lowest_tone = if string.contains('\u{0300}') { Tone::None } else { Tone::Verb };

        Self { base: string.nfd().peekable(), lowest_tone }
    }

    pub fn next(&mut self) -> GraphResult {
        let mut letter = None;

        for char in self.base.by_ref() {
            if filter(char) {
                letter = Some(char);
                break;
            }
        }

        let Some(letter) = letter else {
            return GraphResult::Finished;
        };

        let letter = match letter {
            'n' | 'c' | 's' => {
                if self.base.next_if(|chr| *chr == 'h').is_some() {
                    Letter::build_h_digraph(letter)
                        .expect("UNREACHABLE: n, c and s all have digraphs")
                } else {
                    letter
                        .try_into()
                        .expect("UNREACHABLE: n, c and s are Toaq letters and should parse")
                }
            }
            letter => match letter.try_into() {
                Ok(letter) => letter,
                Err(err) => return GraphResult::Err(err),
            },
        };

        let mut tone = self.lowest_tone;
        let mut underdot = false;

        // Tone and underdot characters are always after the letter.
        self.base.next_if(|next| tone_or_underdot(*next, &mut tone, &mut underdot));
        self.base.next_if(|next| tone_or_underdot(*next, &mut tone, &mut underdot));

        GraphResult::Ok(Grapheme { tone, letter, underdot })
    }

    pub fn will_fail(&mut self) -> bool {
        loop {
            let next = self.next();

            match next {
                GraphResult::Ok(_) => {}
                GraphResult::Err(_) => return true,
                GraphResult::Finished => return false,
            }
        }
    }
}

/// Modifies `tone` and `underdot` if `chr` is either an underdot character or a
/// tone character. Returns true only if this happens.
const fn tone_or_underdot(chr: char, tone: &mut Tone, underdot: &mut bool) -> bool {
    match chr {
        '\u{0300}' => {
            *tone = Tone::Verb;
            *underdot = true;
        }
        '\u{0301}' => *tone = Tone::Noun,
        '\u{0308}' => *tone = Tone::Clause,
        '\u{0302}' => *tone = Tone::Adjunct,
        UNDERDOT => *underdot = true,
        _ => return false,
    }
    true
}
