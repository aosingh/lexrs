"""
Workload benchmark: lexpy (Pure Python) vs lexrs (Rust-backed)

Designed around realistic application scenarios where traversal work
dominates over per-call overhead — the conditions where lexrs shines.

Scenarios:
  1. Index build          — insert the full system dictionary (235k words)
  2. Autocomplete engine  — 1 000 prefix queries, each returning many results
  3. Spell checker        — 300 fuzzy queries (Levenshtein d=2) across full index
  4. Pattern matcher      — broad wildcard patterns that traverse the whole trie
  5. Full index traversal — enumerate every word in the structure via search("*")
"""

import time
import random
import sys
import string

# ── helpers ───────────────────────────────────────────────────────────────────

def load_words(path="/usr/share/dict/words"):
    with open(path) as f:
        return [w.strip() for w in f if w.strip().isalpha()]

def fmt(seconds):
    ms = seconds * 1000
    return f"{ms:7.1f} ms" if ms >= 0.1 else f"{seconds*1e6:7.1f} µs"

def speedup(py_t, rs_t):
    if rs_t == 0:
        return "   ∞  "
    r = py_t / rs_t
    arrow = "▲" if r >= 1 else "▼"
    return f"{arrow} {r:.1f}x"

def bench(fn, repeat=3):
    best = float("inf")
    result = None
    for _ in range(repeat):
        t0 = time.perf_counter()
        result = fn()
        best = min(best, time.perf_counter() - t0)
    return best, result

def header(title):
    print(f"\n{'═'*68}")
    print(f"  {title}")
    print(f"{'═'*68}")
    print(f"  {'lexpy':>14}  {'lexrs':>14}  {'Speedup':>10}  note")
    print(f"  {'-'*62}")

def row(label, py_t, rs_t, note=""):
    print(f"  {fmt(py_t):>14}  {fmt(rs_t):>14}  {speedup(py_t, rs_t):>10}  {label}  {note}")

# ── imports ───────────────────────────────────────────────────────────────────

from lexpy.trie import Trie as PyTrie
from lexpy.dawg import DAWG as PyDAWG
from lexrs import Trie as RsTrie, DAWG as RsDAWG

# ── corpus ────────────────────────────────────────────────────────────────────

ALL_WORDS = load_words()           # ~235k, already sorted
random.seed(42)

# ── workload helpers ──────────────────────────────────────────────────────────

def random_prefixes(words, n, min_len=2, max_len=4):
    """Pick n random short prefixes that actually exist in words."""
    candidates = {w[:l] for w in words for l in range(min_len, max_len + 1)}
    return random.sample(sorted(candidates), min(n, len(candidates)))

def make_typos(words, n):
    """
    Generate n realistic misspellings of words:
    delete one char, swap two adjacent chars, or substitute one char.
    """
    pool = random.sample(words, n * 3)
    typos = []
    for w in pool:
        if len(w) < 3:
            continue
        kind = random.choice(["delete", "swap", "sub"])
        i = random.randint(0, len(w) - 1)
        if kind == "delete":
            typos.append(w[:i] + w[i+1:])
        elif kind == "swap" and i < len(w) - 1:
            lst = list(w); lst[i], lst[i+1] = lst[i+1], lst[i]; typos.append("".join(lst))
        else:
            typos.append(w[:i] + random.choice(string.ascii_lowercase) + w[i+1:])
        if len(typos) == n:
            break
    return typos

# ── scenario 1: full index build ─────────────────────────────────────────────

def scenario_build():
    header("SCENARIO 1 — Full Index Build  (235k words)")

    for label, build_py, build_rs in [
        ("Trie", lambda: (PyTrie(), PyTrie().add_all(ALL_WORDS)),
                 lambda: (RsTrie(), RsTrie().add_all(ALL_WORDS))),
        ("DAWG", lambda: (PyDAWG(), PyDAWG().add_all(ALL_WORDS)),
                 lambda: (RsDAWG(), RsDAWG().add_all(ALL_WORDS))),
    ]:
        # cleaner lambdas
        if label == "Trie":
            py_fn = lambda: [PyTrie().add_all(ALL_WORDS)]
            rs_fn = lambda: [RsTrie().add_all(ALL_WORDS)]
        else:
            py_fn = lambda: [PyDAWG().add_all(ALL_WORDS)]
            rs_fn = lambda: [RsDAWG().add_all(ALL_WORDS)]

        py_t, _ = bench(py_fn, repeat=3)
        rs_t, _ = bench(rs_fn, repeat=3)
        row(label, py_t, rs_t, f"add_all({len(ALL_WORDS):,} words)")

# ── scenario 2: autocomplete engine ──────────────────────────────────────────

def scenario_autocomplete(py_trie, rs_trie, py_dawg, rs_dawg):
    header("SCENARIO 2 — Autocomplete Engine  (1 000 prefix queries)")

    prefixes = random_prefixes(ALL_WORDS, 1000, min_len=2, max_len=4)

    for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
        py_t, py_r = bench(lambda s=py_s: [s.search_with_prefix(p) for p in prefixes])
        rs_t, rs_r = bench(lambda s=rs_s: [s.search_with_prefix(p) for p in prefixes])
        total = sum(len(x) for x in py_r)
        row(label, py_t, rs_t, f"{total:,} results total")

# ── scenario 3: spell checker ─────────────────────────────────────────────────

def scenario_spellcheck(py_trie, rs_trie, py_dawg, rs_dawg):
    header("SCENARIO 3 — Spell Checker  (300 fuzzy queries, d=2)")

    typos = make_typos(ALL_WORDS, 300)

    for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
        py_t, py_r = bench(lambda s=py_s: [s.search_within_distance(w, 2) for w in typos])
        rs_t, _    = bench(lambda s=rs_s: [s.search_within_distance(w, 2) for w in typos])
        total = sum(len(x) for x in py_r)
        row(label, py_t, rs_t, f"{len(typos)} queries → {total:,} suggestions")

# ── scenario 4: broad wildcard patterns ──────────────────────────────────────

def scenario_patterns(py_trie, rs_trie, py_dawg, rs_dawg):
    header("SCENARIO 4 — Pattern Matcher  (broad wildcards, many results)")

    patterns = [
        ("*ing",   "suffix match"),
        ("*tion",  "suffix match"),
        ("*ness",  "suffix match"),
        ("un*",    "prefix match"),
        ("*a*e*",  "multi-wildcard"),
        ("????",   "exact-length 4"),
        ("?????",  "exact-length 5"),
        ("??????", "exact-length 6"),
    ]

    for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
        py_t, py_r = bench(lambda s=py_s: [s.search(p) for p, _ in patterns])
        rs_t, _    = bench(lambda s=rs_s: [s.search(p) for p, _ in patterns])
        total = sum(len(x) for x in py_r)
        row(label, py_t, rs_t, f"{len(patterns)} patterns → {total:,} results")

    # also show per-pattern detail for Trie
    print()
    print(f"  {'Pattern':<12}  {'lexpy':>12}  {'lexrs':>12}  {'Speedup':>10}  results")
    print(f"  {'-'*58}")
    py_t2 = build_py_trie_full
    rs_t2 = build_rs_trie_full
    for pat, desc in patterns:
        py_t, py_r = bench(lambda p=pat: py_t2.search(p))
        rs_t, _    = bench(lambda p=pat: rs_t2.search(p))
        print(f"  {pat:<12}  {fmt(py_t):>12}  {fmt(rs_t):>12}  {speedup(py_t, rs_t):>10}  {len(py_r):>6}  {desc}")

# ── scenario 5: full traversal ────────────────────────────────────────────────

def scenario_traversal(py_trie, rs_trie, py_dawg, rs_dawg):
    header("SCENARIO 5 — Full Index Traversal  (enumerate all words via search('*'))")

    for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
        py_t, py_r = bench(lambda s=py_s: s.search("*"))
        rs_t, _    = bench(lambda s=rs_s: s.search("*"))
        row(label, py_t, rs_t, f"{len(py_r):,} words enumerated")

# ── build full-corpus structures (reused across scenarios) ────────────────────

def build_full():
    print("\nBuilding full-corpus structures (235k words)... ", end="", flush=True)
    py_trie = PyTrie(); py_trie.add_all(ALL_WORDS)
    rs_trie = RsTrie(); rs_trie.add_all(ALL_WORDS)
    py_dawg = PyDAWG(); py_dawg.add_all(ALL_WORDS)
    rs_dawg = RsDAWG(); rs_dawg.add_all(ALL_WORDS)
    print("done.")
    return py_trie, rs_trie, py_dawg, rs_dawg

# ── main ──────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    import lexpy
    print(f"\n{'─'*68}")
    print(f"  lexpy  : {lexpy.__version__} (Pure Python Trie/DAWG)")
    print(f"  lexrs  : 0.1.0  (Rust + PyO3)")
    print(f"  corpus : {len(ALL_WORDS):,} words  (/usr/share/dict/words, alpha-only)")
    print(f"  Python : {sys.version.split()[0]}")
    print(f"  Note   : best of 3 runs, ▲ = lexrs faster, ▼ = lexpy faster")
    print(f"{'─'*68}")

    scenario_build()

    py_trie, rs_trie, py_dawg, rs_dawg = build_full()

    # expose these for the per-pattern detail in scenario 4
    global build_py_trie_full, build_rs_trie_full
    build_py_trie_full = py_trie
    build_rs_trie_full = rs_trie

    scenario_autocomplete(py_trie, rs_trie, py_dawg, rs_dawg)
    scenario_spellcheck  (py_trie, rs_trie, py_dawg, rs_dawg)
    scenario_patterns    (py_trie, rs_trie, py_dawg, rs_dawg)
    scenario_traversal   (py_trie, rs_trie, py_dawg, rs_dawg)

    print()
