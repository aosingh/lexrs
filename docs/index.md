# lexrs

lexrs is a lexicon library built in Rust. It gives you two data structures — **Trie** and **DAWG** — that let you store large word lists and search them with wildcards, prefix completion, and fuzzy (Levenshtein) matching.

If you have used [lexpy](https://github.com/aosingh/lexpy) before, the API is the same. The difference is speed: the core data structures are compiled Rust, so insertion and search run 10–100× faster.

---

## Choose your entry point

<div class="grid cards" markdown>

-   **Python package**

    ---

    Install `pylexrs` from PyPI and use `Trie` and `DAWG` from Python. Pre-built wheels for Python 3.11–3.14 on Linux, macOS, and Windows — no Rust toolchain needed.

    [:octicons-arrow-right-24: Python guide](python.md)

-   **Rust library**

    ---

    Add `lexrs` to your `Cargo.toml` and use the data structures natively in Rust. Zero overhead, full type safety.

    [:octicons-arrow-right-24: Rust guide](rust.md)

-   **HTTP server**

    ---

    Run `writer` and `reader` as separate processes. The writer accepts word ingestion; readers serve search queries from a compressed DAWG. Scale readers horizontally.

    [:octicons-arrow-right-24: Server guide](server.md)

-   **Docker Compose**

    ---

    Bring up the full stack — Consul, writer, two readers, and nginx — with a single command.

    [:octicons-arrow-right-24: Docker guide](docker.md)

</div>

---

## Trie vs DAWG — which should I use?

Both support the same search operations. The difference is in how they store words.

A **Trie** shares prefixes. The words `apple`, `apply`, and `apt` share the prefix `ap`, so they share nodes:

```
        (root)
          │
          a
          │
          p ──────────────┐
          │               │
          p               t  ← "apt"
          │
          l
         / \
        e   y
        ↑   ↑
     "apple" "apply"
```

A **DAWG** also shares suffixes. Words with the same endings — like `nation` and `action` — collapse their shared `tion` suffix into a single path through the graph. For large dictionaries this can reduce node count by 3–5×.

The practical rule:

| | Trie | DAWG |
|---|---|---|
| Words arrive in any order | ✓ | — |
| Words can be inserted incrementally | ✓ | — |
| Large, mostly-static lexicon | — | ✓ |
| Lowest possible memory footprint | — | ✓ |
| Build once, read many times | either | prefer DAWG |

In the HTTP server the writer uses a Trie (delta ingestion, any order) and the reader uses a DAWG (built from a sorted snapshot, optimised for search).
