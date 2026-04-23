"""
Benchmark: lexpy (Pure Python) vs lexrs (Rust-backed)
Compares Trie and DAWG insertion speed and search speed.
"""

import time
import random
import sys

# ── helpers ───────────────────────────────────────────────────────────────────

def load_words(path="/usr/share/dict/words"):
    with open(path) as f:
        return [line.strip() for line in f if line.strip().isalpha()]

def fmt(seconds):
    ms = seconds * 1000
    if ms < 0.01:
        return f"{seconds * 1_000_000:.2f} µs"
    return f"{ms:.2f} ms"

def speedup(py_t, rs_t):
    if rs_t == 0:
        return "∞"
    return f"{py_t / rs_t:.1f}x"

def bench(fn, repeat=5):
    """Run fn() `repeat` times, return (best_time, last_result)."""
    result = None
    best = float("inf")
    for _ in range(repeat):
        t0 = time.perf_counter()
        result = fn()
        best = min(best, time.perf_counter() - t0)
    return best, result

def section(title):
    print("\n" + "=" * 70)
    print(f"{title:^70}")
    print("=" * 70)

# ── imports ───────────────────────────────────────────────────────────────────

from lexpy.trie import Trie as PyTrie
from lexpy.dawg import DAWG as PyDAWG
from lexrs import Trie as RsTrie, DAWG as RsDAWG

# ── dataset ───────────────────────────────────────────────────────────────────

ALL_WORDS_SORTED = load_words()
random.seed(42)

SIZES = [1_000, 10_000, 50_000, 100_000]

# Search benchmarks use a random 50k sample spread across the alphabet
SEARCH_SIZE = 50_000
search_words = random.sample(ALL_WORDS_SORTED, SEARCH_SIZE)

LOOKUP_WORDS = random.sample(search_words, 500)   # all present
MISS_WORDS   = [w + "zzz" for w in LOOKUP_WORDS]  # all absent

PREFIXES  = ["a", "pre", "un", "str", "com"]
PATTERNS  = ["a*", "str*", "*ing", "un?*", "????"]
LEV_CASES = [("python", 1), ("python", 2), ("search", 1), ("search", 2)]

# ── trie builders ─────────────────────────────────────────────────────────────

def build_py_trie(words):
    t = PyTrie(); t.add_all(words); return t

def build_rs_trie(words):
    t = RsTrie(); t.add_all(words); return t

def build_py_dawg(words):
    d = PyDAWG(); d.add_all(words); return d

def build_rs_dawg(words):
    d = RsDAWG(); d.add_all(words); return d

# ── generic bench runners ─────────────────────────────────────────────────────

HDR_INSERT = f"{'Words':>10}  {'lexpy':>12}  {'lexrs':>12}  {'Speedup':>10}"
HDR_SEARCH = f"{'':>14}  {'lexpy':>12}  {'lexrs':>12}  {'Speedup':>10}  results"
SEP = "-" * 70

def run_insertion(label, build_py, build_rs):
    section(f"INSERTION — {label}")
    print(HDR_INSERT)
    print(SEP)
    for n in SIZES:
        words = ALL_WORDS_SORTED[:n]
        py_t, _ = bench(lambda w=words: build_py(w))
        rs_t, _ = bench(lambda w=words: build_rs(w))
        print(f"{n:>10,}  {fmt(py_t):>12}  {fmt(rs_t):>12}  {speedup(py_t, rs_t):>10}")

def run_lookup(label, py_struct, rs_struct):
    section(f"EXACT LOOKUP — {label}  (500 words, built on 50k)")
    print(f"{'Variant':>18}  {'lexpy':>12}  {'lexrs':>12}  {'Speedup':>10}")
    print(SEP)
    for variant, words in [("hit (present)", LOOKUP_WORDS), ("miss (absent)", MISS_WORDS)]:
        py_t, _ = bench(lambda ws=words: [w in py_struct for w in ws])
        rs_t, _ = bench(lambda ws=words: [w in rs_struct for w in ws])
        print(f"{variant:>18}  {fmt(py_t):>12}  {fmt(rs_t):>12}  {speedup(py_t, rs_t):>10}")

def run_prefix(label, py_struct, rs_struct):
    section(f"PREFIX SEARCH — {label}  (built on 50k)")
    print(HDR_SEARCH)
    print(SEP)
    for prefix in PREFIXES:
        py_t, py_r = bench(lambda p=prefix: py_struct.search_with_prefix(p))
        rs_t, _    = bench(lambda p=prefix: rs_struct.search_with_prefix(p))
        print(f"  {prefix!r:>12}  {fmt(py_t):>12}  {fmt(rs_t):>12}  {speedup(py_t, rs_t):>10}  {len(py_r)}")

def run_wildcard(label, py_struct, rs_struct):
    section(f"WILDCARD SEARCH — {label}  (built on 50k)")
    print(HDR_SEARCH)
    print(SEP)
    for pat in PATTERNS:
        py_t, py_r = bench(lambda p=pat: py_struct.search(p))
        rs_t, _    = bench(lambda p=pat: rs_struct.search(p))
        print(f"  {pat!r:>12}  {fmt(py_t):>12}  {fmt(rs_t):>12}  {speedup(py_t, rs_t):>10}  {len(py_r)}")

def run_levenshtein(label, py_struct, rs_struct):
    section(f"LEVENSHTEIN SEARCH — {label}  (built on 50k)")
    print(HDR_SEARCH)
    print(SEP)
    for word, dist in LEV_CASES:
        py_t, py_r = bench(lambda w=word, d=dist: py_struct.search_within_distance(w, d))
        rs_t, _    = bench(lambda w=word, d=dist: rs_struct.search_within_distance(w, d))
        label2 = f"{word!r} d={dist}"
        print(f"  {label2:>12}  {fmt(py_t):>12}  {fmt(rs_t):>12}  {speedup(py_t, rs_t):>10}  {len(py_r)}")

# ── main ──────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    import lexpy
    print(f"\nlexpy  version : {lexpy.__version__}")
    print(f"lexrs  version : 0.1.0 (Rust/PyO3)")
    print(f"word corpus    : {len(ALL_WORDS_SORTED):,} words  (/usr/share/dict/words, alpha-only)")
    print(f"Python         : {sys.version.split()[0]}")

    # ── Trie ──────────────────────────────────────────────────────────────────
    print("\n\n★  TRIE  ★")
    run_insertion("Trie", build_py_trie, build_rs_trie)

    py_trie = build_py_trie(search_words)
    rs_trie = build_rs_trie(search_words)
    run_lookup    ("Trie", py_trie, rs_trie)
    run_prefix    ("Trie", py_trie, rs_trie)
    run_wildcard  ("Trie", py_trie, rs_trie)
    run_levenshtein("Trie", py_trie, rs_trie)

    # ── DAWG ──────────────────────────────────────────────────────────────────
    print("\n\n★  DAWG  ★")
    run_insertion("DAWG", build_py_dawg, build_rs_dawg)

    py_dawg = build_py_dawg(search_words)
    rs_dawg = build_rs_dawg(search_words)
    run_lookup    ("DAWG", py_dawg, rs_dawg)
    run_prefix    ("DAWG", py_dawg, rs_dawg)
    run_wildcard  ("DAWG", py_dawg, rs_dawg)
    run_levenshtein("DAWG", py_dawg, rs_dawg)

    print()
