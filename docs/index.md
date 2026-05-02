# lexrs

[![PyPI](https://img.shields.io/pypi/v/pylexrs?label=pylexrs&color=blue)](https://pypi.org/project/pylexrs/)
[![Crates.io](https://img.shields.io/crates/v/lexrs?label=lexrs&color=orange)](https://crates.io/crates/lexrs)
[![Crates.io](https://img.shields.io/crates/v/lexrs-server?label=lexrs-server&color=orange)](https://crates.io/crates/lexrs-server)

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

## Use cases

Tries and DAWGs are general-purpose string index structures. Anywhere you need fast prefix, pattern, or approximate matching over a large set of strings, they apply.

**Autocomplete and typeahead**
Store product names, usernames, or search terms. Query with `search_with_prefix` on every keystroke. Attach counts to surface the most popular completions first.

**Spell checking and fuzzy matching**
Store a reference dictionary. When a query word is not found, retry with `search_within_distance(word, 1)` to find close matches. Works well for "did you mean?" suggestions in search engines, forms, and chat interfaces.

**Genomics and bioinformatics**
DNA sequences are strings over a four-character alphabet (A, T, G, C). A Trie or DAWG can index all k-mers (substrings of length k) extracted from a reference genome, then answer queries like "does this read occur in the reference?" or "what sequences are within one mutation of this probe?" with wildcard and fuzzy search. The compact node representation matters here — a human genome produces hundreds of millions of k-mers.

**NLP vocabulary management**
Index the vocabulary of a language model or corpus. Use prefix search to enumerate all tokens that share a morphological root, wildcard search to find tokens matching a pattern (e.g. all past-tense verb forms `*ed`), and fuzzy search to cluster near-duplicate tokens before fine-tuning.

**Log and trace analysis**
Index service names, endpoint paths, or error codes from a logging pipeline. Wildcard queries like `api.*/timeout` let operators explore a live system without knowing exact identifiers in advance.

**Domain and hostname lookup**
Store known hostnames or domain allowlists. `contains` is an O(length) exact lookup with no hashing collisions. `search_with_prefix` enumerates all subdomains under a given domain.

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
