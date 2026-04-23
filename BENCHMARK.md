# Genome Benchmark: lexpy vs lexrs vs Pure Rust

A performance comparison of k-mer indexing and querying across three implementations: a pure Python Trie/DAWG (`lexpy`), its Rust counterpart exposed via PyO3 (`lexrs`), and the same Rust code called directly with no Python overhead (Pure Rust).

---

## Setup

| Parameter | Value |
|---|---|
| Reads | 5,000 simulated Illumina reads, 150 bp each |
| Reference genome | 50,000 bp (short, causing high read overlap) |
| k-mer size | k=31 (matches SPAdes/Velvet default) |
| Unique k-mers indexed | ~208,000 |
| Platform | Apple Silicon Mac, Python 3.12 |

A short reference genome relative to read count means the same reference regions appear repeatedly across reads, producing a dense, repetitive k-mer index — a realistic worst case for memory and traversal pressure.

---

## Results

```
Scenario                  lexpy        lexrs     Pure Rust   Rust/lexpy   Rust/lexrs
──────────────────────────────────────────────────────────────────────────────────────
Trie build               12.45 s      2.64 s      442.6 ms       28.1x         6.0x
DAWG build               14.63 s      8.74 s        2.41 s        6.1x         3.6x
Primer lookup (Trie)      5.0 ms      2.6 ms        0.4 ms       12.5x         6.5x
Primer lookup (DAWG)      6.0 ms      3.0 ms        0.4 ms       15.0x         7.5x
Read classification     282.9 ms    392.0 ms      114.7 ms        2.5x         3.4x
Error correct  d=1        1.47 s    346.5 ms       31.1 ms       47.3x        11.1x
Error correct  d=2        3.75 s    918.7 ms       80.8 ms       46.4x        11.4x
Motif discovery         240.5 ms     68.7 ms      103.2 ms        2.3x         0.7x
Full enumeration          2.63 s    575.4 ms       91.6 ms       28.7x         6.3x
Full pipeline            33.06 s     15.18 s        4.74 s        7.0x         3.2x
```

---

## Scenarios and Analysis

### 1. K-mer Index Build (Trie and DAWG)

**What it is.** A k-mer is a fixed-length substring of length k extracted with a sliding window across each read. All unique k-mers are inserted into either a Trie or a DAWG. This is the first step in nearly every short-read assembler and k-mer counter (Jellyfish, KMC, Meryl).

**DNA alphabet properties.** The 4-character alphabet (A, C, G, T) gives each trie node up to 4 children. With k=31, every path is exactly 31 levels deep. The trie is maximally regular — no variable branching, no skipping — which means allocation patterns are predictable and cache-friendly in Rust, and expensive in Python due to dict-per-node overhead.

**DAWG vs Trie.** A DAWG (Directed Acyclic Word Graph) adds suffix minimization on top of the trie: nodes with identical subtrees are merged. This makes the structure more compact and prefix/suffix queries faster, but the build step requires computing a structural signature for each node via hashmap lookup. That extra indirection narrows the Python-to-Rust gap.

**Results.** Trie build is **28x faster** in Pure Rust vs lexpy, but DAWG build is only **6x faster**. The minimization pass is memory-intensive and harder to pipeline — Rust still wins, but the margin shrinks because the bottleneck shifts from interpreter overhead to memory access patterns that both implementations face equally.

---

### 2. Primer / Seed Lookup

**What it is.** 300 primers (20 bp sequences sampled from the reference) are used as prefixes to find all indexed k-mers that begin with that primer, via `search_with_prefix`. This is the seed step in read aligners like BWA-MEM and Bowtie2: before extending an alignment, find all candidate positions sharing a short exact prefix.

**Results.** Pure Rust is **12–15x faster** than lexpy, and about **6–7x faster** than lexrs. The PyO3 boundary cost is visible here (lexrs pays it per-call), but prefix traversal is short enough that the ratio stays moderate.

---

### 3. Read Classification

**What it is.** For 500 reads, every k-mer in each read is checked against the index with `contains()`. That produces 60,000 individual lookups. The fraction of indexed k-mers determines whether a read is classified as matching the reference.

**The anomaly.** This is the weakest Pure Rust result (only **2.5x** over lexpy) and the only case where **lexrs is slower than lexpy** (0.74x). The reason is the PyO3 boundary crossing. Each `contains()` call requires: acquiring the GIL, marshaling a Python `str` to a Rust `&str`, executing the lookup, and returning a Python `bool`. With 60,000 calls, that overhead dominates the actual computation. lexpy avoids this entirely — it stays inside the Python interpreter for all 60,000 lookups. Pure Rust avoids it from the other direction by never touching Python at all.

The lesson: for high-frequency, low-latency calls, PyO3 bindings impose a fixed per-call cost that can exceed the savings from the faster implementation. Batching (pass all k-mers in one call, return a result list) would recover most of this loss.

---

### 4 & 5. Error Correction (d=1 and d=2)

**What it is.** Given a corrupted k-mer (one or two substituted bases, simulating Illumina sequencing errors), find all indexed k-mers within Levenshtein distance d. Tools like BFC and Lighter do this before assembly to repair reads. At d=2, the same approach covers PacBio and Nanopore reads with higher per-base error rates.

**Why this shows the largest speedup.** Error correction requires a recursive traversal of the entire trie, maintaining a Levenshtein DP table at every node. A 31-level trie with branching factor 4 means thousands of node visits per query, and at each node the DP row must be updated and checked. Python pays interpreter dispatch overhead at every node visit and every array operation. Rust pays nothing — the DP update is a tight inner loop that the compiler can vectorize.

**Results.** Pure Rust is **47x faster** than lexpy at d=1 and **46x faster** at d=2. lexrs is **11x faster** than lexpy, meaning PyO3 accounts for a **4–5x** additional tax on top of the algorithm itself. The d=2 ratios are nearly identical to d=1 because the work scales with trie depth, not query count.

---

### 6. Motif Discovery

**What it is.** Wildcard patterns inspired by IUPAC ambiguity codes are searched across the k-mer index. `?` matches any single base (like IUPAC N, R, Y), `*` matches any-length sequence. Example: `GC*GC` finds k-mers containing two GC dinucleotides separated by any intervening sequence.

**The anomaly.** lexrs (**68.7 ms**) is faster than Pure Rust (**103.2 ms**). This is an artifact of different random number generators used in the Python and Rust benchmark harnesses (Python `random` vs xorshift64), which produce slightly different k-mer sets and, critically, different pattern sets. The Python benchmark's patterns hit only ~900 matches; the Rust benchmark's `*` wildcard enumerates all ~213,000 k-mers. The aggregate time comparison is not apples-to-apples here — the per-pattern breakdown is the meaningful metric, and both outperform lexpy by **2–3x** on equivalent patterns.

---

### 7. Full K-mer Enumeration

**What it is.** `search("*")` dumps every indexed k-mer. This is the entry point for De Bruijn graph construction in de-novo assembly: each k-mer becomes a node, and two k-mers sharing a (k-1)-length overlap form a directed edge.

**Results.** Pure Rust is **28.7x faster** than lexpy. The gap is large because enumeration visits every node in the trie exactly once — the Python interpreter pays per-node overhead across all 208,000 k-mers, whereas Rust iterates with no per-node overhead.

---

### 8. Full Pipeline

**What it is.** One end-to-end run: build + primer lookup + read classification + error correction + motif discovery + enumeration. This is the total wall-time for a complete assembly preprocessing pass.

**Results.** Pure Rust completes in **4.74 s** vs **33.06 s** for lexpy — a **7x** end-to-end speedup. lexrs (**15.18 s**) is 2.2x slower than Pure Rust, showing that the PyO3 boundary cost is a persistent drag across the full workload, not just isolated calls.

---

## Key Takeaways

**The PyO3 tax is 6–11x.** Crossing the Python-Rust boundary includes GIL acquisition, string marshaling (`str` → `&str`), and result conversion (`Vec<String>` → Python list). For lexrs, this overhead is unavoidable on every call. Batching multiple operations into single boundary crossings is the primary lever for closing the gap.

**Error correction is the decisive win.** At 47x speedup, it confirms that recursive tree traversal with inner-loop arithmetic is exactly where Rust outperforms Python by the widest margin. If error correction is on the critical path, Pure Rust is non-negotiable.

**Read classification exposes the batching problem.** 60,000 fine-grained calls make lexrs slower than lexpy. This is fixable by design: a `contains_batch(kmers: Vec<&str>) -> Vec<bool>` API would cross the boundary once and recover the full Rust speedup.

**DAWG minimization compresses the Python-Rust gap.** The algorithmic complexity of signature computation (hashmap lookups during minimization) shifts the bottleneck away from interpreter overhead. Both Python and Rust spend meaningful time on memory access rather than instruction dispatch.

**End-to-end, Pure Rust is 7x faster.** For production genomics pipelines processing gigabase-scale inputs, that margin translates directly to wall-time and cost.
