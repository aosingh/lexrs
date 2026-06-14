"""
Benchmark: sequential (Python loop) vs batch (Rayon parallel) for lexrs.

For each operation we compare:
  sequential  — call the single-item method in a Python loop
  batch       — call the batch_* method once with the full list

Both Trie and DAWG are benchmarked.
"""

import random
import sys
import time

from lexrs import DAWG as RsDAWG
from lexrs import Trie as RsTrie

# ── helpers ───────────────────────────────────────────────────────────────────


def load_words(path="/usr/share/dict/words"):
    with open(path) as f:
        return [line.strip() for line in f if line.strip().isalpha()]


def fmt(seconds):
    ms = seconds * 1000
    if ms < 0.01:
        return f"{seconds * 1_000_000:.2f} µs"
    return f"{ms:.2f} ms"


def speedup(seq, batch):
    if batch == 0:
        return "∞"
    return f"{seq / batch:.2f}x"


def bench(fn, repeat=5):
    best = float("inf")
    result = None
    for _ in range(repeat):
        t0 = time.perf_counter()
        result = fn()
        best = min(best, time.perf_counter() - t0)
    return best, result


def section(title):
    print("\n" + "=" * 72)
    print(f"  {title}")
    print("=" * 72)
    print(f"  {'Operation':<28}  {'sequential':>12}  {'batch':>12}  {'speedup':>10}")
    print("  " + "-" * 68)


def row(label, seq_t, batch_t):
    print(
        f"  {label:<28}  {fmt(seq_t):>12}  {fmt(batch_t):>12}  {speedup(seq_t, batch_t):>10}"
    )


# ── dataset ───────────────────────────────────────────────────────────────────

ALL_WORDS = load_words()
random.seed(42)

CORPUS_SIZE = 50_000
corpus = random.sample(ALL_WORDS, min(CORPUS_SIZE, len(ALL_WORDS)))

LOOKUP_SIZE = 1_000
lookup_words = random.sample(corpus, LOOKUP_SIZE)
miss_words = [w + "zzz" for w in lookup_words]

PREFIXES = ["a", "pre", "un", "str", "com", "re", "de", "dis", "over", "mis"] * 20
PATTERNS = ["a*", "str*", "*ing", "un?*", "????"] * 20

LEV_WORDS = random.sample(corpus, 200)
LEV_DIST = 1

# ── build structures ──────────────────────────────────────────────────────────

print(f"\nlexrs  (Rust/PyO3/Rayon)")
print(f"corpus : {len(corpus):,} words  |  lookups : {LOOKUP_SIZE:,}  |  Python : {sys.version.split()[0]}")

trie = RsTrie()
trie.add_all(corpus)

dawg = RsDAWG()
dawg.add_all(corpus)

# ── Trie benchmarks ───────────────────────────────────────────────────────────

section("TRIE — batch_contains  (hits)")
seq_t, _ = bench(lambda: [w in trie for w in lookup_words])
bat_t, _ = bench(lambda: trie.batch_contains(lookup_words))
row(f"contains ×{LOOKUP_SIZE}", seq_t, bat_t)

section("TRIE — batch_contains  (misses)")
seq_t, _ = bench(lambda: [w in trie for w in miss_words])
bat_t, _ = bench(lambda: trie.batch_contains(miss_words))
row(f"contains ×{LOOKUP_SIZE}", seq_t, bat_t)

section("TRIE — batch_search_with_prefix")
seq_t, seq_r = bench(lambda: [trie.search_with_prefix(p) for p in PREFIXES])
bat_t, bat_r = bench(lambda: trie.batch_search_with_prefix(PREFIXES))
total = sum(len(r) for r in seq_r)
row(f"prefix ×{len(PREFIXES)}  ({total} hits)", seq_t, bat_t)

section("TRIE — batch_search  (wildcard)")
seq_t, seq_r = bench(lambda: [trie.search(p) for p in PATTERNS])
bat_t, _     = bench(lambda: trie.batch_search(PATTERNS))
total = sum(len(r) for r in seq_r)
row(f"wildcard ×{len(PATTERNS)}  ({total} hits)", seq_t, bat_t)

section("TRIE — batch_search_within_distance")
seq_t, seq_r = bench(lambda: [trie.search_within_distance(w, LEV_DIST) for w in LEV_WORDS])
bat_t, _     = bench(lambda: trie.batch_search_within_distance(LEV_WORDS, LEV_DIST))
total = sum(len(r) for r in seq_r)
row(f"levenshtein d={LEV_DIST} ×{len(LEV_WORDS)}  ({total} hits)", seq_t, bat_t)

# ── DAWG benchmarks ───────────────────────────────────────────────────────────

section("DAWG — batch_contains  (hits)")
seq_t, _ = bench(lambda: [w in dawg for w in lookup_words])
bat_t, _ = bench(lambda: dawg.batch_contains(lookup_words))
row(f"contains ×{LOOKUP_SIZE}", seq_t, bat_t)

section("DAWG — batch_contains  (misses)")
seq_t, _ = bench(lambda: [w in dawg for w in miss_words])
bat_t, _ = bench(lambda: dawg.batch_contains(miss_words))
row(f"contains ×{LOOKUP_SIZE}", seq_t, bat_t)

section("DAWG — batch_search_with_prefix")
seq_t, seq_r = bench(lambda: [dawg.search_with_prefix(p) for p in PREFIXES])
bat_t, _     = bench(lambda: dawg.batch_search_with_prefix(PREFIXES))
total = sum(len(r) for r in seq_r)
row(f"prefix ×{len(PREFIXES)}  ({total} hits)", seq_t, bat_t)

section("DAWG — batch_search  (wildcard)")
seq_t, seq_r = bench(lambda: [dawg.search(p) for p in PATTERNS])
bat_t, _     = bench(lambda: dawg.batch_search(PATTERNS))
total = sum(len(r) for r in seq_r)
row(f"wildcard ×{len(PATTERNS)}  ({total} hits)", seq_t, bat_t)

section("DAWG — batch_search_within_distance")
seq_t, seq_r = bench(lambda: [dawg.search_within_distance(w, LEV_DIST) for w in LEV_WORDS])
bat_t, _     = bench(lambda: dawg.batch_search_within_distance(LEV_WORDS, LEV_DIST))
total = sum(len(r) for r in seq_r)
row(f"levenshtein d={LEV_DIST} ×{len(LEV_WORDS)}  ({total} hits)", seq_t, bat_t)

print()
