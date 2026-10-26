use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors related to parsing or handling Czech letters.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CzechLetterError {
    #[error("Invalid Czech letter token: '{0}'")]
    InvalidLetter(String),
    #[error("Invalid character in Czech text: '{0}'")]
    InvalidChar(char),
    #[error("Empty string cannot be parsed as a Czech letter")]
    Empty,
}

/// The 42-grapheme Czech crossword alphabet according to section 4 of `PROJECT.md`.
///
/// In Czech crosswords, the digraph "CH" occupies exactly ONE grid cell and is treated
/// as a single letter variant (`CzechLetter::Ch = 13`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CzechLetter {
    A = 0,
    Á = 1,
    B = 2,
    C = 3,
    Č = 4,
    D = 5,
    Ď = 6,
    E = 7,
    É = 8,
    Ě = 9,
    F = 10,
    G = 11,
    H = 12,
    Ch = 13,
    I = 14,
    Í = 15,
    J = 16,
    K = 17,
    L = 18,
    M = 19,
    N = 20,
    Ň = 21,
    O = 22,
    Ó = 23,
    P = 24,
    Q = 25,
    R = 26,
    Ř = 27,
    S = 28,
    Š = 29,
    T = 30,
    Ť = 31,
    U = 32,
    Ú = 33,
    Ů = 34,
    V = 35,
    W = 36,
    X = 37,
    Y = 38,
    Ý = 39,
    Z = 40,
    Ž = 41,
}

impl CzechLetter {
    /// Total count of graphemes in the Czech crossword alphabet (42).
    pub const COUNT: usize = 42;

    /// All 42 Czech letters in canonical index order.
    pub const ALL: [CzechLetter; 42] = [
        CzechLetter::A,
        CzechLetter::Á,
        CzechLetter::B,
        CzechLetter::C,
        CzechLetter::Č,
        CzechLetter::D,
        CzechLetter::Ď,
        CzechLetter::E,
        CzechLetter::É,
        CzechLetter::Ě,
        CzechLetter::F,
        CzechLetter::G,
        CzechLetter::H,
        CzechLetter::Ch,
        CzechLetter::I,
        CzechLetter::Í,
        CzechLetter::J,
        CzechLetter::K,
        CzechLetter::L,
        CzechLetter::M,
        CzechLetter::N,
        CzechLetter::Ň,
        CzechLetter::O,
        CzechLetter::Ó,
        CzechLetter::P,
        CzechLetter::Q,
        CzechLetter::R,
        CzechLetter::Ř,
        CzechLetter::S,
        CzechLetter::Š,
        CzechLetter::T,
        CzechLetter::Ť,
        CzechLetter::U,
        CzechLetter::Ú,
        CzechLetter::Ů,
        CzechLetter::V,
        CzechLetter::W,
        CzechLetter::X,
        CzechLetter::Y,
        CzechLetter::Ý,
        CzechLetter::Z,
        CzechLetter::Ž,
    ];

    /// Returns the uppercase string representation of this letter.
    #[inline]
    pub fn to_str(&self) -> &'static str {
        match self {
            CzechLetter::A => "A",
            CzechLetter::Á => "Á",
            CzechLetter::B => "B",
            CzechLetter::C => "C",
            CzechLetter::Č => "Č",
            CzechLetter::D => "D",
            CzechLetter::Ď => "Ď",
            CzechLetter::E => "E",
            CzechLetter::É => "É",
            CzechLetter::Ě => "Ě",
            CzechLetter::F => "F",
            CzechLetter::G => "G",
            CzechLetter::H => "H",
            CzechLetter::Ch => "CH",
            CzechLetter::I => "I",
            CzechLetter::Í => "Í",
            CzechLetter::J => "J",
            CzechLetter::K => "K",
            CzechLetter::L => "L",
            CzechLetter::M => "M",
            CzechLetter::N => "N",
            CzechLetter::Ň => "Ň",
            CzechLetter::O => "O",
            CzechLetter::Ó => "Ó",
            CzechLetter::P => "P",
            CzechLetter::Q => "Q",
            CzechLetter::R => "R",
            CzechLetter::Ř => "Ř",
            CzechLetter::S => "S",
            CzechLetter::Š => "Š",
            CzechLetter::T => "T",
            CzechLetter::Ť => "Ť",
            CzechLetter::U => "U",
            CzechLetter::Ú => "Ú",
            CzechLetter::Ů => "Ů",
            CzechLetter::V => "V",
            CzechLetter::W => "W",
            CzechLetter::X => "X",
            CzechLetter::Y => "Y",
            CzechLetter::Ý => "Ý",
            CzechLetter::Z => "Z",
            CzechLetter::Ž => "Ž",
        }
    }

    /// Converts this letter to its `u8` index (0..41).
    #[inline]
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Constructs a `CzechLetter` from a `u8` index, or `None` if index >= 42.
    #[inline]
    pub fn from_u8(val: u8) -> Option<Self> {
        if (val as usize) < Self::COUNT {
            Some(Self::ALL[val as usize])
        } else {
            None
        }
    }

    /// Maps a single uppercase Czech character (excluding digraph "CH") to `CzechLetter`.
    pub fn from_char(c: char) -> Result<Self, CzechLetterError> {
        let upper: Vec<char> = c.to_uppercase().collect();
        if upper.len() != 1 {
            return Err(CzechLetterError::InvalidChar(c));
        }
        match upper[0] {
            'A' => Ok(CzechLetter::A),
            'Á' => Ok(CzechLetter::Á),
            'B' => Ok(CzechLetter::B),
            'C' => Ok(CzechLetter::C),
            'Č' => Ok(CzechLetter::Č),
            'D' => Ok(CzechLetter::D),
            'Ď' => Ok(CzechLetter::Ď),
            'E' => Ok(CzechLetter::E),
            'É' => Ok(CzechLetter::É),
            'Ě' => Ok(CzechLetter::Ě),
            'F' => Ok(CzechLetter::F),
            'G' => Ok(CzechLetter::G),
            'H' => Ok(CzechLetter::H),
            'I' => Ok(CzechLetter::I),
            'Í' => Ok(CzechLetter::Í),
            'J' => Ok(CzechLetter::J),
            'K' => Ok(CzechLetter::K),
            'L' => Ok(CzechLetter::L),
            'M' => Ok(CzechLetter::M),
            'N' => Ok(CzechLetter::N),
            'Ň' => Ok(CzechLetter::Ň),
            'O' => Ok(CzechLetter::O),
            'Ó' => Ok(CzechLetter::Ó),
            'P' => Ok(CzechLetter::P),
            'Q' => Ok(CzechLetter::Q),
            'R' => Ok(CzechLetter::R),
            'Ř' => Ok(CzechLetter::Ř),
            'S' => Ok(CzechLetter::S),
            'Š' => Ok(CzechLetter::Š),
            'T' => Ok(CzechLetter::T),
            'Ť' => Ok(CzechLetter::Ť),
            'U' => Ok(CzechLetter::U),
            'Ú' => Ok(CzechLetter::Ú),
            'Ů' => Ok(CzechLetter::Ů),
            'V' => Ok(CzechLetter::V),
            'W' => Ok(CzechLetter::W),
            'X' => Ok(CzechLetter::X),
            'Y' => Ok(CzechLetter::Y),
            'Ý' => Ok(CzechLetter::Ý),
            'Z' => Ok(CzechLetter::Z),
            'Ž' => Ok(CzechLetter::Ž),
            other => Err(CzechLetterError::InvalidChar(other)),
        }
    }

    /// Parses a single token into a `CzechLetter`.
    ///
    /// Handles both single characters (e.g. "A", "á", "č") and the digraph "CH" / "ch" / "Ch".
    pub fn from_token(s: &str) -> Result<Self, CzechLetterError> {
        let norm = normalize_czech_text(s.trim());
        if norm.is_empty() {
            return Err(CzechLetterError::Empty);
        }
        if norm == "CH" {
            return Ok(CzechLetter::Ch);
        }
        let chars: Vec<char> = norm.chars().collect();
        if chars.len() == 1 {
            Self::from_char(chars[0])
        } else {
            Err(CzechLetterError::InvalidLetter(s.to_string()))
        }
    }

    /// Parses a full Czech word into a sequence of `CzechLetter` tokens.
    ///
    /// Handles NFC/NFD normalization, uppercase conversion, and greedily parses "CH" as a single token.
    ///
    /// # Example
    /// ```
    /// use crossword_core::CzechLetter;
    /// let letters = CzechLetter::parse_word("PLACHTA").unwrap();
    /// assert_eq!(letters.len(), 6); // P, L, A, CH, T, A
    /// assert_eq!(letters[3], CzechLetter::Ch);
    /// ```
    pub fn parse_word(s: &str) -> Result<Vec<Self>, CzechLetterError> {
        let normalized = normalize_czech_text(s.trim());
        let chars: Vec<char> = normalized.chars().collect();
        let mut letters = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == 'C' && i + 1 < chars.len() && chars[i + 1] == 'H' {
                letters.push(CzechLetter::Ch);
                i += 2;
            } else {
                letters.push(Self::from_char(chars[i])?);
                i += 1;
            }
        }
        Ok(letters)
    }

    /// Formats a sequence of Czech letters into an uppercase string.
    pub fn format_word(letters: &[Self]) -> String {
        let mut res = String::new();
        for l in letters {
            res.push_str(l.to_str());
        }
        res
    }
}

/// Normalizes Czech text to uppercase NFC equivalent, composing any decomposed diacritical marks.
pub fn normalize_czech_text(input: &str) -> String {
    let composed = compose_czech_combining_marks(input);
    composed.to_uppercase()
}

/// Composes combining diacritical marks for Czech letters (acute, caron, ring above) onto base characters.
fn compose_czech_combining_marks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(&comb) = chars.peek() {
            let composed = match (ch, comb) {
                // Combining acute accent: \u{0301}
                ('A' | 'a', '\u{0301}') => Some(if ch.is_uppercase() { 'Á' } else { 'á' }),
                ('E' | 'e', '\u{0301}') => Some(if ch.is_uppercase() { 'É' } else { 'é' }),
                ('I' | 'i', '\u{0301}') => Some(if ch.is_uppercase() { 'Í' } else { 'í' }),
                ('O' | 'o', '\u{0301}') => Some(if ch.is_uppercase() { 'Ó' } else { 'ó' }),
                ('U' | 'u', '\u{0301}') => Some(if ch.is_uppercase() { 'Ú' } else { 'ú' }),
                ('Y' | 'y', '\u{0301}') => Some(if ch.is_uppercase() { 'Ý' } else { 'ý' }),

                // Combining caron (háček): \u{030C}
                ('C' | 'c', '\u{030C}') => Some(if ch.is_uppercase() { 'Č' } else { 'č' }),
                ('D' | 'd', '\u{030C}') => Some(if ch.is_uppercase() { 'Ď' } else { 'ď' }),
                ('E' | 'e', '\u{030C}') => Some(if ch.is_uppercase() { 'Ě' } else { 'ě' }),
                ('N' | 'n', '\u{030C}') => Some(if ch.is_uppercase() { 'Ň' } else { 'ň' }),
                ('R' | 'r', '\u{030C}') => Some(if ch.is_uppercase() { 'Ř' } else { 'ř' }),
                ('S' | 's', '\u{030C}') => Some(if ch.is_uppercase() { 'Š' } else { 'š' }),
                ('T' | 't', '\u{030C}') => Some(if ch.is_uppercase() { 'Ť' } else { 'ť' }),
                ('Z' | 'z', '\u{030C}') => Some(if ch.is_uppercase() { 'Ž' } else { 'ž' }),

                // Combining ring above (kroužek): \u{030A}
                ('U' | 'u', '\u{030A}') => Some(if ch.is_uppercase() { 'Ů' } else { 'ů' }),

                _ => None,
            };
            if let Some(c) = composed {
                out.push(c);
                chars.next();
                continue;
            }
        }
        out.push(ch);
    }
    out
}

impl fmt::Display for CzechLetter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl FromStr for CzechLetter {
    type Err = CzechLetterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_token(s)
    }
}

impl TryFrom<&str> for CzechLetter {
    type Error = CzechLetterError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::from_token(s)
    }
}

impl TryFrom<char> for CzechLetter {
    type Error = CzechLetterError;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        Self::from_char(c)
    }
}
