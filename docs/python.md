# Python Package

`pylexrs` exposes the Rust `Trie` and `DAWG` to Python via [PyO3](https://pyo3.rs). The API mirrors the original [lexpy](https://github.com/aosingh/lexpy) library, so existing code requires minimal changes.

## Install

```bash
pip install pylexrs
```

Wheels are pre-built for Python 3.11–3.14 on Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64). No Rust toolchain required.

---

## Trie

### Create and insert

```python
from lexrs import Trie

t = Trie()

# Insert with an optional frequency count (default: 1)
t.add("apple", 5)
t.add("apply")

# Insert multiple words at once
t.add_all(["apt", "banana", "band", "bandana"])

# Load from a file — one word per line
t.add_from_file("words.txt")
```

### Membership

```python
"apple" in t              # True
"appl"  in t              # False — not a complete word
t.contains_prefix("app")  # True
t.contains_prefix("xyz")  # False
```

### Wildcard search

```python
t.search("ap*")           # ["apple", "apply", "apt"]
t.search("b???")          # ["band"]  — exactly 4 chars
t.search("b*na")          # ["banana", "bandana"]
t.search("*")             # every word in the trie

# Include frequency counts
t.search("ap*", with_count=True)
# [("apple", 5), ("apply", 1), ("apt", 1)]
```

### Prefix completion

```python
t.search_with_prefix("ban")
# ["banana", "band", "bandana"]

t.search_with_prefix("ban", with_count=True)
# [("banana", 1), ("band", 1), ("bandana", 1)]
```

### Levenshtein fuzzy search

```python
t.search_within_distance("aple", 1)
# ["apple"]  — one edit away

t.search_within_distance("bannana", 2)
# ["banana", "bandana"]

t.search_within_distance("aple", 1, with_count=True)
# [("apple", 5)]
```

### Stats and iteration

```python
t.get_word_count()   # sum of all frequencies
len(t)               # number of nodes
```

---

## DAWG

A DAWG compresses shared suffixes on top of shared prefixes, giving a significantly smaller node count for large lexicons. Words must be inserted in sorted order — `add_all()` handles this automatically.

### Create and insert

```python
from lexrs import DAWG

d = DAWG()

# add_all sorts automatically
d.add_all(["apple", "apply", "apt", "banana"])

# add() requires pre-sorted input
d.add("cherry")
```

!!! warning "Sort before calling `add()` directly"
    `add_all()` sorts its input before inserting. If you call `add()` directly, words must arrive in lexicographic order.

### Search

The DAWG exposes the same search interface as the Trie:

```python
"apple" in d                                   # True
d.search("ap*")                                # ["apple", "apply", "apt"]
d.search("ap*", with_count=True)               # [("apple", 1), ...]
d.search_with_prefix("ban")                    # ["banana"]
d.search_within_distance("aple", 1)            # ["apple"]
d.search_within_distance("aple", 1, with_count=True)  # [("apple", 1)]
```

---

## Wildcard syntax

| Pattern | Meaning | Example matches |
|---|---|---|
| `*` | Zero or more characters | `ap*` → apple, apply, apt |
| `?` | Exactly one character | `b??d` → band |
| `h*` | All words starting with `h` | hello, hi, hey |
| `?at` | Three-letter words ending in `at` | bat, cat, hat |
| `a?*` | Words of two or more characters starting with `a` | ap, apple, … |

Consecutive wildcards are normalised: `**` → `*`, `?*` → `*`.

---

## Migrating from lexpy

The only change is the import path. Everything else is identical.

```python
# lexpy 1.x
from lexpy.trie import Trie
from lexpy.dawg import DAWG

# pylexrs
from lexrs import Trie, DAWG
```

---

## API reference

| Method | Description |
|---|---|
| `add(word, count=1)` | Insert a word with an optional frequency count |
| `add_all(words)` | Insert from any iterable; DAWG sorts automatically |
| `add_from_file(path)` | Insert words from a file (one per line) |
| `word in t` | Exact membership test |
| `contains_prefix(prefix)` | `True` if any word starts with the prefix |
| `search(pattern, with_count=False)` | Wildcard search |
| `search_with_prefix(prefix, with_count=False)` | Prefix completion |
| `search_within_distance(word, dist, with_count=False)` | Levenshtein fuzzy search |
| `get_word_count()` | Sum of all word frequencies |
| `len(t)` | Number of nodes |
