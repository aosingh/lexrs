# lexrs

Efficient lexicon data structures — **Trie** and **DAWG** (Directed Acyclic Word Graph) — backed by Rust for 10–100× faster insertion and search compared to pure Python.

`lexrs` is the successor to [lexpy](https://github.com/aosingh/lexpy). It exposes the same API, so existing code requires minimal changes.

## Install

```bash
pip install lexrs
```

## API

Both `Trie` and `DAWG` expose the same interface:

| Method | Description |
|---|---|
| `add(word, count)` | Insert a word with an optional frequency count |
| `add_all(words)` | Insert from any iterable |
| `add_from_file(path)` | Insert words from a file (one per line) |
| `contains(word)` | Exact membership test |
| `contains_prefix(prefix)` | Check if any word starts with the given prefix |
| `search(pattern)` | Wildcard search (`*` = zero or more chars, `?` = exactly one) |
| `search(pattern, with_count=True)` | Like `search`, but returns `(word, count)` pairs |
| `search_with_prefix(prefix)` | All words beginning with the prefix |
| `search_with_prefix(prefix, with_count=True)` | Like above, with counts |
| `search_within_distance(word, dist)` | Levenshtein fuzzy search |
| `search_within_distance(word, dist, with_count=True)` | Like above, with counts |
| `get_word_count()` | Number of words stored |
| `len(t)` | Number of nodes in the structure |

## Usage

```python
from lexrs import Trie, DAWG

# ── Trie ──────────────────────────────────────────────────────────────────────
t = Trie()
t.add("hello", 5)           # word + optional frequency count
t.add_all(["world", "foo"])
t.add_from_file("words.txt")

"hello" in t                # True
t.contains_prefix("wor")    # True
t.get_word_count()          # total words stored
len(t)                      # total nodes

t.search("h*")                               # wildcard → list of words
t.search("h*", with_count=True)              # → list of (word, count)
t.search_with_prefix("wo")                   # prefix completion
t.search_with_prefix("wo", with_count=True)  # with counts
t.search_within_distance("helo", 1)          # fuzzy, Levenshtein ≤ 1
t.search_within_distance("helo", 1, with_count=True)

# ── DAWG ──────────────────────────────────────────────────────────────────────
# DAWG compresses shared suffixes — fewer nodes for large lexicons.
# Words are sorted automatically by add_all.
d = DAWG()
d.add_all(["apple", "apply", "apt"])

"apple" in d                          # True
d.search("ap*")                       # wildcard
d.search_within_distance("aple", 1)   # fuzzy
```

## Wildcard syntax

| Pattern | Meaning |
|---|---|
| `*` | Zero or more characters |
| `?` | Exactly one character |
| `h*` | All words starting with `h` |
| `?at` | Three-letter words ending in `at` |
| `a?*` | Words of two or more characters starting with `a` |

Consecutive wildcards are normalized (`**` → `*`, `?*` → `*`).

## Migrating from lexpy

```python
# lexpy 1.x
from lexpy.trie import Trie
from lexpy.dawg import DAWG

# lexrs
from lexrs import Trie, DAWG
```

The API is otherwise identical.

## More

Full documentation, the production HTTP server (reader/writer), Docker Compose setup, and benchmarks are available at [github.com/aosingh/lexrs](https://github.com/aosingh/lexrs).
