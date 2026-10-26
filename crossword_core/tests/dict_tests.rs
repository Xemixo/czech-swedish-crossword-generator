use std::path::Path;
use bitvec::prelude::*;
use crossword_core::{
    CzechLetter, DictLoader, Dictionary, SlotRequirement,
};

fn get_sample_dict_path() -> std::path::PathBuf {
    if Path::new("reference/sample_dict.txt").exists() {
        Path::new("reference/sample_dict.txt").to_path_buf()
    } else if Path::new("../reference/sample_dict.txt").exists() {
        Path::new("../reference/sample_dict.txt").to_path_buf()
    } else {
        panic!("reference/sample_dict.txt could not be found");
    }
}

#[test]
fn test_integration_load_sample_dict() {
    let path = get_sample_dict_path();
    let entries = DictLoader::from_tsv(&path).expect("Failed to load sample dictionary");

    assert_eq!(entries.len(), 23);

    // Verify words with CH
    let ch_words: Vec<String> = entries
        .iter()
        .filter(|e| e.word.contains(&CzechLetter::Ch))
        .map(|e| e.word_str())
        .collect();

    assert!(ch_words.contains(&"CHATA".to_string()));
    assert!(ch_words.contains(&"CHLEB".to_string()));
    assert!(ch_words.contains(&"KACHNA".to_string()));
    assert!(ch_words.contains(&"OCHOTA".to_string()));
    assert!(ch_words.contains(&"PLACHTA".to_string()));
}

#[test]
fn test_integration_trie_queries() {
    let path = get_sample_dict_path();
    let dict = Dictionary::from_tsv(&path).unwrap();

    // 1. Exact match lookup
    let kachna_word = CzechLetter::parse_word("KACHNA").unwrap();
    assert_eq!(kachna_word.len(), 5);
    assert!(dict.trie.contains(&kachna_word));

    // 2. Wildcard lookup: [Exact(K), Wildcard, Exact(Ch), Wildcard, Wildcard] -> KACHNA
    let pattern_kachna = vec![
        SlotRequirement::Exact(CzechLetter::K),
        SlotRequirement::Wildcard,
        SlotRequirement::Exact(CzechLetter::Ch),
        SlotRequirement::Wildcard,
        SlotRequirement::Wildcard,
    ];
    let matches = dict.find_matches(&pattern_kachna);
    assert_eq!(matches.len(), 1);
    let entry = dict.entry(matches[0]).unwrap();
    assert_eq!(entry.word_str(), "KACHNA");
    assert_eq!(entry.clue, "Vodní pták");

    // 3. Length mismatch: 4-letter pattern with [K, Wildcard, CH, Wildcard] should not match 5-letter KACHNA
    let pattern_4 = vec![
        SlotRequirement::Exact(CzechLetter::K),
        SlotRequirement::Wildcard,
        SlotRequirement::Exact(CzechLetter::Ch),
        SlotRequirement::Wildcard,
    ];
    let matches_4 = dict.find_matches(&pattern_4);
    assert!(matches_4.is_empty(), "4-letter pattern must not match 5-letter KACHNA");

    // 4. Wildcard matching CHATA: [Exact(CH), Wildcard, Exact(T), Wildcard]
    let pattern_chata = vec![
        SlotRequirement::Exact(CzechLetter::Ch),
        SlotRequirement::Wildcard,
        SlotRequirement::Exact(CzechLetter::T),
        SlotRequirement::Wildcard,
    ];
    let matches_chata = dict.find_matches(&pattern_chata);
    assert_eq!(matches_chata.len(), 1);
    assert_eq!(dict.entry(matches_chata[0]).unwrap().word_str(), "CHATA");
}

#[test]
fn test_integration_length_index_queries() {
    let path = get_sample_dict_path();
    let dict = Dictionary::from_tsv(&path).unwrap();

    // Word counts by length
    // In sample_dict.txt:
    // Length 3: PES, LES, OKO, DŮM, KŮŇ, DEN, NOC (7 words)
    let len3_words = dict.words_of_length(3);
    assert_eq!(len3_words.len(), 7);

    // Length 5: KOČKA, KACHNA, PRAHA, STROM, OCHOTA (5 words)
    let len5_words = dict.words_of_length(5);
    assert_eq!(len5_words.len(), 5);

    // Query constraint: Length 5, pos 0 = K
    let k_constraints = vec![(0, CzechLetter::K)];
    let k_matches = dict.query_constraints(5, &k_constraints);
    assert_eq!(k_matches.len(), 2); // KOČKA and KACHNA
    let k_words: Vec<String> = k_matches.iter().map(|&id| dict.entry(id).unwrap().word_str()).collect();
    assert!(k_words.contains(&"KOČKA".to_string()));
    assert!(k_words.contains(&"KACHNA".to_string()));

    // Query constraint: Length 5, pos 0 = K AND pos 2 = CH
    let kachna_constraints = vec![(0, CzechLetter::K), (2, CzechLetter::Ch)];
    let kachna_matches = dict.query_constraints(5, &kachna_constraints);
    assert_eq!(kachna_matches.len(), 1);
    assert_eq!(dict.entry(kachna_matches[0]).unwrap().word_str(), "KACHNA");

    // Zero-allocation buffer reuse
    let mut bv = BitVec::new();
    dict.index.query_into(5, &kachna_constraints, &mut bv);
    let matched_ids: Vec<u32> = bv.iter_ones().map(|i| i as u32).collect();
    assert_eq!(matched_ids, kachna_matches);
}

#[test]
fn test_integration_consistency_trie_and_length_index() {
    let path = get_sample_dict_path();
    let dict = Dictionary::from_tsv(&path).unwrap();

    // Test a variety of slot patterns and compare Trie matches to LengthIndex matches
    let test_patterns = [
        "P?A?A",   // PRAHA
        "???",     // all 3-letter words
        "????",    // all 4-letter words
        "?T?L",    // STŮL
        "??A?T?",  // PLACHTA
    ];

    for pat_str in test_patterns {
        let pattern = SlotRequirement::parse_pattern(pat_str).unwrap();
        let trie_results = dict.find_matches(&pattern);

        let mut constraints = Vec::new();
        for (pos, req) in pattern.iter().enumerate() {
            if let SlotRequirement::Exact(letter) = req {
                constraints.push((pos, *letter));
            }
        }
        let index_results = dict.query_constraints(pattern.len(), &constraints);

        let mut trie_sorted = trie_results.clone();
        trie_sorted.sort_unstable();
        let mut index_sorted = index_results.clone();
        index_sorted.sort_unstable();

        assert_eq!(
            trie_sorted, index_sorted,
            "Trie and LengthIndex results must match for pattern '{pat_str}'"
        );
    }
}


