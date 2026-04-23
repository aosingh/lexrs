# benchmarks

Performance benchmarks comparing `lexrs` (Rust/PyO3) against `lexpy` (pure Python) across a range of workloads and corpus sizes.

## Scripts

| File | Description |
|---|---|
| `benchmark.py` | Main benchmark: insertion, exact lookup, prefix, wildcard, and Levenshtein search |
| `benchmark_genome.py` | Genome-scale benchmark (large, repetitive string corpus) |
| `benchmark_summary.py` | Prints a summary table from stored results |
| `benchmark_workloads.py` | Parameterized workload runner for profiling specific scenarios |

## Prerequisites

```bash
# Install lexpy (pure Python baseline)
pip install lexpy

# Build and install lexrs into the active virtualenv
maturin develop --features python   # run from the repo root
```

## Running

```bash
# Full comparison benchmark (uses /usr/share/dict/words)
python benchmarks/benchmark.py

# Genome-scale
python benchmarks/benchmark_genome.py
```

## What is measured

The main benchmark (`benchmark.py`) runs each operation 5 times and takes the best time to reduce noise.

**Corpus**: `/usr/share/dict/words`, filtered to alphabetic words only (~235k words on macOS).

**Corpus sizes tested for insertion**: 1k, 10k, 50k, 100k words.

**Search benchmarks** are built on a random 50k-word sample.

| Section | What is timed |
|---|---|
| Insertion | `add_all(words)` for Trie and DAWG at each corpus size |
| Exact lookup | 500 present words + 500 absent words (`w in structure`) |
| Prefix search | `search_with_prefix(p)` for 5 common prefixes |
| Wildcard search | `search(pattern)` for 5 patterns (`a*`, `str*`, `*ing`, `un?*`, `????`) |
| Levenshtein | `search_within_distance(word, dist)` for 4 word/distance pairs |

## Interpreting results

Each row prints `lexpy time`, `lexrs time`, and a speedup multiplier. Higher speedup = faster Rust implementation. Typical results show **10–100×** speedup for CPU-bound operations like wildcard and Levenshtein search.

See [BENCHMARK.md](../BENCHMARK.md) at the repo root for recorded results.
