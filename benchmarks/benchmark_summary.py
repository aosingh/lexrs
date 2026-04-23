"""
Three-way genome benchmark summary: lexpy vs lexrs vs Pure Rust.

Runs both benchmarks, parses their timing output, and prints a single
side-by-side table with speedup columns.
"""

import subprocess, re, sys, time, os

# ── run sub-benchmarks ─────────────────────────────────────────────────────────

def run(cmd):
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    result = subprocess.run(cmd, capture_output=True, text=True, cwd=root)
    if result.returncode != 0:
        print(f"ERROR:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)
    return result.stdout

# ── time helpers ───────────────────────────────────────────────────────────────

_TIME_RE = re.compile(r"([\d.]+)\s*(µs|ms|s)\b")

def to_ms(s: str):
    m = _TIME_RE.search(s)
    if not m or "TIMEOUT" in s:
        return None
    val, unit = float(m.group(1)), m.group(2)
    return {"µs": val / 1000, "ms": val, "s": val * 1000}[unit]

def fmt(ms):
    if ms is None: return "TIMEOUT"
    if ms >= 1000:  return f"{ms/1000:.2f} s"
    if ms >= 0.1:   return f"{ms:.1f} ms"
    return f"{ms*1000:.1f} µs"

def sx(base, fast):
    if base is None or fast is None or fast == 0: return "n/a"
    return f"{base/fast:.1f}x"

# ── parsers ────────────────────────────────────────────────────────────────────

def parse_python(text: str):
    """
    Python benchmark rows look like:
      Trie                  12.45  s       2.64  s    ▲  4.7x  ...
    Two time values per row: first = lexpy, second = lexrs.
    Returns {(scenario_num, label): (lexpy_ms, lexrs_ms)}
    """
    data = {}
    scenario = None
    for line in text.splitlines():
        m = re.search(r"SCENARIO\s+(\d+)", line)
        if m:
            scenario = int(m.group(1))
            continue
        if scenario is None:
            continue
        # match: 2-space indent, label, then two time tokens
        m = re.match(r"^\s{2}(\S.+?)\s{2,}", line)
        if not m:
            continue
        label = m.group(1).strip()
        times = _TIME_RE.findall(line)
        if len(times) >= 2:
            t1 = to_ms(f"{times[0][0]} {times[0][1]}")
            t2 = to_ms(f"{times[1][0]} {times[1][1]}")
            data[(scenario, label)] = (t1, t2)
    return data

def parse_rust(text: str):
    """
    Rust benchmark rows look like:
      Trie                  442.6 ms  ...
    One time value per row.
    Returns {(scenario_num, label): rust_ms}
    """
    data = {}
    scenario = None
    for line in text.splitlines():
        m = re.search(r"SCENARIO\s+(\d+)", line)
        if m:
            scenario = int(m.group(1))
            continue
        if scenario is None:
            continue
        m = re.match(r"^\s{2}(\S.+?)\s{2,}", line)
        if not m:
            continue
        label = m.group(1).strip()
        times = _TIME_RE.findall(line)
        if times:
            data[(scenario, label)] = to_ms(f"{times[0][0]} {times[0][1]}")
    return data

def find_py(data, scenario, label_prefix):
    for (s, l), v in data.items():
        if s == scenario and l.lower().startswith(label_prefix.lower()):
            return v
    return (None, None)

def find_rs(data, scenario, label_prefix):
    for (s, l), v in data.items():
        if s == scenario and l.lower().startswith(label_prefix.lower()):
            return v
    return None

# ── scenarios ──────────────────────────────────────────────────────────────────
#   (display name, scenario#, py label prefix, rust label prefix)

ROWS = [
    ("Trie build",           1, "Trie",      "Trie"),
    ("DAWG build",           1, "DAWG",      "DAWG"),
    ("Primer lookup (Trie)", 2, "Trie",      "Trie"),
    ("Primer lookup (DAWG)", 2, "DAWG",      "DAWG"),
    ("Read classification",  3, "Trie",      "Trie"),
    ("Error correct  d=1",   4, "Trie",      "Trie"),
    ("Error correct  d=2",   5, "Trie",      "Trie"),
    ("Motif discovery",      6, "Trie",      "Trie (all)"),
    ("Full enumeration",     7, "Trie",      "Trie"),
    ("Full pipeline",        8, "Trie+DAWG", "Trie+DAWG"),
]

# ── main ──────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    print("\nRunning Python benchmark (lexpy + lexrs)...", flush=True)
    t0 = time.perf_counter()
    py_out = run([sys.executable, "benchmarks/benchmark_genome.py"])
    print(f"  done in {time.perf_counter()-t0:.0f}s")

    print("Running Rust benchmark (pure Rust)...", flush=True)
    t0 = time.perf_counter()
    rs_out = run(["cargo", "run", "--release", "--bin", "genome_bench", "--quiet"])
    print(f"  done in {time.perf_counter()-t0:.0f}s")

    py_data = parse_python(py_out)
    rs_data = parse_rust(rs_out)

    W = 88
    print(f"\n{'═'*W}")
    print(f"  {'GENOME BENCHMARK — THREE-WAY COMPARISON':^{W-4}}")
    print(f"  {'lexpy (Pure Python)  vs  lexrs (Rust+PyO3)  vs  Pure Rust':^{W-4}}")
    print(f"{'═'*W}")
    hdr = (f"  {'Scenario':<24}  {'lexpy':>10}  {'lexrs':>10}  {'Pure Rust':>10}"
           f"  {'Rust/lexpy':>12}  {'Rust/lexrs':>12}")
    print(hdr)
    print(f"  {'-'*(W-4)}")

    for display, snum, py_lbl, rs_lbl in ROWS:
        lexpy_ms, lexrs_ms = find_py(py_data, snum, py_lbl)
        rust_ms             = find_rs(rs_data, snum, rs_lbl)
        print(f"  {display:<24}  {fmt(lexpy_ms):>10}  {fmt(lexrs_ms):>10}"
              f"  {fmt(rust_ms):>10}"
              f"  {sx(lexpy_ms, rust_ms):>12}"
              f"  {sx(lexrs_ms, rust_ms):>12}")

    print(f"{'═'*W}")
    print(f"  Speedup: Pure Rust ÷ lexpy  and  Pure Rust ÷ lexrs  (higher = Rust faster)")
    print()
