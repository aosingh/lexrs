/// Pure-Rust genome workload benchmark.
///
/// Mirrors the Python benchmark_genome.py scenarios exactly so results
/// can be placed side-by-side:
///
///   lexpy  (Pure Python)
///   lexrs  (Rust via PyO3)   ← from the Python benchmark
///   Rust   (this binary)     ← zero Python overhead, zero PyO3 boundary
///
/// Usage
/// ─────
///   cargo run --release --bin genome_bench            # defaults
///   cargo run --release --bin genome_bench -- --help  # show all params
///
/// All parameters are controlled via environment variables:
///
///   REF_LEN       Reference genome length in bp          (default: 50000)
///   READ_LEN      Simulated read length in bp            (default: 150)
///   K             k-mer size                             (default: 31)
///   NUM_READS     Number of simulated reads              (default: 5000)
///   ERROR_RATE    Per-base sequencing error rate         (default: 0.01)
///   NUM_PRIMERS   Number of primer queries               (default: 300)
///   PRIMER_LEN    Primer length in bp                    (default: 20)
///   NUM_CLASSIFY  Reads used for classification          (default: 500)
///   NUM_CORR_D1   Error-correction queries at d=1        (default: 200)
///   NUM_CORR_D2   Error-correction queries at d=2        (default: 50)
///   REPEAT        Benchmark repetitions (best-of-N)      (default: 3)
///   SEED          RNG seed (hex or decimal)              (default: 0xD1A)
///
/// Examples
/// ────────
///   # Quick smoke test
///   NUM_READS=500 K=21 cargo run --release --bin genome_bench
///
///   # Large-scale run
///   REF_LEN=500000 NUM_READS=50000 cargo run --release --bin genome_bench
///
///   # Stress error-correction only (increase query count)
///   NUM_CORR_D1=1000 NUM_CORR_D2=200 cargo run --release --bin genome_bench
use std::collections::HashSet;
use std::time::{Duration, Instant};

use lexrs::dawg::Dawg;
use lexrs::trie::Trie;

// ── tiny seeded PRNG (xorshift64) ─────────────────────────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn next_usize(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ── DNA helpers ───────────────────────────────────────────────────────────────

const BASES: [char; 4] = ['A', 'C', 'G', 'T'];

fn random_seq(len: usize, rng: &mut Rng) -> String {
    (0..len).map(|_| BASES[rng.next_usize(4)]).collect()
}

fn sample_read(reference: &[char], read_len: usize, rng: &mut Rng, error_rate: f64) -> String {
    let start = rng.next_usize(reference.len() - read_len + 1);
    reference[start..start + read_len]
        .iter()
        .map(|&b| {
            if rng.next_f64() < error_rate {
                let alt = rng.next_usize(3);
                let others: Vec<char> = BASES.iter().copied().filter(|&c| c != b).collect();
                others[alt % 3]
            } else {
                b
            }
        })
        .collect()
}

fn kmers(seq: &str, k: usize) -> Vec<String> {
    let chars: Vec<char> = seq.chars().collect();
    (0..=chars.len().saturating_sub(k))
        .map(|i| chars[i..i + k].iter().collect())
        .collect()
}

fn introduce_errors(kmer: &str, n: usize, rng: &mut Rng) -> String {
    let mut chars: Vec<char> = kmer.chars().collect();
    let len = chars.len();
    // pick n distinct positions
    let mut positions: Vec<usize> = (0..len).collect();
    for i in 0..n {
        let j = i + rng.next_usize(len - i);
        positions.swap(i, j);
    }
    for &pos in &positions[..n] {
        let orig = chars[pos];
        let others: Vec<char> = BASES.iter().copied().filter(|&c| c != orig).collect();
        chars[pos] = others[rng.next_usize(3)];
    }
    chars.iter().collect()
}

// ── timing helpers ────────────────────────────────────────────────────────────

fn fmt(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms >= 1000.0 {
        format!("{:6.2}  s", ms / 1000.0)
    } else if ms >= 0.1 {
        format!("{:7.1} ms", ms)
    } else {
        format!("{:7.1} µs", d.as_secs_f64() * 1e6)
    }
}

/// Run `f` `repeat` times and return the best duration.
fn bench<F, R>(mut f: F, repeat: usize) -> (Duration, R)
where
    F: FnMut() -> R,
{
    let mut best = Duration::MAX;
    let mut last_result = None;
    for _ in 0..repeat {
        let t0 = Instant::now();
        let result = f();
        let elapsed = t0.elapsed();
        if elapsed < best {
            best = elapsed;
        }
        last_result = Some(result);
    }
    (best, last_result.unwrap())
}

fn section(title: &str, subtitle: &str) {
    println!("\n{}", "═".repeat(72));
    println!("  {title}");
    if !subtitle.is_empty() {
        println!("  {subtitle}");
    }
    println!("{}", "═".repeat(72));
    println!("  {:<16}  {:>12}  note", "", "Pure Rust");
    println!("  {}", "-".repeat(50));
}

fn row(label: &str, t: Duration, note: &str) {
    println!("  {label:<16}  {:>12}  {note}", fmt(t));
}

// ── env-var helpers ───────────────────────────────────────────────────────────

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| {
            // accept "0x..." hex or plain decimal
            if v.starts_with("0x") || v.starts_with("0X") {
                u64::from_str_radix(&v[2..], 16).ok()
            } else {
                v.parse().ok()
            }
        })
        .unwrap_or(default)
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    // print help and exit if --help / -h passed
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        print!(
            "{}",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/bin/genome_bench.rs"
            ))
            .lines()
            .skip(3) // skip the first //! / /// line
            .take_while(|l| l.starts_with("///"))
            .map(|l| l.trim_start_matches("///").trim_start_matches(' '))
            .collect::<Vec<_>>()
            .join("\n")
        );
        println!();
        return;
    }

    let ref_len = env_usize("REF_LEN", 50_000);
    let read_len = env_usize("READ_LEN", 150);
    let k = env_usize("K", 31);
    let num_reads = env_usize("NUM_READS", 5_000);
    let error_rate = env_f64("ERROR_RATE", 0.01);
    let num_primers = env_usize("NUM_PRIMERS", 300);
    let primer_len = env_usize("PRIMER_LEN", 20);
    let num_corr_d1 = env_usize("NUM_CORR_D1", 200);
    let num_corr_d2 = env_usize("NUM_CORR_D2", 50);
    let num_classify = env_usize("NUM_CLASSIFY", 500);
    let repeat = env_usize("REPEAT", 3);
    let seed = env_u64("SEED", 0xD1A);

    println!("\n{}", "─".repeat(72));
    println!("  Genome Workload Benchmark — Pure Rust");
    println!("{}", "─".repeat(72));
    println!("  Reference   : {ref_len} bp  (REF_LEN)");
    println!(
        "  Read length : {read_len} bp  (READ_LEN)  |  k = {k}  (K)  |  reads = {num_reads}  (NUM_READS)"
    );
    println!(
        "  Error rate  : {:.1}%  (ERROR_RATE)  |  seed = {seed:#x}  (SEED)",
        error_rate * 100.0
    );
    println!("  Primers     : {num_primers}  (NUM_PRIMERS)  ×  {primer_len} bp  (PRIMER_LEN)");
    println!("  Timing      : best of {repeat} runs  (REPEAT)");
    println!("  Override any parameter via environment variable, e.g.:");
    println!("    K=21 NUM_READS=1000 cargo run --release --bin genome_bench");
    println!("{}", "─".repeat(72));

    let mut rng = Rng::new(seed);

    // ── generate data ─────────────────────────────────────────────────────────

    print!("\n  Generating reference genome ({ref_len} bp)... ");
    let ref_chars: Vec<char> = random_seq(ref_len, &mut rng).chars().collect();
    println!("done.");

    print!("  Sampling {num_reads} reads... ");
    let reads: Vec<String> = (0..num_reads)
        .map(|_| sample_read(&ref_chars, read_len, &mut rng, error_rate))
        .collect();
    println!("done.");

    print!("  Extracting k-mers (k={k})... ");
    let all_kmer_set: HashSet<String> = reads.iter().flat_map(|r| kmers(r, k)).collect();
    let mut unique_kmers: Vec<String> = all_kmer_set.into_iter().collect();
    unique_kmers.sort();
    let kmers_per_read = read_len - k + 1;
    println!(
        "{} unique k-mers  ({kmers_per_read} per read).",
        unique_kmers.len()
    );

    // Primers: take first `primer_len` chars of positions in reference
    let primer_starts: Vec<usize> = (0..num_primers)
        .map(|_| rng.next_usize(ref_len - primer_len))
        .collect();
    let primers: Vec<String> = primer_starts
        .iter()
        .map(|&s| ref_chars[s..s + primer_len].iter().collect())
        .collect();

    // Classify reads
    let classify_reads: Vec<String> = (0..num_classify)
        .map(|_| sample_read(&ref_chars, read_len, &mut rng, error_rate))
        .collect();

    // Corrupted k-mers
    let pool: Vec<String> = {
        let mut idx: Vec<usize> = (0..unique_kmers.len()).collect();
        for i in 0..num_corr_d1 + num_corr_d2 {
            let j = i + rng.next_usize(unique_kmers.len() - i);
            idx.swap(i, j);
        }
        idx[..num_corr_d1 + num_corr_d2]
            .iter()
            .map(|&i| unique_kmers[i].clone())
            .collect()
    };
    let corrupted_d1: Vec<String> = pool[..num_corr_d1]
        .iter()
        .map(|k| introduce_errors(k, 1, &mut rng))
        .collect();
    let corrupted_d2: Vec<String> = pool[num_corr_d1..]
        .iter()
        .map(|k| introduce_errors(k, 2, &mut rng))
        .collect();

    let motifs: &[(&str, &str)] = &[
        (
            "ATG????????????????????????????????????",
            "start-codon context",
        ),
        (
            "????GG?????????????????????????",
            "GG dinucleotide, ambiguous flanks",
        ),
        ("ACGT*ACGT", "ACGT repeat with variable gap"),
        ("GC*GC", "GC island pair"),
        ("AAAA*TTTT", "poly-A/T islands"),
        (
            "?????ACGT??????????????????????",
            "conserved ACGT with flanks",
        ),
        ("*", "any k-mer (full wildcard scan)"),
    ];

    // ── scenario 1: k-mer index build ────────────────────────────────────────

    section(
        "SCENARIO 1 — K-mer Index Build",
        &format!(
            "add_all({} unique {k}-mers)  |  alphabet=4  |  depth={k}",
            unique_kmers.len()
        ),
    );

    let (t, _) = bench(
        || {
            let mut t = Trie::new();
            t.add_all(unique_kmers.iter().cloned()).unwrap();
            t
        },
        repeat,
    );
    row("Trie", t, &format!("{} k-mers, k={k}", unique_kmers.len()));

    let (t, _) = bench(
        || {
            let mut d = Dawg::new();
            d.add_all(unique_kmers.iter().cloned()).unwrap();
            d
        },
        repeat,
    );
    row("DAWG", t, &format!("{} k-mers, k={k}", unique_kmers.len()));

    // ── build persistent structures ───────────────────────────────────────────

    print!("\n  Building persistent index... ");
    let mut trie = Trie::new();
    trie.add_all(unique_kmers.iter().cloned()).unwrap();
    let mut dawg = Dawg::new();
    dawg.add_all(unique_kmers.iter().cloned()).unwrap();
    println!("done.");

    // ── scenario 2: primer / seed lookup ─────────────────────────────────────

    section(
        "SCENARIO 2 — Primer / Seed Lookup",
        &format!("search_with_prefix({num_primers} primers × {primer_len} bp)"),
    );

    let (t, results) = bench(
        || {
            primers
                .iter()
                .map(|p| trie.search_with_prefix(p))
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "Trie",
        t,
        &format!("{total} hits  (~{} per primer)", total / num_primers.max(1)),
    );

    let (t, results) = bench(
        || {
            primers
                .iter()
                .map(|p| dawg.search_with_prefix(p))
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "DAWG",
        t,
        &format!("{total} hits  (~{} per primer)", total / num_primers.max(1)),
    );

    // ── scenario 3: read classification ──────────────────────────────────────

    section(
        "SCENARIO 3 — Read Classification",
        &format!(
            "contains() for every {k}-mer in {num_classify} reads  ({} total calls)",
            num_classify * kmers_per_read
        ),
    );

    let (t, results) = bench(
        || {
            classify_reads
                .iter()
                .map(|r| kmers(r, k).iter().filter(|km| trie.contains(km)).count())
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().sum();
    row(
        "Trie",
        t,
        &format!("{} calls → {total} hits", num_classify * kmers_per_read),
    );

    let (t, results) = bench(
        || {
            classify_reads
                .iter()
                .map(|r| kmers(r, k).iter().filter(|km| dawg.contains(km)).count())
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().sum();
    row(
        "DAWG",
        t,
        &format!("{} calls → {total} hits", num_classify * kmers_per_read),
    );

    // ── scenario 4: error correction d=1 ─────────────────────────────────────

    section(
        "SCENARIO 4 — Error Correction (d=1)",
        &format!("search_within_distance({num_corr_d1} corrupted k-mers, dist=1)"),
    );

    let (t, results) = bench(
        || {
            corrupted_d1
                .iter()
                .map(|c| trie.search_within_distance(c, 1))
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "Trie",
        t,
        &format!("{total} correction candidates  ({num_corr_d1} queries)"),
    );

    let (t, results) = bench(
        || {
            corrupted_d1
                .iter()
                .map(|c| dawg.search_within_distance(c, 1))
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "DAWG",
        t,
        &format!("{total} correction candidates  ({num_corr_d1} queries)"),
    );

    // ── scenario 5: error correction d=2 ─────────────────────────────────────

    section(
        "SCENARIO 5 — Error Correction (d=2)",
        &format!("search_within_distance({num_corr_d2} corrupted k-mers, dist=2)"),
    );

    let (t, results) = bench(
        || {
            corrupted_d2
                .iter()
                .map(|c| trie.search_within_distance(c, 2))
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "Trie",
        t,
        &format!("{total} candidates  ({num_corr_d2} queries)"),
    );

    let (t, results) = bench(
        || {
            corrupted_d2
                .iter()
                .map(|c| dawg.search_within_distance(c, 2))
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "DAWG",
        t,
        &format!("{total} candidates  ({num_corr_d2} queries)"),
    );

    // ── scenario 6: motif discovery ───────────────────────────────────────────

    section(
        "SCENARIO 6 — Motif Discovery",
        "IUPAC-inspired wildcard patterns across the full k-mer index",
    );

    let (t, results) = bench(
        || {
            motifs
                .iter()
                .map(|(p, _)| trie.search(p).unwrap())
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "Trie (all)",
        t,
        &format!("{} patterns → {total} hits", motifs.len()),
    );

    let (t, results) = bench(
        || {
            motifs
                .iter()
                .map(|(p, _)| dawg.search(p).unwrap())
                .collect::<Vec<_>>()
        },
        repeat,
    );
    let total: usize = results.iter().map(|r| r.len()).sum();
    row(
        "DAWG (all)",
        t,
        &format!("{} patterns → {total} hits", motifs.len()),
    );

    println!();
    println!(
        "  {:<40}  {:>10}  {:>7}  desc",
        "Pattern", "Pure Rust", "hits"
    );
    println!("  {}", "-".repeat(72));
    for (pat, desc) in motifs {
        let (t, r) = bench(|| trie.search(pat).unwrap(), 3);
        println!("  {pat:<40}  {:>10}  {:>7}  {desc}", fmt(t), r.len());
    }

    // ── scenario 7: full k-mer enumeration ───────────────────────────────────

    section(
        "SCENARIO 7 — Full K-mer Enumeration",
        "search('*') — de-novo assembly graph construction",
    );

    let (t, r) = bench(|| trie.search("*").unwrap(), 3);
    row("Trie", t, &format!("{} k-mers enumerated", r.len()));

    let (t, r) = bench(|| dawg.search("*").unwrap(), 3);
    row("DAWG", t, &format!("{} k-mers enumerated", r.len()));

    // ── scenario 8: full pipeline ─────────────────────────────────────────────

    section(
        "SCENARIO 8 — Full Pipeline",
        "Total wall-time: build + primer lookup + classify + error-correct + enumerate",
    );

    let (t, _) = bench(
        || {
            // build
            let mut t = Trie::new();
            t.add_all(unique_kmers.iter().cloned()).unwrap();
            let mut d = Dawg::new();
            d.add_all(unique_kmers.iter().cloned()).unwrap();
            // primer lookup
            for p in &primers {
                t.search_with_prefix(p);
            }
            // read classification
            for r in &classify_reads {
                kmers(r, k).iter().filter(|km| t.contains(km)).count();
            }
            // error correction d=1
            for c in &corrupted_d1 {
                t.search_within_distance(c, 1);
            }
            // motif discovery (no open-ended *)
            for (p, _) in motifs.iter().filter(|(p, _)| *p != "*") {
                t.search(p).unwrap();
            }
            // full enumeration
            t.search("*").unwrap();
        },
        repeat.min(1),
    );
    row("Trie+DAWG", t, "end-to-end preprocessing pass");

    println!();
}
