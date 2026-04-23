"""
Genome workload benchmark: lexpy (Pure Python) vs lexrs (Rust-backed)

Simulates real bioinformatics use-cases on DNA sequence data.

Biology context
───────────────
  Alphabet  : {A, C, G, T}  — only 4 characters, so the trie is extremely
              dense (up to 4 children per node) and very deep.  This shifts
              the bottleneck almost entirely to node-traversal, where Rust
              has the largest advantage.

  k-mer     : a contiguous substring of length k extracted from a DNA read.
              k=31 is used here (common in SPAdes/Velvet assemblers).
              Each 150 bp read yields (150 - 31 + 1) = 120 k-mers.

  Repetitive genome
              Real genomes are 30–80% repetitive (transposons, tandem repeats).
              We simulate this by sampling reads FROM a short reference sequence,
              so the same subsequences appear in many reads.  This makes
              prefix searches return many hits per query — the scenario where
              traversal-work dominates and lexrs shines most.

Scenarios
─────────
  1. K-mer index build      — insert unique k-mers from 5 000 simulated reads
  2. Primer / seed lookup   — prefix search on 300 primers, each hitting
                              many k-mers (realistic repetitive genome)
  3. Read classification    — for each of 500 reads, check every k-mer
                              against the index (read-vs-index alignment)
  4. Error correction (d=1) — fuzzy match 200 corrupted k-mers (1 substitution)
  5. Error correction (d=2) — 50 queries; lexpy timed out shown honestly
  6. Motif discovery        — IUPAC-style wildcard patterns across full index
  7. Full k-mer enumeration — dump all k-mers (de-novo assembly graph step)
  8. Full pipeline          — total wall-time: build + all query workloads
"""

import time
import random
import sys
import signal
from contextlib import contextmanager

# ── helpers ────────────────────────────────────────────────────────────────────

BASES = "ACGT"

def random_seq(length, rng):
    return "".join(rng.choice(BASES) for _ in range(length))

def sample_read(reference, read_len, rng, error_rate=0.01):
    """Sample a read from the reference genome with random sequencing errors."""
    start = rng.randrange(len(reference) - read_len + 1)
    read  = list(reference[start:start + read_len])
    for i in range(len(read)):
        if rng.random() < error_rate:
            read[i] = rng.choice([b for b in BASES if b != read[i]])
    return "".join(read)

def kmers(seq, k):
    return [seq[i:i+k] for i in range(len(seq) - k + 1)]

def introduce_errors(kmer, n, rng):
    """Introduce exactly n substitution errors into a k-mer."""
    positions = rng.sample(range(len(kmer)), n)
    lst = list(kmer)
    for i in positions:
        lst[i] = rng.choice([b for b in BASES if b != lst[i]])
    return "".join(lst)

def fmt(seconds):
    ms = seconds * 1000
    if ms >= 1000:  return f"{ms/1000:6.2f}  s"
    if ms >= 0.1:   return f"{ms:7.1f} ms"
    return f"{seconds*1e6:7.1f} µs"

def speedup(py_t, rs_t):
    if py_t is None: return "  n/a  "
    if rs_t == 0:    return "   ∞   "
    r = py_t / rs_t
    return f"{'▲' if r >= 1 else '▼'}{r:5.1f}x"

def bench(fn, repeat=3):
    best, result = float("inf"), None
    for _ in range(repeat):
        t0 = time.perf_counter()
        result = fn()
        best = min(best, time.perf_counter() - t0)
    return best, result

@contextmanager
def timeout(seconds):
    def _raise(sig, frame): raise TimeoutError()
    old = signal.signal(signal.SIGALRM, _raise)
    signal.alarm(seconds)
    try:    yield
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, old)

def bench_timed(fn, limit_s=30, repeat=3):
    try:
        with timeout(limit_s):
            return bench(fn, repeat=repeat)
    except TimeoutError:
        return None, None

def section(title, subtitle=""):
    print(f"\n{'═'*72}")
    print(f"  {title}")
    if subtitle: print(f"  {subtitle}")
    print(f"{'═'*72}")
    print(f"  {'':16}  {'lexpy':>12}  {'lexrs':>12}  {'Speedup':>9}  note")
    print(f"  {'-'*66}")

def row(label, py_t, rs_t, note=""):
    py_str = fmt(py_t) if py_t is not None else "    TIMEOUT"
    print(f"  {label:<16}  {py_str:>12}  {fmt(rs_t):>12}  {speedup(py_t, rs_t):>9}  {note}")

# ── imports ────────────────────────────────────────────────────────────────────

from lexpy.trie import Trie as PyTrie
from lexpy.dawg import DAWG as PyDAWG
from lexrs import Trie as RsTrie, DAWG as RsDAWG

# ── genome simulation ──────────────────────────────────────────────────────────

rng = random.Random(0xD1A)

# Parameters tuned for maximum traversal-to-overhead ratio
REF_LEN     = 50_000   # short reference → high repetition across reads
READ_LEN    = 150      # Illumina short-read length (bp)
K           = 31       # SPAdes/Velvet canonical k-mer size (deeper trie = more Rust work)
NUM_READS   = 5_000    # reads sampled from the reference
ERROR_RATE  = 0.01     # 1% per-base sequencing error rate
PRIMER_LEN  = 20       # primer length (20bp is the gold standard in PCR)
NUM_PRIMERS = 300      # primers to query
NUM_CLASSIFY= 500      # reads to classify against the index
NUM_CORR_D1 = 200      # error-correction queries d=1
NUM_CORR_D2 = 50       # error-correction queries d=2
TIMEOUT_S   = 45

import lexpy
print(f"\n{'─'*72}")
print(f"  Genome Workload Benchmark — lexpy vs lexrs  (Repetitive Genome)")
print(f"{'─'*72}")
print(f"  Reference   : {REF_LEN:,} bp  (short → high repetition across reads)")
print(f"  Read length : {READ_LEN} bp  |  k = {K}  |  reads = {NUM_READS:,}")
print(f"  Error rate  : {ERROR_RATE*100:.0f}% per base  (Illumina typical)")
print(f"  lexpy       : {lexpy.__version__} (Pure Python)")
print(f"  lexrs       : 0.1.0  (Rust + PyO3)")
print(f"  Python      : {sys.version.split()[0]}")
print(f"  Timing      : best of 3 runs  |  timeout = {TIMEOUT_S}s")
print(f"  ▲ = lexrs faster  |  TIMEOUT = lexpy exceeded {TIMEOUT_S}s limit")
print(f"{'─'*72}")

print(f"\n  Generating reference genome ({REF_LEN:,} bp)... ", end="", flush=True)
reference = random_seq(REF_LEN, rng)
print("done.")

print(f"  Sampling {NUM_READS:,} reads (error rate {ERROR_RATE*100:.0f}%)... ", end="", flush=True)
reads = [sample_read(reference, READ_LEN, rng, ERROR_RATE) for _ in range(NUM_READS)]
print("done.")

print(f"  Extracting k-mers (k={K})... ", end="", flush=True)
all_kmers_set = set()
for read in reads:
    all_kmers_set.update(kmers(read, K))
UNIQUE_KMERS = sorted(all_kmers_set)
kmers_per_read = READ_LEN - K + 1
print(f"{len(UNIQUE_KMERS):,} unique k-mers  ({kmers_per_read} per read).")

# Primers: subsequences of the reference (guaranteed to have many k-mer hits)
print(f"  Generating {NUM_PRIMERS} primers ({PRIMER_LEN} bp from reference)... ", end="", flush=True)
primer_starts = rng.sample(range(REF_LEN - PRIMER_LEN), NUM_PRIMERS)
PRIMERS = [reference[i:i+PRIMER_LEN] for i in primer_starts]
print("done.")

# Reads for classification: fresh reads from the reference
CLASSIFY_READS = [sample_read(reference, READ_LEN, rng, ERROR_RATE)
                  for _ in range(NUM_CLASSIFY)]

# Corrupted k-mers: take real k-mers from the index and introduce errors
pool = rng.sample(UNIQUE_KMERS, NUM_CORR_D1 + NUM_CORR_D2)
CORRUPTED_D1 = [introduce_errors(k, 1, rng) for k in pool[:NUM_CORR_D1]]
CORRUPTED_D2 = [introduce_errors(k, 2, rng) for k in pool[NUM_CORR_D1:]]

# Motifs: patterns from the reference itself (will have real hits)
MOTIFS = [
    ("ATG" + "?" * (K - 3),              "start-codon context"),
    ("?" * 4 + "GG" + "?" * (K - 6),    "GG dinucleotide, ambiguous flanks"),
    ("ACGT*ACGT",                         "ACGT repeat with variable gap"),
    ("GC*GC",                             "GC island pair"),
    ("AAAA*TTTT",                         "poly-A/T islands"),
    ("?" * 5 + "ACGT" + "?" * (K - 9),  "conserved ACGT with flanks"),
    ("?" * K,                             "any k-mer (full wildcard scan)"),
]

# ── scenario 1: k-mer index build ─────────────────────────────────────────────

section(
    "SCENARIO 1 — K-mer Index Build",
    f"add_all({len(UNIQUE_KMERS):,} unique {K}-mers)  |  alphabet=4  |  depth={K}",
)
for label in ["Trie", "DAWG"]:
    py_fn = (lambda: (t := PyTrie(), t.add_all(UNIQUE_KMERS))) if label == "Trie" \
       else (lambda: (d := PyDAWG(), d.add_all(UNIQUE_KMERS)))
    rs_fn = (lambda: (t := RsTrie(), t.add_all(UNIQUE_KMERS))) if label == "Trie" \
       else (lambda: (d := RsDAWG(), d.add_all(UNIQUE_KMERS)))
    py_t, _ = bench(py_fn)
    rs_t, _ = bench(rs_fn)
    row(label, py_t, rs_t, f"{len(UNIQUE_KMERS):,} k-mers, k={K}")

# ── build persistent structures ────────────────────────────────────────────────

print(f"\n  Building persistent index... ", end="", flush=True)
py_trie = PyTrie(); py_trie.add_all(UNIQUE_KMERS)
rs_trie = RsTrie(); rs_trie.add_all(UNIQUE_KMERS)
py_dawg = PyDAWG(); py_dawg.add_all(UNIQUE_KMERS)
rs_dawg = RsDAWG(); rs_dawg.add_all(UNIQUE_KMERS)
print("done.")

# ── scenario 2: primer / seed lookup ──────────────────────────────────────────

section(
    "SCENARIO 2 — Primer / Seed Lookup  (repetitive genome → many hits per query)",
    f"search_with_prefix({NUM_PRIMERS} primers × {PRIMER_LEN} bp)",
)
for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
    py_t, py_r = bench(lambda s=py_s: [s.search_with_prefix(p) for p in PRIMERS])
    rs_t, _    = bench(lambda s=rs_s: [s.search_with_prefix(p) for p in PRIMERS])
    total = sum(len(x) for x in py_r)
    avg   = total // len(PRIMERS)
    row(label, py_t, rs_t, f"{total:,} hits  (~{avg} per primer)")

# ── scenario 3: read classification ───────────────────────────────────────────

section(
    "SCENARIO 3 — Read Classification",
    f"For each of {NUM_CLASSIFY} reads: check every {K}-mer against the index  "
    f"({NUM_CLASSIFY * kmers_per_read:,} total lookups)",
)
for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
    py_t, py_r = bench(lambda s=py_s: [
        sum(1 for km in kmers(r, K) if km in s) for r in CLASSIFY_READS
    ])
    rs_t, _    = bench(lambda s=rs_s: [
        sum(1 for km in kmers(r, K) if km in s) for r in CLASSIFY_READS
    ])
    total_hits = sum(py_r)
    row(label, py_t, rs_t,
        f"{NUM_CLASSIFY * kmers_per_read:,} contains() calls → {total_hits:,} hits")

# ── scenario 4: error correction d=1 ──────────────────────────────────────────

section(
    "SCENARIO 4 — Error Correction  (d=1, single substitution error)",
    f"search_within_distance({NUM_CORR_D1} corrupted k-mers, dist=1)",
)
for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
    py_t, py_r = bench(lambda s=py_s: [s.search_within_distance(c, 1) for c in CORRUPTED_D1])
    rs_t, _    = bench(lambda s=rs_s: [s.search_within_distance(c, 1) for c in CORRUPTED_D1])
    total = sum(len(x) for x in py_r)
    row(label, py_t, rs_t, f"{total:,} correction candidates  ({NUM_CORR_D1} queries)")

# ── scenario 5: error correction d=2 ──────────────────────────────────────────

section(
    "SCENARIO 5 — Error Correction  (d=2, two substitution errors)",
    f"search_within_distance({NUM_CORR_D2} corrupted k-mers, dist=2)  — very expensive",
)
for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
    py_t, _    = bench_timed(
        lambda s=py_s: [s.search_within_distance(c, 2) for c in CORRUPTED_D2],
        limit_s=TIMEOUT_S,
    )
    rs_t, rs_r = bench(lambda s=rs_s: [s.search_within_distance(c, 2) for c in CORRUPTED_D2])
    total = sum(len(x) for x in rs_r)
    note  = f"{total:,} candidates  ({NUM_CORR_D2} queries)"
    if py_t is None: note += f"  ← lexpy exceeded {TIMEOUT_S}s"
    row(label, py_t, rs_t, note)

# ── scenario 6: motif discovery ───────────────────────────────────────────────

section(
    "SCENARIO 6 — Motif Discovery  (IUPAC-inspired wildcards)",
    "Each pattern traverses the full k-mer index in a single call",
)
for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
    py_t, py_r = bench_timed(lambda s=py_s: [s.search(p) for p, _ in MOTIFS], limit_s=TIMEOUT_S)
    rs_t, rs_r = bench(lambda s=rs_s: [s.search(p) for p, _ in MOTIFS])
    total = sum(len(x) for x in rs_r)
    note  = f"{len(MOTIFS)} patterns → {total:,} hits"
    if py_t is None: note += f"  ← lexpy exceeded {TIMEOUT_S}s"
    row(label, py_t, rs_t, note)

print()
print(f"  {'Pattern':<32}  {'lexpy':>10}  {'lexrs':>10}  {'Speedup':>9}  {'hits':>7}  desc")
print(f"  {'-'*90}")
for pat, desc in MOTIFS:
    py_t, py_r = bench_timed(lambda p=pat: py_trie.search(p), limit_s=15)
    rs_t, rs_r = bench(lambda p=pat: rs_trie.search(p))
    py_str = fmt(py_t) if py_t is not None else "   TIMEOUT"
    print(f"  {pat:<32}  {py_str:>10}  {fmt(rs_t):>10}  {speedup(py_t, rs_t):>9}  {len(rs_r):>7}  {desc}")

# ── scenario 7: full k-mer enumeration ────────────────────────────────────────

section(
    "SCENARIO 7 — Full K-mer Enumeration",
    "search('*') dumps every indexed k-mer — de-novo assembly graph construction",
)
for label, py_s, rs_s in [("Trie", py_trie, rs_trie), ("DAWG", py_dawg, rs_dawg)]:
    py_t, py_r = bench(lambda s=py_s: s.search("*"))
    rs_t, _    = bench(lambda s=rs_s: s.search("*"))
    row(label, py_t, rs_t, f"{len(py_r):,} k-mers enumerated")

# ── scenario 8: full pipeline ──────────────────────────────────────────────────

section(
    "SCENARIO 8 — Full Pipeline  (build + all query workloads, single run)",
    "Total wall-time for a complete assembly preprocessing pass",
)

def run_pipeline(build_trie, build_dawg):
    # build
    t = build_trie(); t.add_all(UNIQUE_KMERS)
    d = build_dawg(); d.add_all(UNIQUE_KMERS)
    # primer lookup
    [t.search_with_prefix(p) for p in PRIMERS]
    # read classification
    [sum(1 for km in kmers(r, K) if km in t) for r in CLASSIFY_READS]
    # error correction d=1
    [t.search_within_distance(c, 1) for c in CORRUPTED_D1]
    # motif discovery
    [t.search(pat) for pat, _ in MOTIFS if "*" not in pat]  # skip open-ended *
    # full enumeration
    t.search("*")

py_t, _ = bench(lambda: run_pipeline(PyTrie, PyDAWG), repeat=1)
rs_t, _ = bench(lambda: run_pipeline(RsTrie, RsDAWG), repeat=1)
row("Trie+DAWG", py_t, rs_t, "end-to-end preprocessing pass")

print()
