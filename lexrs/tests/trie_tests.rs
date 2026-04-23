use lexrs::Trie;

// ── Word count ────────────────────────────────────────────────────────────────

#[test]
fn test_word_count_greater_than_zero() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley", "ashes"].map(String::from))
        .unwrap();
    assert!(trie.word_count() > 0);
    assert_eq!(trie.word_count(), 3);
}

#[test]
fn test_word_count_zero() {
    let trie = Trie::new();
    assert_eq!(trie.word_count(), 0);
}

// ── Exact word search ─────────────────────────────────────────────────────────

#[test]
fn test_word_in_trie() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(trie.contains("ash"));
}

#[test]
fn test_word_not_in_trie() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(!trie.contains("salary"));
    assert!(!trie.contains("mash lolley"));
}

// ── Insertion ─────────────────────────────────────────────────────────────────

#[test]
fn test_add_single_word() {
    let mut trie = Trie::new();
    trie.add("axe", 1).unwrap();
    assert!(trie.contains("axe"));
}

#[test]
fn test_add_all_vec() {
    let mut trie = Trie::new();
    trie.add_all(["axe", "kick"].map(String::from)).unwrap();
    assert!(trie.contains("axe"));
    assert!(trie.contains("kick"));
    assert_eq!(trie.word_count(), 2);
}

#[test]
fn test_add_all_generator() {
    let mut trie = Trie::new();
    let words = vec!["ash", "ashley", "simpson"]
        .into_iter()
        .map(String::from);
    trie.add_all(words).unwrap();
    assert!(trie.contains("ash"));
    assert!(trie.contains("ashley"));
    assert!(trie.contains("simpson"));
    assert_eq!(trie.word_count(), 3);
}

#[test]
fn test_add_from_file() {
    let mut trie = Trie::new();
    trie.add_from_file("tests/data/words2.txt").unwrap();
    assert!(trie.contains("ash"));
    assert!(trie.contains("ashley"));
    assert!(trie.contains("simpson"));
    assert_eq!(trie.word_count(), 8);
}

// ── Node count ────────────────────────────────────────────────────────────────

#[test]
fn test_node_count() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley"].map(String::from)).unwrap();
    // root + a + s + h + l + e + y = 7 nodes (including root)
    assert_eq!(trie.node_count(), 7);
}

// ── Prefix existence ──────────────────────────────────────────────────────────

#[test]
fn test_prefix_exists() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(trie.contains_prefix("ash"));
    assert!(trie.contains_prefix("as"));
    assert!(trie.contains_prefix("a"));
}

#[test]
fn test_prefix_not_exists() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(!trie.contains_prefix("xmas"));
    assert!(!trie.contains_prefix("xor"));
    assert!(!trie.contains_prefix("sh"));
}

// ── Prefix search ─────────────────────────────────────────────────────────────

#[test]
fn test_prefix_search() {
    let mut trie = Trie::new();
    trie.add_all(["ashlame", "ashley", "askoiu", "ashlo"].map(String::from))
        .unwrap();
    assert!(!trie.contains("ash"));
    assert!(trie.contains("ashley"));
    assert_eq!(trie.word_count(), 4);
    assert!(trie.contains_prefix("ash"));

    let mut results = trie.search_with_prefix("ash");
    results.sort();
    assert_eq!(results, vec!["ashlame", "ashley", "ashlo"]);
}

// ── Wildcard search ───────────────────────────────────────────────────────────

#[test]
fn test_asterisk_search() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley"].map(String::from)).unwrap();

    let mut r = trie.search("a*").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);

    let mut r = trie.search("a?*").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);

    let mut r = trie.search("a*?").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);

    let mut r = trie.search("a***").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);
}

#[test]
fn test_question_search() {
    let mut trie = Trie::new();
    trie.add_all(["ab", "as", "ash", "ashley"].map(String::from))
        .unwrap();

    let mut r = trie.search("a?").unwrap();
    r.sort();
    assert_eq!(r, vec!["ab", "as"]);
}

#[test]
fn test_combined_wildcard_search() {
    let mut trie = Trie::new();
    trie.add_all(["ab", "as", "ash", "ashley"].map(String::from))
        .unwrap();

    let mut r = trie.search("*a******?").unwrap();
    r.sort();
    assert_eq!(r, vec!["ab", "as", "ash", "ashley"]);
}

#[test]
fn test_special_chars_in_words() {
    let mut trie = Trie::new();
    trie.add_all(["ab", "as", "ash", "ashley", "#$%^a"].map(String::from))
        .unwrap();
    assert!(trie.contains("ash"));
    assert!(trie.contains("ashley"));
    assert!(trie.contains("#$%^a"));
}

// ── Levenshtein distance search ───────────────────────────────────────────────

#[test]
fn test_search_within_distance() {
    let mut trie = Trie::new();
    trie.add_all(["ash", "ashley", "ashe", "sheer"].map(String::from))
        .unwrap();

    // distance 0 — exact
    let mut r = trie.search_within_distance("ash", 0);
    r.sort();
    assert_eq!(r, vec!["ash"]);

    // distance 1 — ash, ashe
    let mut r = trie.search_within_distance("ash", 1);
    r.sort();
    assert_eq!(r, vec!["ash", "ashe"]);
}
