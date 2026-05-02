# Rust Library

## Setup

Add `lexrs` to your `Cargo.toml`:

```toml
[dependencies]
lexrs = "0.2"
```

---

## Building a Trie

Start with `Trie::new()`. Words can be added in any order.

```rust
use lexrs::Trie;

let mut trie = Trie::new();
trie.add("apple", 3).unwrap();   // word + frequency count
trie.add("apply", 1).unwrap();
trie.add("apt",   1).unwrap();
trie.add("banana", 5).unwrap();
```

If all your words have the same count, `add_all` is more concise:

```rust
trie.add_all(vec!["apple", "apply", "apt", "banana"]).unwrap();
```

To load a word list from disk (one word per line):

```rust
trie.add_from_file("words.txt").unwrap();
```

---

## Searching

### Exact membership

```rust
trie.contains("apple")      // true
trie.contains("appl")       // false — "appl" is not a complete word
trie.contains_prefix("app") // true  — at least one word starts with "app"
```

### Wildcard search

The wildcard language has two symbols:

- `*` matches **zero or more** characters
- `?` matches **exactly one** character

```rust
trie.search("ap*").unwrap()
// → ["apple", "apply", "apt"]

trie.search("appl?").unwrap()
// → ["apple", "apply"]   — "appl" + exactly one char

trie.search("b???").unwrap()
// → (nothing) — no 4-letter words starting with "b" in our example
//   add "band" and you'd get ["band"]

trie.search("*ana*").unwrap()
// → ["banana"]
```

Consecutive wildcards normalise automatically: `**` becomes `*`, and `?*` becomes `*`.

To retrieve frequency counts alongside words:

```rust
trie.search_with_count("ap*").unwrap()
// → [("apple", 3), ("apply", 1), ("apt", 1)]
```

### Prefix completion

When you know the start of a word and want all completions:

```rust
trie.search_with_prefix("app")
// → ["apple", "apply"]

trie.search_with_prefix_count("app")
// → [("apple", 3), ("apply", 1)]
```

### Levenshtein fuzzy search

Fuzzy search finds words within a given edit distance. An edit is an insertion, deletion, or substitution of a single character.

```rust
trie.search_within_distance("aple", 1)
// → ["apple"]   — "apple" is one insertion away from "aple"

trie.search_within_distance("bananaa", 1)
// → ["banana"]  — one deletion away

trie.search_within_distance("bannana", 2)
// → ["banana"]  — two substitutions
```

With counts:

```rust
trie.search_within_distance_count("aple", 1)
// → [("apple", 3)]
```

---

## Building a DAWG

A DAWG minimises the trie by merging shared suffixes. The words `nation` and `action` both end in `tion` — a DAWG stores that suffix once.

The constraint is that words must be inserted in **lexicographic order**. `add_all` handles this automatically by sorting before inserting:

```rust
use lexrs::Dawg;

let mut dawg = Dawg::new();
dawg.add_all(vec!["apple", "apply", "apt", "banana"]).unwrap();
// add_all sorts the input, then inserts, then calls reduce()
```

If you need to add words one at a time, sort them yourself first and call `reduce()` when done:

```rust
let mut dawg = Dawg::new();
for word in sorted_words {
    dawg.add(word, 1).unwrap();
}
dawg.reduce(); // finalise minimisation — do not skip this
```

!!! warning
    Forgetting `reduce()` after manual insertions leaves the DAWG partially minimised. Search still works, but node count will be higher than optimal.

### Searching a DAWG

The API is identical to the Trie:

```rust
dawg.contains("apple");                       // true
dawg.search("ap*").unwrap();                  // ["apple", "apply", "apt"]
dawg.search_with_prefix("app");               // ["apple", "apply"]
dawg.search_within_distance("aple", 1);       // ["apple"]
dawg.search_within_distance_count("aple", 1); // [("apple", 1)]
```

---

## Inspecting the structure

```rust
trie.word_count()   // sum of all frequency counts
trie.node_count()   // number of nodes allocated
```

These are useful for monitoring memory use after loading a large lexicon.

---

## Error handling

`add` and `search` return `Result`. The error type is `lexrs::LexError`. In practice the main failure case for `search` is a malformed pattern (e.g. an unmatched bracket if you extend the wildcard language).

```rust
match trie.search("ap*") {
    Ok(words)  => println!("{words:?}"),
    Err(e)     => eprintln!("search failed: {e}"),
}
```
