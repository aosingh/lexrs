# lexrs

**lexrs** is a high-performance lexicon library implementing two data structures — **Trie** and **DAWG** (Directed Acyclic Word Graph) — compiled in Rust with Python bindings via [PyO3](https://pyo3.rs).

It is the successor to [lexpy](https://github.com/aosingh/lexpy). The API is identical, but insertion and search run 10–100× faster by moving the core data structures to Rust.

---

## Install

=== "Python"

    ```bash
    pip install pylexrs
    ```

=== "Rust"

    ```toml
    # Cargo.toml
    [dependencies]
    lexrs = "0.2"
    ```

=== "HTTP Server"

    ```bash
    cargo install lexrs-server
    ```

---

## What's inside

| Component | Description |
|---|---|
| **`lexrs`** (Rust crate) | Core Trie and DAWG implementations. Use as a library in any Rust project. |
| **`pylexrs`** (Python package) | PyO3 bindings exposing the same API to Python. Drop-in replacement for `lexpy`. |
| **`lexrs-server`** (Rust crate) | Two production binaries — `writer` and `reader` — forming a horizontally scalable HTTP search service. |

---

## Architecture overview

At the library level, **Trie** and **DAWG** share the same interface. Choose based on your use case:

- **Trie** — insertions in any order; good for mutable, delta-style ingestion.
- **DAWG** — words must be inserted in sorted order; call `reduce()` after loading. Compresses shared suffixes on top of shared prefixes, producing a much smaller node count for large lexicons. Ideal for read-heavy workloads.

For production deployments the HTTP server separates writes from reads:

```
            ┌───────────┐
  writes ──▶│  writer   │──▶ shared volume (snapshots/)
            └─────┬─────┘         │
                  │ Consul KV     │
                  ▼               ▼
            ┌─────────────┐  ┌──────────┐
            │   Consul    │─▶│ reader   │ × N ──▶ search queries
            └─────────────┘  └──────────┘
```

- The **writer** buffers incoming words in a Trie and compacts them periodically into a versioned snapshot file on a shared volume.
- Each **reader** loads the latest snapshot into a DAWG and hot-reloads new versions without downtime via Consul's blocking-query watch.
- **nginx** routes write endpoints to the writer and read endpoints round-robin across all readers.

---

## Quick example

=== "Python"

    ```python
    from lexrs import Trie

    t = Trie()
    t.add_all(["apple", "apply", "apt", "banana"])

    t.search("ap*")                        # ["apple", "apply", "apt"]
    t.search_within_distance("aple", 1)    # ["apple"]  (Levenshtein ≤ 1)
    t.search_with_prefix("ban")            # ["banana"]
    ```

=== "Rust"

    ```rust
    use lexrs::Trie;

    let mut trie = Trie::new();
    trie.add_all(vec!["apple", "apply", "apt", "banana"]).unwrap();

    let results = trie.search("ap*").unwrap();          // ["apple", "apply", "apt"]
    let fuzzy   = trie.search_within_distance("aple", 1); // ["apple"]
    ```

=== "HTTP"

    ```bash
    # ingest
    curl -X POST http://localhost/words \
      -H 'Content-Type: application/json' \
      -d '{"words": ["apple", "apply", "apt", "banana"]}'

    # wildcard search
    curl 'http://localhost/search?q=ap*'

    # fuzzy search
    curl 'http://localhost/search?q=aple&dist=1'
    ```
