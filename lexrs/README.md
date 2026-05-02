# lexrs — core library

The `lexrs` Rust crate implements two lexicon data structures — **Trie** and **DAWG** — with an optional Python extension module built via [PyO3](https://pyo3.rs).

## Crate layout

```
lexrs/
  src/
    lib.rs      — public re-exports and Python module registration
    trie.rs     — Trie: arena-allocated prefix tree, wildcard & Levenshtein search
    dawg.rs     — DAWG: minimized Trie with suffix compression
    node.rs     — Node type shared by both structures
    utils.rs    — file I/O helpers and wildcard pattern normalization
    error.rs    — LexError enum (used for out-of-order DAWG insertion, etc.)
    python.rs   — PyO3 wrappers (compiled only with --features python)
    bin/
      genome_bench.rs — standalone benchmark binary
  tests/
    trie_tests.rs — Rust unit tests for Trie
    dawg_tests.rs — Rust unit tests for DAWG
```

## Data structures

### Trie

Arena-allocated prefix tree (`Vec<Node>`). Words may be inserted in any order. Good for write-heavy workloads or when insertion order is unknown.

### DAWG (Directed Acyclic Word Graph)

A minimized Trie that merges identical suffix subtrees, yielding a much smaller node count for large lexicons. Words **must be inserted in lexicographic order**. Call `reduce()` after individual `add()` calls to finalize minimization; `add_all()` handles this automatically.

## API

Both types expose the same interface:

| Method | Description |
|---|---|
| `add(word, count)` | Insert a word with an optional frequency count |
| `add_all(words)` | Insert from any iterable (sorts before inserting into DAWG) |
| `add_from_file(path)` | Insert words from a newline-delimited file |
| `contains(word)` | Exact membership test |
| `contains_prefix(prefix)` | True if any stored word starts with this prefix |
| `search(pattern)` | Wildcard search (`*` = any chars, `?` = exactly one) |
| `search_with_count(pattern)` | Like `search`, returns `(word, count)` pairs |
| `search_with_prefix(prefix)` | All words beginning with the prefix |
| `search_with_prefix_count(prefix)` | Like above, with counts |
| `search_within_distance(word, dist)` | Levenshtein fuzzy search |
| `search_within_distance_count(word, dist)` | Like above, with counts |
| `word_count()` | Number of words stored |
| `node_count()` | Number of nodes in the structure |

Consecutive wildcards are normalized: `**` → `*`, `?*` → `*`.

## Install

Add to your `Cargo.toml`:

```toml
[dependencies]
lexrs = "0.1"
```

## Python bindings

The same data structures are available as a Python package:

```bash
pip install pylexrs
```

```python
from lexrs import Trie, DAWG
t = Trie()
t.add_all(["apple", "apply", "apt"])
t.search("ap*")
```

```python
from lexrs import Trie, DAWG

t = Trie()
t.add("hello", 5)
t.add_all(["world", "foo"])
"hello" in t               # True
t.search("h*")             # wildcard
t.search_within_distance("helo", 1)  # fuzzy

d = DAWG()
d.add_all(["apple", "apply", "apt"])
"apple" in d
d.search("ap?")
```

## Cargo features

| Feature | Effect |
|---|---|
| `python` | Enables PyO3 and compiles the `lexrs` Python extension module |
