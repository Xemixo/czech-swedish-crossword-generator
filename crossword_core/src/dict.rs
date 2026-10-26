//! Dictionary loader & Trie/DAWG indexer.
//!
//! Provides:
//! - `DictEntry`: tokenized word (`Vec<CzechLetter>`) and clue string.
//! - `DictLoader`: parser for UTF-8 TSV and pipe-separated dictionaries.
//! - `SlotRequirement`: exact letter constraint or wildcard.
//! - `Trie` and `TrieNode`: 42-ary level-indexed array Trie for pattern matching.
//! - `LengthIndex`: length-partitioned bitset index for fast constraint satisfaction.
//! - `Dictionary`: unified container with entries, Trie, and LengthIndex.

use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use bitvec::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use thiserror::Error;

use crate::letter::{normalize_czech_text, CzechLetter, CzechLetterError};

/// Errors encountered while loading or querying dictionaries.
#[derive(Error, Debug)]
pub enum DictError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid dictionary line at line {line_num}: '{line}'")]
    InvalidLine { line_num: usize, line: String },

    #[error("Empty word at line {line_num}")]
    EmptyWord { line_num: usize },

    #[error("Invalid Czech word at line {line_num} ('{word}'): {source}")]
    InvalidWord {
        line_num: usize,
        word: String,
        source: CzechLetterError,
    },
}

/// Definition difficulty level for crossword words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Difficulty {
    Easy = 1,
    Medium = 2,
    Hard = 3,
}

impl Difficulty {
    /// Parses difficulty from string ("easy", "medium", "hard", "snadna", "stredni", "tezka", etc.).
    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.trim().to_lowercase();
        match lower.as_str() {
            "easy" | "snadna" | "snadná" | "lehka" | "lehká" | "1" => Some(Difficulty::Easy),
            "medium" | "stredni" | "střední" | "normal" | "2" => Some(Difficulty::Medium),
            "hard" | "tezka" | "těžká" | "obtizna" | "obtížná" | "3" => Some(Difficulty::Hard),
            _ => None,
        }
    }

    /// Internal compact 1-based code (1=Easy, 2=Medium, 3=Hard).
    #[inline]
    pub fn code(&self) -> u8 {
        *self as u8
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
        }
    }

    pub fn display_cs(&self) -> &'static str {
        match self {
            Difficulty::Easy => "Snadná",
            Difficulty::Medium => "Střední",
            Difficulty::Hard => "Těžká",
        }
    }
}

/// User's choice of crossword difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DifficultyChoice {
    Easy,
    Medium,
    Hard,
    All,
}

impl DifficultyChoice {
    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.trim().to_lowercase();
        match lower.as_str() {
            "easy" | "snadna" | "snadná" | "lehka" | "lehká" | "1" => Some(DifficultyChoice::Easy),
            "medium" | "stredni" | "střední" | "normal" | "2" => Some(DifficultyChoice::Medium),
            "hard" | "tezka" | "těžká" | "obtizna" | "obtížná" | "3" => Some(DifficultyChoice::Hard),
            "all" | "any" | "mix" | "vse" | "vše" | "libovolna" | "libovolná" | "4" => Some(DifficultyChoice::All),
            _ => None,
        }
    }

    pub fn target_difficulty(&self) -> Option<Difficulty> {
        match self {
            DifficultyChoice::Easy => Some(Difficulty::Easy),
            DifficultyChoice::Medium => Some(Difficulty::Medium),
            DifficultyChoice::Hard => Some(Difficulty::Hard),
            DifficultyChoice::All => None,
        }
    }

    pub fn display_cs(&self) -> &'static str {
        match self {
            DifficultyChoice::Easy => "Snadná",
            DifficultyChoice::Medium => "Střední",
            DifficultyChoice::Hard => "Těžká",
            DifficultyChoice::All => "Libovolná / Mix",
        }
    }
}

/// A single entry in the crossword dictionary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictEntry {
    /// The word tokenized into Czech crossword letters (CH is a single token).
    pub word: Vec<CzechLetter>,
    /// The clue / definition text associated with this word.
    pub clue: String,
    /// Editorial difficulty level (Easy, Medium, Hard), if known.
    pub difficulty: Option<Difficulty>,
}

impl DictEntry {
    /// Creates a new `DictEntry` without explicit difficulty.
    pub fn new(word: Vec<CzechLetter>, clue: impl Into<String>) -> Self {
        Self {
            word,
            clue: clue.into(),
            difficulty: None,
        }
    }

    /// Creates a new `DictEntry` with explicit difficulty.
    pub fn with_difficulty(
        word: Vec<CzechLetter>,
        clue: impl Into<String>,
        difficulty: Option<Difficulty>,
    ) -> Self {
        Self {
            word,
            clue: clue.into(),
            difficulty,
        }
    }

    /// Formats the word back to its uppercase Czech string representation.
    pub fn word_str(&self) -> String {
        CzechLetter::format_word(&self.word)
    }

    /// Returns the number of letter cells this word occupies (CH counts as 1).
    pub fn len(&self) -> usize {
        self.word.len()
    }

    /// Returns true if this entry has zero letters.
    pub fn is_empty(&self) -> bool {
        self.word.is_empty()
    }
}

/// Slot requirement for pattern matching: either an exact letter or a wildcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotRequirement {
    /// Exact match against a specific Czech letter.
    Exact(CzechLetter),
    /// Wildcard matching any Czech letter.
    Wildcard,
}

impl SlotRequirement {
    /// Checks whether this requirement matches a given Czech letter.
    #[inline]
    pub fn matches(&self, letter: CzechLetter) -> bool {
        match self {
            SlotRequirement::Exact(l) => *l == letter,
            SlotRequirement::Wildcard => true,
        }
    }

    /// Parses a pattern string where '?', '_', '.', or '*' represent wildcards,
    /// and other characters represent Czech letters (with "CH" greedily parsed as a single token).
    pub fn parse_pattern(s: &str) -> Result<Vec<Self>, CzechLetterError> {
        let normalized = normalize_czech_text(s.trim());
        let chars: Vec<char> = normalized.chars().collect();
        let mut reqs = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '?' || c == '_' || c == '.' || c == '*' {
                reqs.push(SlotRequirement::Wildcard);
                i += 1;
            } else if c == 'C' && i + 1 < chars.len() && chars[i + 1] == 'H' {
                reqs.push(SlotRequirement::Exact(CzechLetter::Ch));
                i += 2;
            } else {
                let letter = CzechLetter::from_char(c)?;
                reqs.push(SlotRequirement::Exact(letter));
                i += 1;
            }
        }
        Ok(reqs)
    }
}

impl From<CzechLetter> for SlotRequirement {
    #[inline]
    fn from(l: CzechLetter) -> Self {
        SlotRequirement::Exact(l)
    }
}

impl From<Option<CzechLetter>> for SlotRequirement {
    #[inline]
    fn from(opt: Option<CzechLetter>) -> Self {
        match opt {
            Some(l) => SlotRequirement::Exact(l),
            None => SlotRequirement::Wildcard,
        }
    }
}

impl fmt::Display for SlotRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotRequirement::Exact(l) => write!(f, "{}", l),
            SlotRequirement::Wildcard => write!(f, "?"),
        }
    }
}

fn parse_line_tokens(line: &str) -> Option<(&str, Option<Difficulty>, &str)> {
    if line.contains('\t') {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let diff = Difficulty::parse(parts[1]);
            if diff.is_some() {
                let first_tab = line.find('\t')?;
                let rest = &line[first_tab + 1..];
                let second_tab = rest.find('\t')?;
                Some((parts[0].trim(), diff, rest[second_tab + 1..].trim()))
            } else {
                let first_tab = line.find('\t')?;
                Some((parts[0].trim(), None, line[first_tab + 1..].trim()))
            }
        } else if parts.len() == 2 {
            Some((parts[0].trim(), None, parts[1].trim()))
        } else {
            None
        }
    } else if let Some(idx1) = line.find('|') {
        let word_part = line[..idx1].trim();
        let rest = line[idx1 + 1..].trim();
        if let Some(idx2) = rest.find('|') {
            let diff = Difficulty::parse(&rest[..idx2]);
            if diff.is_some() {
                Some((word_part, diff, rest[idx2 + 1..].trim()))
            } else {
                Some((word_part, None, rest))
            }
        } else {
            Some((word_part, None, rest))
        }
    } else {
        None
    }
}

/// Loader for UTF-8 TSV and pipe-delimited dictionaries.
pub struct DictLoader;

impl DictLoader {
    /// Loads dictionary entries from a TSV or pipe-delimited text file.
    pub fn from_tsv<P: AsRef<Path>>(path: P) -> Result<Vec<DictEntry>, DictError> {
        let file = File::open(path)?;
        Self::from_reader(BufReader::new(file))
    }

    /// Loads dictionary entries from any `BufRead` source.
    ///
    /// Accepts lines formatted as:
    /// - `WORD<TAB>LEVEL<TAB>CLUE` (word_difficulty.tsv)
    /// - `WORD<TAB>CLUE` (standard 2-column TSV)
    /// - `WORD|LEVEL|CLUE` or `WORD|CLUE`
    ///
    /// Ignores empty lines and lines starting with `#` (comments).
    pub fn from_reader<R: BufRead>(reader: R) -> Result<Vec<DictEntry>, DictError> {
        let mut entries = Vec::new();

        for (line_idx, line_res) in reader.lines().enumerate() {
            let raw_line = line_res?;
            let line_num = line_idx + 1;
            let line = raw_line.trim_start_matches('\u{feff}');
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let (word_str, difficulty, clue_str) = parse_line_tokens(line).ok_or_else(|| {
                DictError::InvalidLine {
                    line_num,
                    line: raw_line.clone(),
                }
            })?;

            if word_str.is_empty() {
                return Err(DictError::EmptyWord { line_num });
            }

            let word = CzechLetter::parse_word(word_str).map_err(|e| DictError::InvalidWord {
                line_num,
                word: word_str.to_string(),
                source: e,
            })?;

            entries.push(DictEntry::with_difficulty(word, clue_str, difficulty));
        }

        Ok(entries)
    }

    /// Loads dictionary entries from a TSV or pipe-delimited text file, skipping malformed lines.
    pub fn from_tsv_lenient<P: AsRef<Path>>(path: P) -> Result<Vec<DictEntry>, DictError> {
        let file = File::open(path)?;
        Self::from_reader_lenient(BufReader::new(file))
    }

    /// Loads dictionary entries from any `BufRead` source, skipping malformed lines or invalid words.
    pub fn from_reader_lenient<R: BufRead>(reader: R) -> Result<Vec<DictEntry>, DictError> {
        let mut entries = Vec::new();

        for line_res in reader.lines() {
            let raw_line = match line_res {
                Ok(l) => l,
                Err(_) => continue,
            };
            let line = raw_line.trim_start_matches('\u{feff}');
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let (word_str, difficulty, clue_str) = match parse_line_tokens(line) {
                Some(tokens) => tokens,
                None => continue,
            };

            if word_str.is_empty() {
                continue;
            }

            if let Ok(word) = CzechLetter::parse_word(word_str) {
                entries.push(DictEntry::with_difficulty(word, clue_str, difficulty));
            }
        }

        Ok(entries)
    }
}

/// A node in the level-indexed array Trie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrieNode {
    /// 42 child pointers, indexed by `CzechLetter as u8`.
    pub children: [Option<u32>; CzechLetter::COUNT],
    /// True if this node represents the end of a valid word.
    pub is_terminal: bool,
    /// IDs of dictionary entries terminating at this node (supports multiple clues per word).
    pub entry_ids: SmallVec<[u32; 2]>,
}

impl Default for TrieNode {
    fn default() -> Self {
        Self {
            children: [None; CzechLetter::COUNT],
            is_terminal: false,
            entry_ids: SmallVec::new(),
        }
    }
}

/// Level-indexed array Trie where nodes have 42 child pointers (one for each `CzechLetter`).
#[derive(Debug, Clone)]
pub struct Trie {
    /// Flat pool of Trie nodes; `nodes[0]` is always the root.
    pub nodes: Vec<TrieNode>,
}

impl Default for Trie {
    fn default() -> Self {
        Self::new()
    }
}

impl Trie {
    /// Creates an empty Trie containing only the root node.
    pub fn new() -> Self {
        let mut trie = Self { nodes: Vec::new() };
        trie.nodes.push(TrieNode::default());
        trie
    }

    /// Constructs a Trie from an existing slice of dictionary entries.
    pub fn from_entries(entries: &[DictEntry]) -> Self {
        let mut trie = Self::new();
        for (id, entry) in entries.iter().enumerate() {
            trie.insert(&entry.word, id as u32);
        }
        trie
    }

    /// Inserts a tokenized word and its associated dictionary entry ID into the Trie.
    pub fn insert(&mut self, word: &[CzechLetter], entry_id: u32) {
        let mut curr = 0;
        for &letter in word {
            let idx = letter.as_u8() as usize;
            let next = match self.nodes[curr].children[idx] {
                Some(child_idx) => child_idx as usize,
                None => {
                    let new_idx = self.nodes.len();
                    self.nodes.push(TrieNode::default());
                    self.nodes[curr].children[idx] = Some(new_idx as u32);
                    new_idx
                }
            };
            curr = next;
        }
        self.nodes[curr].is_terminal = true;
        if !self.nodes[curr].entry_ids.contains(&entry_id) {
            self.nodes[curr].entry_ids.push(entry_id);
        }
    }

    /// Total number of nodes in the Trie.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the entry IDs associated with the exact word, if it exists in the Trie.
    pub fn get(&self, word: &[CzechLetter]) -> Option<&[u32]> {
        let mut curr = 0;
        for &letter in word {
            let idx = letter.as_u8() as usize;
            curr = self.nodes[curr].children[idx]? as usize;
        }
        let node = &self.nodes[curr];
        if node.is_terminal {
            Some(&node.entry_ids)
        } else {
            None
        }
    }

    /// Returns true if the exact word exists and is terminal in the Trie.
    #[inline]
    pub fn contains(&self, word: &[CzechLetter]) -> bool {
        self.get(word).is_some()
    }

    /// Searches for words matching a slot pattern of exact letters and wildcards.
    ///
    /// The search requires exact word length matching `pattern.len()`.
    pub fn find_matches(&self, pattern: &[SlotRequirement]) -> Vec<u32> {
        let mut results = Vec::new();
        self.find_matches_into(pattern, &mut results);
        results
    }

    /// Searches for words matching `pattern` and writes matching entry IDs into `results`.
    ///
    /// Reuses the `results` vector to minimize heap allocations.
    pub fn find_matches_into(&self, pattern: &[SlotRequirement], results: &mut Vec<u32>) {
        results.clear();
        if self.nodes.is_empty() {
            return;
        }
        self.find_matches_recursive(0, 0, pattern, results);
    }

    fn find_matches_recursive(
        &self,
        node_idx: usize,
        depth: usize,
        pattern: &[SlotRequirement],
        results: &mut Vec<u32>,
    ) {
        if depth == pattern.len() {
            let node = &self.nodes[node_idx];
            if node.is_terminal {
                results.extend_from_slice(&node.entry_ids);
            }
            return;
        }

        let node = &self.nodes[node_idx];
        match pattern[depth] {
            SlotRequirement::Exact(letter) => {
                let idx = letter.as_u8() as usize;
                if let Some(child_idx) = node.children[idx] {
                    self.find_matches_recursive(child_idx as usize, depth + 1, pattern, results);
                }
            }
            SlotRequirement::Wildcard => {
                for child in &node.children {
                    if let Some(child_idx) = *child {
                        self.find_matches_recursive(child_idx as usize, depth + 1, pattern, results);
                    }
                }
            }
        }
    }

    /// Finds all entry IDs for words starting with the specified letter prefix.
    pub fn find_prefix_matches(&self, prefix: &[CzechLetter]) -> Vec<u32> {
        if self.nodes.is_empty() {
            return Vec::new();
        }
        let mut curr = 0;
        for &letter in prefix {
            let idx = letter.as_u8() as usize;
            match self.nodes[curr].children[idx] {
                Some(child_idx) => curr = child_idx as usize,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        self.collect_all_terminal(curr, &mut results);
        results
    }

    fn collect_all_terminal(&self, node_idx: usize, results: &mut Vec<u32>) {
        let node = &self.nodes[node_idx];
        if node.is_terminal {
            results.extend_from_slice(&node.entry_ids);
        }
        for child in &node.children {
            if let Some(child_idx) = *child {
                self.collect_all_terminal(child_idx as usize, results);
            }
        }
    }
}

/// Length-partitioned bitset index for fast constraint queries.
#[derive(Debug, Clone)]
pub struct LengthIndex {
    total_entries: usize,
    /// Lists of entry IDs grouped by word length: `entries_by_length[len]`.
    entries_by_length: Vec<Vec<u32>>,
    /// Bitset of all entries having word length `len`: `all_by_length[len]`.
    all_by_length: Vec<BitVec<usize, Lsb0>>,
    /// Positional bitsets: `pos_letter[len][pos][letter_idx]`.
    pos_letter: Vec<Vec<[Option<BitVec<usize, Lsb0>>; CzechLetter::COUNT]>>,
}

impl LengthIndex {
    /// Builds a `LengthIndex` from dictionary entries.
    pub fn new(entries: &[DictEntry]) -> Self {
        let total_entries = entries.len();
        let max_len = entries.iter().map(|e| e.word.len()).max().unwrap_or(0);

        let mut entries_by_length = vec![Vec::new(); max_len + 1];
        let mut all_by_length = vec![BitVec::repeat(false, total_entries); max_len + 1];

        let mut pos_letter = Vec::with_capacity(max_len + 1);
        for len in 0..=max_len {
            let mut pos_vec = Vec::with_capacity(len);
            for _ in 0..len {
                pos_vec.push(std::array::from_fn(|_| None));
            }
            pos_letter.push(pos_vec);
        }

        for (id, entry) in entries.iter().enumerate() {
            let len = entry.word.len();
            entries_by_length[len].push(id as u32);
            all_by_length[len].set(id, true);

            for (pos, &letter) in entry.word.iter().enumerate() {
                let letter_idx = letter.as_u8() as usize;
                let slot = &mut pos_letter[len][pos][letter_idx];
                if slot.is_none() {
                    let mut bv = BitVec::repeat(false, total_entries);
                    bv.set(id, true);
                    *slot = Some(bv);
                } else {
                    slot.as_mut().unwrap().set(id, true);
                }
            }
        }

        Self {
            total_entries,
            entries_by_length,
            all_by_length,
            pos_letter,
        }
    }

    /// Total number of indexed entries.
    #[inline]
    pub fn total_entries(&self) -> usize {
        self.total_entries
    }

    /// Maximum word length observed in the index.
    #[inline]
    pub fn max_length(&self) -> usize {
        self.entries_by_length.len().saturating_sub(1)
    }

    /// Returns a slice of entry IDs for all words of the given exact length.
    ///
    /// This is an $O(1)$ zero-allocation operation.
    #[inline]
    pub fn words_of_length(&self, length: usize) -> &[u32] {
        if length < self.entries_by_length.len() {
            &self.entries_by_length[length]
        } else {
            &[]
        }
    }

    /// Returns the number of words of the given exact length.
    #[inline]
    pub fn count_of_length(&self, length: usize) -> usize {
        self.words_of_length(length).len()
    }

    /// Queries entries of exact `length` satisfying all `(position, letter)` constraints,
    /// writing the matching bitset into `out`.
    ///
    /// Reuses caller's `out` allocation to avoid heap allocations on critical query paths.
    pub fn query_into(
        &self,
        length: usize,
        constraints: &[(usize, CzechLetter)],
        out: &mut BitVec<usize, Lsb0>,
    ) {
        out.clear();
        out.resize(self.total_entries, false);

        if length == 0 || length >= self.all_by_length.len() {
            return;
        }

        if constraints.is_empty() {
            out.clone_from(&self.all_by_length[length]);
            return;
        }

        // Quick check: if any constraint is out of bounds or has 0 matching words, return empty
        for &(pos, letter) in constraints {
            if pos >= length {
                return;
            }
            let letter_idx = letter.as_u8() as usize;
            if self.pos_letter[length][pos][letter_idx].is_none() {
                return;
            }
        }

        // Initialize with the bitset of the first constraint
        let (first_pos, first_letter) = constraints[0];
        let first_bv = self.pos_letter[length][first_pos][first_letter.as_u8() as usize]
            .as_ref()
            .unwrap();
        out.clone_from(first_bv);

        // Bitwise AND with subsequent constraints
        for &(pos, letter) in &constraints[1..] {
            let bv = self.pos_letter[length][pos][letter.as_u8() as usize]
                .as_ref()
                .unwrap();
            *out &= bv;
            if out.not_any() {
                break;
            }
        }
    }

    /// Queries entries of exact `length` satisfying all `(position, letter)` constraints simultaneously.
    pub fn query(
        &self,
        length: usize,
        constraints: &[(usize, CzechLetter)],
    ) -> BitVec<usize, Lsb0> {
        let mut out = BitVec::repeat(false, self.total_entries);
        self.query_into(length, constraints, &mut out);
        out
    }

    /// Queries entries of exact `length` and returns a list of matching entry IDs.
    pub fn query_entry_ids(
        &self,
        length: usize,
        constraints: &[(usize, CzechLetter)],
    ) -> Vec<u32> {
        let bv = self.query(length, constraints);
        bv.iter_ones().map(|idx| idx as u32).collect()
    }
}

/// A complete crossword dictionary combining entries, Trie, and length-partitioned index.
#[derive(Debug, Clone)]
pub struct Dictionary {
    /// All dictionary entries.
    pub entries: Vec<DictEntry>,
    /// Trie for prefix and slot pattern lookups.
    pub trie: Trie,
    /// Length-partitioned bitset index for CSP domain propagation.
    pub index: LengthIndex,
    /// Precomputed compact difficulty codes: 1=Easy, 2=Medium, 3=Hard, 0=None.
    pub compact_difficulties: Vec<u8>,
    /// Precomputed flat word letter bytes (up to 24 Czech letters) for zero-indirection solver access.
    pub flat_words: Vec<[u8; 24]>,
}

impl Dictionary {
    /// Builds a new `Dictionary` from a vector of entries.
    pub fn new(entries: Vec<DictEntry>) -> Self {
        let trie = Trie::from_entries(&entries);
        let index = LengthIndex::new(&entries);
        let mut compact_difficulties = Vec::with_capacity(entries.len());
        let mut flat_words = Vec::with_capacity(entries.len());
        for e in &entries {
            compact_difficulties.push(e.difficulty.map(|d| d.code()).unwrap_or(0));
            let mut arr = [255u8; 24];
            for (i, &l) in e.word.iter().take(24).enumerate() {
                arr[i] = l.as_u8();
            }
            flat_words.push(arr);
        }
        Self {
            entries,
            trie,
            index,
            compact_difficulties,
            flat_words,
        }
    }

    /// Returns counts: (easy, medium, hard, none)
    pub fn difficulty_counts(&self) -> (usize, usize, usize, usize) {
        let mut easy = 0;
        let mut medium = 0;
        let mut hard = 0;
        let mut none = 0;
        for &d in &self.compact_difficulties {
            match d {
                1 => easy += 1,
                2 => medium += 1,
                3 => hard += 1,
                _ => none += 1,
            }
        }
        (easy, medium, hard, none)
    }

    /// Loads a dictionary from a TSV or pipe-delimited file.
    pub fn from_tsv<P: AsRef<Path>>(path: P) -> Result<Self, DictError> {
        let entries = DictLoader::from_tsv(path)?;
        Ok(Self::new(entries))
    }

    /// Loads a dictionary from file using lenient parsing (gracefully skips malformed lines or words).
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, DictError> {
        let entries = DictLoader::from_tsv_lenient(path)?;
        Ok(Self::new(entries))
    }

    /// Loads a dictionary from a reader.
    pub fn from_reader<R: BufRead>(reader: R) -> Result<Self, DictError> {
        let entries = DictLoader::from_reader(reader)?;
        Ok(Self::new(entries))
    }

    /// Retrieves a dictionary entry by its ID.
    #[inline]
    pub fn entry(&self, id: u32) -> Option<&DictEntry> {
        self.entries.get(id as usize)
    }

    /// Number of entries in the dictionary.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the dictionary has zero entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all entry IDs of the given exact word length.
    #[inline]
    pub fn words_of_length(&self, length: usize) -> &[u32] {
        self.index.words_of_length(length)
    }

    /// Pattern match search via the Trie.
    #[inline]
    pub fn find_matches(&self, pattern: &[SlotRequirement]) -> Vec<u32> {
        self.trie.find_matches(pattern)
    }

    /// Constraint query via the `LengthIndex`.
    #[inline]
    pub fn query_constraints(
        &self,
        length: usize,
        constraints: &[(usize, CzechLetter)],
    ) -> Vec<u32> {
        self.index.query_entry_ids(length, constraints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SAMPLE_TSV: &str = "\
# In-memory test dictionary with comments and whitespace
AUTO\tOsobní motorové vozidlo
PES\tDomácí čtyřnohý mazlíček
KOČKA|Domácí šelma
CHATA\tVíkendové stavení
CHLEB|Základní pečivo
LES\tHustý porost stromů
OKO|Zrakový orgán
VODA\tČirá životodárná kapalina
KACHNA|Vodní pták
PRAHA\tHlavní město ČR
STŮL|Čtyřnohý kus nábytku
PLACHTA\tLátka na lodi

# Duplicate word with different clue
AUTO\tSamohyb
";

    #[test]
    fn test_dict_loader_in_memory() {
        let cursor = Cursor::new(SAMPLE_TSV);
        let entries = DictLoader::from_reader(cursor).expect("Failed to load sample TSV");

        // 13 valid lines (including 1 duplicate word with different clue)
        assert_eq!(entries.len(), 13);

        // Verify AUTO entry 0
        assert_eq!(entries[0].word_str(), "AUTO");
        assert_eq!(entries[0].clue, "Osobní motorové vozidlo");
        assert_eq!(entries[0].len(), 4);

        // Verify CHATA has 4 letters with CH as single letter
        assert_eq!(entries[3].word_str(), "CHATA");
        assert_eq!(entries[3].word.len(), 4);
        assert_eq!(entries[3].word[0], CzechLetter::Ch);

        // Verify KACHNA has 5 letters with CH as single letter (K, A, CH, N, A)
        let kachna_entry = entries.iter().find(|e| e.word_str() == "KACHNA").unwrap();
        assert_eq!(kachna_entry.word.len(), 5);
        assert_eq!(
            kachna_entry.word,
            vec![
                CzechLetter::K,
                CzechLetter::A,
                CzechLetter::Ch,
                CzechLetter::N,
                CzechLetter::A
            ]
        );

        // Verify duplicate word
        let auto_entries: Vec<_> = entries.iter().filter(|e| e.word_str() == "AUTO").collect();
        assert_eq!(auto_entries.len(), 2);
    }

    #[test]
    fn test_dict_loader_invalid_lines() {
        // Line without delimiter
        let bad1 = "INVALIDLINE";
        let res = DictLoader::from_reader(Cursor::new(bad1));
        assert!(matches!(res, Err(DictError::InvalidLine { line_num: 1, .. })));

        // Empty word
        let bad2 = "\tClue without word";
        let res = DictLoader::from_reader(Cursor::new(bad2));
        assert!(matches!(res, Err(DictError::EmptyWord { line_num: 1 })));

        // Invalid Czech character
        let bad3 = "WORD123\tNumbers not allowed";
        let res = DictLoader::from_reader(Cursor::new(bad3));
        assert!(matches!(res, Err(DictError::InvalidWord { line_num: 1, .. })));
    }

    #[test]
    fn test_trie_insert_and_exact_lookup() {
        let cursor = Cursor::new(SAMPLE_TSV);
        let entries = DictLoader::from_reader(cursor).unwrap();
        let trie = Trie::from_entries(&entries);

        // Exact match for AUTO returns both entry IDs (0 and 12)
        let auto_word = CzechLetter::parse_word("AUTO").unwrap();
        assert!(trie.contains(&auto_word));
        let ids = trie.get(&auto_word).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids, &[0, 12]);

        // Exact match for KACHNA
        let kachna_word = CzechLetter::parse_word("KACHNA").unwrap();
        assert!(trie.contains(&kachna_word));

        // Non-existent word
        let non_word = CzechLetter::parse_word("BRNO").unwrap();
        assert!(!trie.contains(&non_word));
        assert_eq!(trie.get(&non_word), None);
    }

    #[test]
    fn test_trie_wildcard_lookups() {
        let cursor = Cursor::new(SAMPLE_TSV);
        let entries = DictLoader::from_reader(cursor).unwrap();
        let trie = Trie::from_entries(&entries);

        // 1. Wildcard lookup matching "KACHNA" [K, Wildcard, CH, Wildcard, Wildcard]
        let pattern_kachna = vec![
            SlotRequirement::Exact(CzechLetter::K),
            SlotRequirement::Wildcard,
            SlotRequirement::Exact(CzechLetter::Ch),
            SlotRequirement::Wildcard,
            SlotRequirement::Wildcard,
        ];
        let matches = trie.find_matches(&pattern_kachna);
        assert_eq!(matches.len(), 1);
        assert_eq!(entries[matches[0] as usize].word_str(), "KACHNA");

        // 2. Pattern with exact ends: [K, Wildcard, CH, Wildcard, A]
        let pattern_kachna2 = vec![
            SlotRequirement::Exact(CzechLetter::K),
            SlotRequirement::Wildcard,
            SlotRequirement::Exact(CzechLetter::Ch),
            SlotRequirement::Wildcard,
            SlotRequirement::Exact(CzechLetter::A),
        ];
        let matches2 = trie.find_matches(&pattern_kachna2);
        assert_eq!(matches2.len(), 1);
        assert_eq!(entries[matches2[0] as usize].word_str(), "KACHNA");

        // 3. Pattern matching 4-letter words with CH: [CH, Wildcard, T, Wildcard] -> CHATA
        let pattern_chata = vec![
            SlotRequirement::Exact(CzechLetter::Ch),
            SlotRequirement::Wildcard,
            SlotRequirement::Exact(CzechLetter::T),
            SlotRequirement::Wildcard,
        ];
        let matches_chata = trie.find_matches(&pattern_chata);
        assert_eq!(matches_chata.len(), 1);
        assert_eq!(entries[matches_chata[0] as usize].word_str(), "CHATA");

        // 4. Wildcard matching all 3-letter words: PES, LES, OKO
        let pattern_3 = vec![
            SlotRequirement::Wildcard,
            SlotRequirement::Wildcard,
            SlotRequirement::Wildcard,
        ];
        let matches_3 = trie.find_matches(&pattern_3);
        let matched_words: Vec<String> = matches_3
            .iter()
            .map(|&id| entries[id as usize].word_str())
            .collect();
        // Trie traversal naturally visits children in CzechLetter order: LES (L), OKO (O), PES (P)
        assert_eq!(matched_words, vec!["LES", "OKO", "PES"]);
    }

    #[test]
    fn test_trie_edge_cases() {
        let cursor = Cursor::new(SAMPLE_TSV);
        let entries = DictLoader::from_reader(cursor).unwrap();
        let trie = Trie::from_entries(&entries);

        // Empty pattern
        let empty_matches = trie.find_matches(&[]);
        assert!(empty_matches.is_empty());

        // Length mismatch: 4-letter pattern [K, ?, CH, ?] does NOT match 5-letter KACHNA
        let pattern_4 = vec![
            SlotRequirement::Exact(CzechLetter::K),
            SlotRequirement::Wildcard,
            SlotRequirement::Exact(CzechLetter::Ch),
            SlotRequirement::Wildcard,
        ];
        let matches_4 = trie.find_matches(&pattern_4);
        assert!(matches_4.is_empty());

        // Pattern matching no existing word
        let pattern_none = vec![
            SlotRequirement::Exact(CzechLetter::X),
            SlotRequirement::Wildcard,
            SlotRequirement::Wildcard,
        ];
        let matches_none = trie.find_matches(&pattern_none);
        assert!(matches_none.is_empty());

        // Prefix match
        let prefix_ch = vec![CzechLetter::Ch];
        let prefix_matches = trie.find_prefix_matches(&prefix_ch);
        let prefix_words: Vec<String> = prefix_matches
            .iter()
            .map(|&id| entries[id as usize].word_str())
            .collect();
        assert!(prefix_words.contains(&"CHATA".to_string()));
        assert!(prefix_words.contains(&"CHLEB".to_string()));
    }

    #[test]
    fn test_length_index() {
        let cursor = Cursor::new(SAMPLE_TSV);
        let entries = DictLoader::from_reader(cursor).unwrap();
        let index = LengthIndex::new(&entries);

        // Words of length 3: PES, LES, OKO
        let len3 = index.words_of_length(3);
        assert_eq!(len3.len(), 3);

        // Query with constraint: length 5, pos 0 is K, pos 2 is CH
        let constraints = vec![(0, CzechLetter::K), (2, CzechLetter::Ch)];
        let matching_ids = index.query_entry_ids(5, &constraints);
        assert_eq!(matching_ids.len(), 1);
        assert_eq!(entries[matching_ids[0] as usize].word_str(), "KACHNA");

        // Query with zero constraints: returns all words of length 4
        let len4_all = index.query_entry_ids(4, &[]);
        assert_eq!(len4_all.len(), index.words_of_length(4).len());

        // Query_into buffer reuse test
        let mut buffer = BitVec::new();
        index.query_into(5, &constraints, &mut buffer);
        let ones: Vec<usize> = buffer.iter_ones().collect();
        assert_eq!(ones.len(), 1);
        assert_eq!(ones[0] as u32, matching_ids[0]);

        // Impossible constraint: pos 0 is Z in length 3 words
        let z_constraints = vec![(0, CzechLetter::Z)];
        assert!(index.query_entry_ids(3, &z_constraints).is_empty());

        // Out of bounds pos
        let oob = vec![(10, CzechLetter::A)];
        assert!(index.query_entry_ids(3, &oob).is_empty());
    }

    #[test]
    fn test_trie_and_length_index_consistency() {
        let cursor = Cursor::new(SAMPLE_TSV);
        let dict = Dictionary::from_reader(cursor).unwrap();

        // Pattern: [?, O, ?, ?] (length 4, pos 1 is O) -> VODA
        let pattern = SlotRequirement::parse_pattern("?O??").unwrap();
        let trie_matches = dict.find_matches(&pattern);

        let constraints = vec![(1, CzechLetter::O)];
        let index_matches = dict.query_constraints(4, &constraints);

        assert_eq!(trie_matches, index_matches);
        assert_eq!(dict.entry(trie_matches[0]).unwrap().word_str(), "VODA");
    }

    #[test]
    fn test_sample_dict_integration() {
        // Find reference/sample_dict.txt whether running from workspace root or crate dir
        let path = if Path::new("reference/sample_dict.txt").exists() {
            Path::new("reference/sample_dict.txt").to_path_buf()
        } else if Path::new("../reference/sample_dict.txt").exists() {
            Path::new("../reference/sample_dict.txt").to_path_buf()
        } else {
            panic!("reference/sample_dict.txt not found");
        };

        let dict = Dictionary::from_tsv(&path).expect("Failed to load reference/sample_dict.txt");
        assert_eq!(dict.len(), 23);

        // Verify KACHNA lookup consistency
        let kachna_pattern = SlotRequirement::parse_pattern("K?CH??").unwrap();
        assert_eq!(kachna_pattern.len(), 5);
        let matches = dict.find_matches(&kachna_pattern);
        assert_eq!(matches.len(), 1);
        assert_eq!(dict.entry(matches[0]).unwrap().word_str(), "KACHNA");

        // Verify LengthIndex consistency for KACHNA
        let constraints = vec![(0, CzechLetter::K), (2, CzechLetter::Ch)];
        let index_matches = dict.query_constraints(5, &constraints);
        assert_eq!(matches, index_matches);

        // Verify PLACHTA (6 letters: P, L, A, CH, T, A)
        let plachta_pattern = SlotRequirement::parse_pattern("PLA?T?").unwrap();
        let plachta_matches = dict.find_matches(&plachta_pattern);
        assert_eq!(plachta_matches.len(), 1);
        assert_eq!(dict.entry(plachta_matches[0]).unwrap().word_str(), "PLACHTA");
    }
}
