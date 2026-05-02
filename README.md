# lexrs

A Rust library implementing two efficient lexicon data structures — **Trie** and **DAWG** (Directed Acyclic Word Graph) — with an optional Python binding via [PyO3](https://pyo3.rs) and [Maturin](https://maturin.rs).

`lexrs` is the successor to [lexpy](https://github.com/aosingh/lexpy) (pure Python). It exposes the same API while delivering 10–100× faster insertion and search by moving the core data structures to Rust.

## Table of contents

- [Install](#install)
- [Data structures](#data-structures)
- [Features](#features)
- [Rust usage](#rust-usage)
- [Python usage](#python-usage)
  - [API](#api)
  - [Wildcard syntax](#wildcard-syntax)
- [Production HTTP server](#production-http-server)
- [Running tests](#running-tests)
- [Project structure](#project-structure)
- [Related components](#related-components)
- [License](#license)

## Install

**Rust library** — add to `Cargo.toml`:

```toml
[dependencies]
lexrs = "0.2"
```

**Python package**:

```bash
pip install pylexrs
```

**HTTP server binaries**:

```bash
cargo install lexrs-server
```

This installs both the `writer` and `reader` binaries. See the [Production HTTP server](#production-http-server) section for deployment details.

---

## Data structures

### Trie

A standard prefix tree using arena allocation (`Vec<Node>`). Supports insertion in any order.

### DAWG

A minimized Trie that compresses shared suffixes in addition to shared prefixes, resulting in a significantly smaller node count for large lexicons. Words **must be inserted in lexicographic (alphabetical) order**; call `reduce()` after all insertions to finalize minimization.

## Features

Both `Trie` and `DAWG` expose the same API:

| Method | Description |
|---|---|
| `add(word, count)` | Insert a word with an optional frequency count |
| `add_all(words)` | Insert from any iterable |
| `add_from_file(path)` | Insert words from a file (one per line) |
| `contains(word)` | Exact membership test |
| `contains_prefix(prefix)` | Check if any word starts with the given prefix |
| `search(pattern)` | Wildcard search (`*` = zero or more chars, `?` = exactly one) |
| `search_with_count(pattern)` | Like `search`, but returns `(word, count)` pairs |
| `search_with_prefix(prefix)` | All words beginning with the prefix |
| `search_with_prefix_count(prefix)` | Like above, with counts |
| `search_within_distance(word, dist)` | Levenshtein fuzzy search |
| `search_within_distance_count(word, dist)` | Like above, with counts |
| `word_count()` | Number of words stored |
| `node_count()` | Number of nodes in the structure |

## Rust usage

```toml
# Cargo.toml
[dependencies]
lexrs = "0.2"
```

```rust
use lexrs::{Trie, Dawg};

// Trie
let mut trie = Trie::new();
trie.add("hello", 1).unwrap();
trie.add_all(vec!["world".to_string(), "foo".to_string()]).unwrap();

assert!(trie.contains("hello"));
assert!(trie.contains_prefix("wor"));

let results = trie.search("h*").unwrap();       // wildcard
let fuzzy   = trie.search_within_distance("helo", 1); // Levenshtein ≤ 1

// DAWG (words must be inserted in alphabetical order)
let mut dawg = Dawg::new();
dawg.add_all(vec!["apple".to_string(), "apply".to_string(), "apt".to_string()]).unwrap();
// add_all sorts automatically; call reduce() if you use add() directly
dawg.reduce();

assert!(dawg.contains("apple"));
```

## Python usage

The library is available on PyPI as `pylexrs`:

```bash
pip install pylexrs
```

### API

```python
from lexrs import Trie, DAWG

# ── Trie ──────────────────────────────────────────────────────────────────────
t = Trie()
t.add("hello", 5)          # word + optional count
t.add_all(["world", "foo"])
t.add_from_file("words.txt")

"hello" in t               # True
t.contains_prefix("wor")   # True
t.get_word_count()         # total words
len(t)                     # total nodes

t.search("h*")                          # wildcard, returns list of words
t.search("h*", with_count=True)         # returns list of (word, count)
t.search_with_prefix("wo")              # prefix completion
t.search_with_prefix_count("wo")        # with counts
t.search_within_distance("helo", 1)     # fuzzy (Levenshtein ≤ 1)
t.search_within_distance("helo", 1, with_count=True)

# ── DAWG ──────────────────────────────────────────────────────────────────────
d = DAWG()
d.add_all(["apple", "apply", "apt"])   # sorted automatically

"apple" in d               # True
d.search("ap*")            # wildcard
d.search_within_distance("aple", dist=1)
```

### Wildcard syntax

| Pattern | Meaning |
|---|---|
| `*` | Zero or more characters |
| `?` | Exactly one character |
| `h*` | All words starting with `h` |
| `?at` | Three-letter words ending in `at` |
| `a?*` | Words of two or more characters starting with `a` |

Consecutive wildcards are normalized (`**` → `*`, `?*` → `*`).

## Production HTTP server

`lexrs` ships a production-ready HTTP service in [`lexrs-server/`](lexrs-server/) with two binaries:

| Binary | Role |
|---|---|
| **writer** | Accepts word ingestion (`POST /ingest`), buffers a delta Trie in memory, and periodically compacts it into a versioned snapshot on disk. |
| **reader** | Loads the latest snapshot into a DAWG and serves all search queries. Multiple readers can run as replicas and hot-reload new snapshots without downtime. |

[Consul](https://www.consul.io/) is used for snapshot version coordination — the writer publishes new snapshot versions to the Consul KV store and readers watch for changes via blocking queries, atomically swapping the in-memory DAWG when a new snapshot arrives.

### Docker Compose — full stack in one command

The [`docker/`](docker/) directory has a Compose file that brings up the entire stack:

```
Consul ──► writer ──► (snapshots on shared volume)
                              │
              ┌───────────────┘
              ▼
         reader-1   reader-2
              │         │
              └────┬────┘
                   ▼
                 nginx  (port 80)
                 /ingest  → writer
                 /search  → readers (round-robin)
```

```bash
cd docker
docker compose up -d
```

```bash
# ingest words
curl -s -X POST http://localhost/ingest \
  -H 'Content-Type: application/json' \
  -d '["apple", "apply", {"word": "application", "count": 5}]'

# prefix search
curl -s 'http://localhost/search?prefix=app'

# wildcard search
curl -s 'http://localhost/search?pattern=app*'

# fuzzy search (Levenshtein distance ≤ 1)
curl -s 'http://localhost/search?word=aple&dist=1'
```

The writer and reader are built from a single Dockerfile — the `command:` field in Compose selects which binary to start. See [`lexrs-server/README.md`](lexrs-server/README.md) for the full API reference and configuration options.

## Running tests

```bash
# Rust unit tests
cargo test

# Python tests
pip install pylexrs pytest
pytest tests/
```

## Project structure

```
src/
  lib.rs      — public re-exports and Python module registration
  trie.rs     — Trie implementation + wildcard / Levenshtein helpers
  dawg.rs     — DAWG implementation
  node.rs     — shared Node type (arena-allocated)
  utils.rs    — file I/O and pattern normalization
  error.rs    — LexError enum
tests/
  test_python_api.py — pytest suite for the Python bindings
```

## Related components

| Directory | Description |
|---|---|
| [lexrs-server/](lexrs-server/) | Production HTTP server — a **writer** binary for word ingestion and compaction, and a **reader** binary for search. Readers scale horizontally and hot-reload snapshots via Consul. |
| [docker/](docker/) | Docker Compose setup running the full stack: Consul, writer, two reader replicas, and an nginx reverse proxy that routes reads and writes to the right service. |
| [lexpy-shim/](lexpy-shim/) | Source for `lexpy==2.x` — a one-file compatibility shim that re-exports `lexrs` so existing `lexpy` users can upgrade without changing their imports. |
| [benchmarks/](benchmarks/) | Python scripts comparing `lexrs` against `lexpy` (pure Python) across insertion, prefix, wildcard, and Levenshtein workloads. |
| [tests/](tests/) | Integration tests — `pytest` suite for the Python API and Rust-level integration tests. |

## License

MIT
