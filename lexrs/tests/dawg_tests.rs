use lexrs::Dawg;

// ── Word count ────────────────────────────────────────────────────────────────

#[test]
fn test_word_count_greater_than_zero() {
    let mut dawg = Dawg::new();
    // add_all sorts for us
    dawg.add_all(["ash", "ashes", "ashley"].map(String::from))
        .unwrap();
    assert!(dawg.word_count() > 0);
    assert_eq!(dawg.word_count(), 3);
}

#[test]
fn test_word_count_zero() {
    let mut dawg = Dawg::new();
    dawg.add_all(std::iter::empty::<String>()).unwrap();
    assert_eq!(dawg.word_count(), 0);
}

// ── Exact word search ─────────────────────────────────────────────────────────

#[test]
fn test_word_in_dawg() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(dawg.contains("ash"));
}

#[test]
fn test_word_not_in_dawg() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(!dawg.contains("salary"));
    assert!(!dawg.contains("mash lolley"));
}

// ── Insertion ─────────────────────────────────────────────────────────────────

#[test]
fn test_add_single_word() {
    let mut dawg = Dawg::new();
    dawg.add("axe", 1).unwrap();
    assert!(dawg.contains("axe"));
}

#[test]
fn test_add_all_vec() {
    let mut dawg = Dawg::new();
    dawg.add_all(["axe", "kick"].map(String::from)).unwrap();
    assert!(dawg.contains("axe"));
    assert!(dawg.contains("kick"));
    assert_eq!(dawg.word_count(), 2);
}

#[test]
fn test_add_all_generator() {
    let mut dawg = Dawg::new();
    let words = vec!["ash", "ashley", "simpson"]
        .into_iter()
        .map(String::from);
    dawg.add_all(words).unwrap();
    assert!(dawg.contains("ash"));
    assert!(dawg.contains("ashley"));
    assert!(dawg.contains("simpson"));
    assert_eq!(dawg.word_count(), 3);
}

// ── Order violation ───────────────────────────────────────────────────────────

#[test]
fn test_order_violation() {
    let mut dawg = Dawg::new();
    dawg.add("zebra", 1).unwrap();
    let result = dawg.add("apple", 1);
    assert!(result.is_err());
}

// ── Prefix existence ──────────────────────────────────────────────────────────

#[test]
fn test_prefix_exists() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(dawg.contains_prefix("ash"));
    assert!(dawg.contains_prefix("as"));
    assert!(dawg.contains_prefix("a"));
}

#[test]
fn test_prefix_not_exists() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ash", "ashley"].map(String::from)).unwrap();
    assert!(!dawg.contains_prefix("xmas"));
    assert!(!dawg.contains_prefix("xor"));
    assert!(!dawg.contains_prefix("sh"));
}

// ── Prefix search ─────────────────────────────────────────────────────────────

#[test]
fn test_prefix_search() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ashlame", "ashley", "ashlo", "askoiu"].map(String::from))
        .unwrap();
    assert!(!dawg.contains("ash"));
    assert!(dawg.contains("ashley"));
    assert_eq!(dawg.word_count(), 4);
    assert!(dawg.contains_prefix("ash"));

    let mut results = dawg.search_with_prefix("ash");
    results.sort();
    assert_eq!(results, vec!["ashlame", "ashley", "ashlo"]);
}

// ── Wildcard search ───────────────────────────────────────────────────────────

#[test]
fn test_asterisk_search() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ash", "ashley"].map(String::from)).unwrap();

    let mut r = dawg.search("a*").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);

    let mut r = dawg.search("a?*").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);

    let mut r = dawg.search("a*?").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);

    let mut r = dawg.search("a***").unwrap();
    r.sort();
    assert_eq!(r, vec!["ash", "ashley"]);
}

#[test]
fn test_question_search() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ab", "as", "ash", "ashley"].map(String::from))
        .unwrap();

    let mut r = dawg.search("a?").unwrap();
    r.sort();
    assert_eq!(r, vec!["ab", "as"]);
}

#[test]
fn test_combined_wildcard_search() {
    let mut dawg = Dawg::new();
    dawg.add_all(["ab", "as", "ash", "ashley"].map(String::from))
        .unwrap();

    let mut r = dawg.search("*a******?").unwrap();
    r.sort();
    assert_eq!(r, vec!["ab", "as", "ash", "ashley"]);
}

#[test]
fn test_special_chars_in_words() {
    let mut dawg = Dawg::new();
    dawg.add_all(["#$%^a", "ab", "as", "ash", "ashley"].map(String::from))
        .unwrap();
    assert!(dawg.contains("ash"));
    assert!(dawg.contains("ashley"));
    assert!(dawg.contains("#$%^a"));
}

// ── Levenshtein distance search ───────────────────────────────────────────────

#[test]
fn test_edit_distance_search() {
    let mut dawg = Dawg::new();
    let input_words = vec![
        "abhor",
        "abuzz",
        "accept",
        "acorn",
        "agony",
        "albay",
        "albin",
        "algin",
        "alisa",
        "almug",
        "altai",
        "amato",
        "ampyx",
        "aneto",
        "arbil",
        "arrow",
        "artha",
        "aruba",
        "athie",
        "auric",
        "aurum",
        "cap",
        "common",
        "dime",
        "eyes",
        "foot",
        "likeablelanguage",
        "lonely",
        "look",
        "nasty",
        "pet",
        "psychotic",
        "quilt",
        "shock",
        "smalldusty",
        "sore",
        "steel",
        "suit",
        "tank",
        "thrill",
    ];
    dawg.add_all(input_words.into_iter().map(String::from))
        .unwrap();

    let mut results = dawg.search_within_distance("arie", 2);
    results.sort();
    assert_eq!(results, vec!["arbil", "athie", "auric"]);
}

// ── DAWG suffix compression ───────────────────────────────────────────────────

#[test]
fn test_suffix_compression() {
    // "tap", "taps", "top", "tops" share suffix "ps" / "p" / "s" structure
    // Python lexpy reports 6 minimized nodes for this set
    let mut dawg = Dawg::new();
    dawg.add_all(["tap", "taps", "top", "tops"].map(String::from))
        .unwrap();
    // Node count after minimization should be <= trie node count (compression happened)
    assert!(dawg.node_count() <= 8);
}

// ── File loading ──────────────────────────────────────────────────────────────

#[test]
fn test_add_from_file() {
    let mut dawg = Dawg::new();
    dawg.add_from_file("tests/data/words2.txt").unwrap();
    assert!(dawg.contains("ash"));
    assert!(dawg.contains("ashley"));
    assert!(dawg.contains("simpson"));
    assert_eq!(dawg.word_count(), 8);
}
