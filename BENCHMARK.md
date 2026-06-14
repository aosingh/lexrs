# Genome Benchmark: lexpy vs pylexrs vs lexrs

> **Note:** The results below reflect the sequential API (`search_with_prefix`, `contains`, etc.).
> As of v1.0, batch APIs (`batch_contains`, `batch_search`, `batch_search_with_prefix`,
> `batch_search_within_distance`) are available and deliver an additional **2–6× speedup** over
> the sequential pylexrs numbers shown here. See `benchmarks/benchmark_batch.py` for batch-specific
> results.

A performance comparison of k-mer indexing and querying across three implementations: a pure Python Trie/DAWG (`lexpy`), its Rust counterpart exposed via PyO3 (`pylexrs`), and the same Rust code called directly with no Python overhead (`lexrs`).

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
Scenario                  lexpy      pylexrs        lexrs   lexrs/lexpy  lexrs/pylexrs
──────────────────────────────────────────────────────────────────────────────────────
Trie build                9.07 s      2.29 s      471.9 ms        19.2x          4.9x
DAWG build               11.91 s      7.18 s        2.10 s         5.7x          3.4x
Primer lookup (Trie)      4.3 ms      0.9 ms        0.4 ms        10.7x          2.2x
Primer lookup (DAWG)      5.6 ms      1.0 ms        0.4 ms        14.0x          2.5x
Read classification     305.8 ms    193.0 ms      124.3 ms         2.5x          1.6x
Error correct  d=1        1.45 s     53.7 ms       38.6 ms        37.6x          1.4x
Error correct  d=2        3.60 s    150.8 ms       88.9 ms        40.5x          1.7x
Motif discovery         245.3 ms     63.2 ms      148.8 ms         1.6x          0.4x
Full enumeration          2.61 s    547.9 ms       90.2 ms        28.9x          6.1x
Full pipeline            28.18 s     12.96 s        4.51 s         6.2x          2.9x
```

---

## Scenarios and Analysis

### 1. K-mer Index Build (Trie and DAWG)

**What it is.** A k-mer is a fixed-length substring of length k extracted with a sliding window across each read. All unique k-mers are inserted into either a Trie or a DAWG. This is the first step in nearly every short-read assembler and k-mer counter (Jellyfish, KMC, Meryl).

**DNA alphabet properties.** The 4-character alphabet (A, C, G, T) gives each trie node up to 4 children. With k=31, every path is exactly 31 levels deep. The trie is maximally regular — no variable branching, no skipping — which means allocation patterns are predictable and cache-friendly in Rust, and expensive in Python due to dict-per-node overhead.

**DAWG vs Trie.** A DAWG (Directed Acyclic Word Graph) adds suffix minimization on top of the trie: nodes with identical subtrees are merged. This makes the structure more compact and prefix/suffix queries faster, but the build step requires computing a structural signature for each node via hashmap lookup. That extra indirection narrows the Python-to-Rust gap.

**Results.** Trie build is **19x faster** in lexrs vs lexpy, but DAWG build is only **6x faster**. The minimization pass is memory-intensive and harder to pipeline — Rust still wins, but the margin shrinks because the bottleneck shifts from interpreter overhead to memory access patterns that both implementations face equally.

---

### 2. Primer / Seed Lookup

**What it is.** 300 primers (20 bp sequences sampled from the reference) are used as prefixes to find all indexed k-mers that begin with that primer, via `search_with_prefix`. This is the seed step in read aligners like BWA-MEM and Bowtie2: before extending an alignment, find all candidate positions sharing a short exact prefix.

**Results.** lexrs is **11–14x faster** than lexpy, and about **2–2.5x faster** than pylexrs. With batch APIs, pylexrs now crosses the boundary once for all primers rather than per-call, which accounts for the improved ratio vs the old sequential numbers.

---

### 3. Read Classification

**What it is.** For 500 reads, every k-mer in each read is checked against the index with `contains()`. That produces 60,000 individual lookups. The fraction of indexed k-mers determines whether a read is classified as matching the reference.

**The anomaly.** This is the weakest lexrs result (only **2.5x** over lexpy) and the only case where **pylexrs is slower than lexpy** (0.74x). The reason is the PyO3 boundary crossing. Each `contains()` call requires: acquiring the GIL, marshaling a Python `str` to a Rust `&str`, executing the lookup, and returning a Python `bool`. With 60,000 calls, that overhead dominates the actual computation. lexpy avoids this entirely — it stays inside the Python interpreter for all 60,000 lookups. lexrs avoids it from the other direction by never touching Python at all.

**Fix (v1.0).** `batch_contains(kmers)` crosses the boundary once, processes all k-mers in parallel via Rayon, and returns a `list[bool]`. The read classification loop is now `sum(trie.batch_contains(kmers(r, k)))` per read — pylexrs (193 ms) is now faster than lexpy (305 ms), a complete reversal from the old sequential result.

---

### 4 & 5. Error Correction (d=1 and d=2)

**What it is.** Given a corrupted k-mer (one or two substituted bases, simulating Illumina sequencing errors), find all indexed k-mers within Levenshtein distance d. Tools like BFC and Lighter do this before assembly to repair reads. At d=2, the same approach covers PacBio and Nanopore reads with higher per-base error rates.

**Why this shows the largest speedup.** Error correction requires a recursive traversal of the entire trie, maintaining a Levenshtein DP table at every node. A 31-level trie with branching factor 4 means thousands of node visits per query, and at each node the DP row must be updated and checked. Python pays interpreter dispatch overhead at every node visit and every array operation. Rust pays nothing — the DP update is a tight inner loop that the compiler can vectorize.

**Results.** lexrs is **38–40x faster** than lexpy. pylexrs is **27–24x faster** than lexpy, with the batch API bringing it to within **1.4–1.7x** of lexrs. The d=2 ratios are nearly identical to d=1 because the work scales with trie depth, not query count.

---

### 6. Motif Discovery

**What it is.** Wildcard patterns inspired by IUPAC ambiguity codes are searched across the k-mer index. `?` matches any single base (like IUPAC N, R, Y), `*` matches any-length sequence. Example: `GC*GC` finds k-mers containing two GC dinucleotides separated by any intervening sequence.

**The anomaly.** pylexrs (**63.2 ms**) is faster than lexrs (**148.8 ms**). This is an artifact of different random number generators used in the Python and Rust benchmark harnesses (Python `random` vs xorshift64), which produce slightly different k-mer sets and, critically, different pattern sets. The Python benchmark's patterns hit only ~900 matches; the Rust benchmark's `*` wildcard enumerates all ~213,000 k-mers. The aggregate time comparison is not apples-to-apples here — the per-pattern breakdown is the meaningful metric, and both outperform lexpy by **2–3x** on equivalent patterns.

---

### 7. Full K-mer Enumeration

**What it is.** `search("*")` dumps every indexed k-mer. This is the entry point for De Bruijn graph construction in de-novo assembly: each k-mer becomes a node, and two k-mers sharing a (k-1)-length overlap form a directed edge.

**Results.** lexrs is **28.9x faster** than lexpy. The gap is large because enumeration visits every node in the trie exactly once — the Python interpreter pays per-node overhead across all 208,000 k-mers, whereas Rust iterates with no per-node overhead.

---

### 8. Full Pipeline

**What it is.** One end-to-end run: build + primer lookup + read classification + error correction + motif discovery + enumeration. This is the total wall-time for a complete assembly preprocessing pass.

**Results.** lexrs completes in **4.51 s** vs **28.18 s** for lexpy — a **6.2x** end-to-end speedup. pylexrs (**12.96 s**) is 2.9x slower than lexrs, though the gap has narrowed significantly with batch APIs handling the high-frequency call sites.

---

## Key Takeaways

**The PyO3 tax is 6–11x per call.** Crossing the Python-Rust boundary includes GIL acquisition, string marshaling (`str` → `&str`), and result conversion (`Vec<String>` → Python list). The batch APIs (`batch_contains`, `batch_search`, `batch_search_with_prefix`, `batch_search_within_distance`) amortise this cost across an entire input list, crossing the boundary once regardless of list length, and process items in parallel via Rayon.

**Error correction is the decisive win.** At 38–40x speedup, it confirms that recursive tree traversal with inner-loop arithmetic is exactly where Rust outperforms Python by the widest margin. With batch APIs, pylexrs is now within 1.4–1.7x of lexrs for error correction. If error correction is on the critical path, lexrs is non-negotiable.

**Read classification is solved by `batch_contains`.** Sequential per-k-mer calls made pylexrs slower than lexpy. With `batch_contains`, pylexrs (193 ms) is now faster than lexpy (305 ms) — a complete reversal.

**DAWG minimization compresses the Python-Rust gap.** The algorithmic complexity of signature computation (hashmap lookups during minimization) shifts the bottleneck away from interpreter overhead. Both Python and Rust spend meaningful time on memory access rather than instruction dispatch.

**End-to-end, lexrs is 6x faster.** For production genomics pipelines processing gigabase-scale inputs, that margin translates directly to wall-time and cost. With batch APIs, pylexrs closes a significant portion of the remaining gap without requiring a full Rust rewrite of the calling code.
