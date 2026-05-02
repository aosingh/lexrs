# Python Package

## Install

```bash
pip install pylexrs
```

Pre-built wheels are available for Python 3.11–3.14 on Linux (x86\_64, aarch64), macOS (x86\_64, Apple Silicon), and Windows. No compiler or Rust toolchain needed.

---

## Your first trie

```python
from lexrs import Trie

t = Trie()
t.add("apple", 5)   # word + frequency count
t.add("apply", 2)
t.add("apt",   1)
t.add("banana", 8)
```

The frequency count is optional — it defaults to 1 if you omit it. You can also load many words at once:

```python
t.add_all(["apple", "apply", "apt", "banana"])
```

Or from a file where each line is one word:

```python
t.add_from_file("words.txt")
```

---

## Searching

### Membership

```python
"apple" in t              # True
"appl"  in t              # False — not a complete word
t.contains_prefix("app")  # True — at least one word starts with "app"
t.contains_prefix("xyz")  # False
```

### Wildcard search

Two wildcard characters are supported:

| Symbol | Matches |
|---|---|
| `*` | Zero or more characters |
| `?` | Exactly one character |

```python
t.search("ap*")      # ["apple", "apply", "apt"]
t.search("appl?")    # ["apple", "apply"]
t.search("b????")    # 5-letter words starting with b  →  ["banana"] if 6 letters... nothing here
t.search("*ana*")    # ["banana"]
t.search("*")        # every word in the trie
```

To get counts alongside results, pass `with_count=True`:

```python
t.search("ap*", with_count=True)
# [("apple", 5), ("apply", 2), ("apt", 1)]
```

### Prefix completion

When you want every word that starts with a given string:

```python
t.search_with_prefix("app")
# ["apple", "apply"]

t.search_with_prefix("app", with_count=True)
# [("apple", 5), ("apply", 2)]
```

### Fuzzy search

Fuzzy search finds words within a Levenshtein edit distance. Each edit is one insertion, deletion, or character substitution.

```python
t.search_within_distance("aple", 1)
# ["apple"]  — one insertion ("p") away

t.search_within_distance("bannana", 2)
# ["banana"] — two edits away

# With counts
t.search_within_distance("aple", 1, with_count=True)
# [("apple", 5)]
```

A distance of 1 catches most typos. Distance 2 catches transpositions and double-typos but returns more false positives.

---

## Stats

```python
t.get_word_count()   # sum of all frequency counts across all words
len(t)               # number of nodes in the trie
```

---

## DAWG

For large, stable word lists — dictionaries, corpora — the DAWG uses significantly less memory than the Trie by compressing shared suffixes. The trade-off is that words must be inserted in alphabetical order.

`add_all` handles sorting automatically, so in most cases you can switch from `Trie` to `DAWG` by changing just the import:

```python
from lexrs import DAWG

d = DAWG()
d.add_all(["apple", "apply", "apt", "banana"])
# sorted automatically, DAWG minimised on completion
```

If you add words one at a time with `add()`, they must arrive in sorted order:

```python
d = DAWG()
for word in sorted(my_words):
    d.add(word)
```

All search methods are identical to `Trie`:

```python
"apple" in d                                      # True
d.search("ap*")                                   # ["apple", "apply", "apt"]
d.search_with_prefix("app")                       # ["apple", "apply"]
d.search_within_distance("aple", 1)               # ["apple"]
d.search_within_distance("aple", 1, with_count=True)  # [("apple", 1)]
```

---

## Migrating from lexpy

Change the import — nothing else:

```python
# Before
from lexpy.trie import Trie
from lexpy.dawg import DAWG

# After
from lexrs import Trie, DAWG
```

The method signatures, wildcard syntax, and return types are all the same. The only behavioural difference is speed.

---

## Working with large word lists

For a 370 000-word English dictionary, a typical workflow looks like this:

```python
from lexrs import DAWG

# Load once at startup
d = DAWG()
d.add_from_file("/usr/share/dict/words")

# Then query as many times as you want — searches are fast
results = d.search("un*able")
fuzzy   = d.search_within_distance("misspeling", 2)
```

Loading 370k words takes roughly 1–2 seconds. After that, individual searches complete in microseconds.
