# Rust Library

The `lexrs` crate provides two data structures — **Trie** and **DAWG** — with an identical API. Both support wildcard search, prefix completion, and Levenshtein fuzzy search.

## Add to your project

```toml
[dependencies]
lexrs = "0.2"
```

---

## Trie

A Trie is a prefix tree that accepts insertions in any order. It is the right choice when you need to add words incrementally or in unsorted order.

### Create and insert

```rust
use lexrs::Trie;

let mut trie = Trie::new();

// Insert a single word with a frequency count
trie.add("apple", 3).unwrap();

// Insert multiple words (count defaults to 1)
trie.add_all(vec!["apply", "apt", "banana", "band"]).unwrap();

// Load from a file — one word per line
trie.add_from_file("words.txt").unwrap();
```

### Membership tests

```rust
trie.contains("apple");         // true
trie.contains("appl");          // false — not a complete word
trie.contains_prefix("app");    // true — some word starts with "app"
trie.contains_prefix("xyz");    // false
```

### Wildcard search

Wildcards use `*` (zero or more characters) and `?` (exactly one character). Consecutive wildcards are normalised (`**` → `*`, `?*` → `*`).

```rust
trie.search("ap*").unwrap();      // ["apple", "apply", "apt"]
trie.search("b???").unwrap();     // ["band"]  — exactly 4 chars starting with b
trie.search("a?*").unwrap();      // all words ≥ 2 chars starting with a
trie.search("*").unwrap();        // every word in the trie
```

To get frequency counts alongside results:

```rust
trie.search_with_count("ap*").unwrap();
// [("apple", 3), ("apply", 1), ("apt", 1)]
```

### Prefix completion

```rust
trie.search_with_prefix("ban");
// ["banana", "band"]

trie.search_with_prefix_count("ban");
// [("banana", 1), ("band", 1)]
```

### Levenshtein fuzzy search

Returns all words within a given edit distance from the query.

```rust
trie.search_within_distance("aple", 1);
// ["apple"]  — one insertion away

trie.search_within_distance("bannana", 2);
// ["banana"] — two edits away

trie.search_within_distance_count("aple", 1);
// [("apple", 3)]
```

### Stats

```rust
trie.word_count();   // total frequency count across all words
trie.node_count();   // number of nodes in the trie
```

---

## DAWG

A DAWG (Directed Acyclic Word Graph) is a minimised Trie that also compresses shared suffixes. For large, stable lexicons this can reduce node count dramatically. The trade-off is that **words must be inserted in lexicographic (sorted) order**, and you must call `reduce()` to finalise minimisation.

### Create and insert

```rust
use lexrs::Dawg;

let mut dawg = Dawg::new();

// add_all sorts automatically before inserting
dawg.add_all(vec!["apple", "apply", "apt", "banana"]).unwrap();

// If you use add() directly, words must arrive sorted
dawg.add("cherry", 1).unwrap();

// Finalise minimisation — required after using add() directly
dawg.reduce();
```

!!! warning "Call `reduce()` after manual insertions"
    If you add words one by one with `add()`, the DAWG is not fully minimised until you call `reduce()`. `add_all()` calls it automatically.

### Search

The DAWG exposes the same search API as the Trie:

```rust
dawg.contains("apple");                          // true
dawg.contains_prefix("app");                     // true
dawg.search("ap*").unwrap();                     // ["apple", "apply", "apt"]
dawg.search_with_prefix("ban");                  // ["banana"]
dawg.search_within_distance("aple", 1);          // ["apple"]
dawg.search_within_distance_count("aple", 1);    // [("apple", 1)]
```

### When to use DAWG vs Trie

| | Trie | DAWG |
|---|---|---|
| Insertion order | Any | Sorted |
| Memory usage | Higher | Lower (shared suffixes compressed) |
| Build time | Faster | Slightly slower (`reduce()`) |
| Search speed | Comparable | Comparable |
| Best for | Live ingestion, delta updates | Large static lexicons, read-heavy |

---

## API reference

| Method | Trie | DAWG | Description |
|---|:---:|:---:|---|
| `add(word, count)` | ✓ | ✓ | Insert one word with a frequency count |
| `add_all(words)` | ✓ | ✓ | Insert from any iterable; DAWG sorts first |
| `add_from_file(path)` | ✓ | ✓ | Insert words from a file (one per line) |
| `reduce()` | — | ✓ | Finalise DAWG minimisation |
| `contains(word)` | ✓ | ✓ | Exact membership test |
| `contains_prefix(prefix)` | ✓ | ✓ | True if any word starts with the prefix |
| `search(pattern)` | ✓ | ✓ | Wildcard search, returns `Vec<String>` |
| `search_with_count(pattern)` | ✓ | ✓ | Wildcard search with counts |
| `search_with_prefix(prefix)` | ✓ | ✓ | All words beginning with prefix |
| `search_with_prefix_count(prefix)` | ✓ | ✓ | Prefix completion with counts |
| `search_within_distance(word, dist)` | ✓ | ✓ | Levenshtein fuzzy search |
| `search_within_distance_count(word, dist)` | ✓ | ✓ | Fuzzy search with counts |
| `word_count()` | ✓ | ✓ | Sum of all word frequencies |
| `node_count()` | ✓ | ✓ | Number of nodes in the structure |
