use std::io::{BufRead, BufReader, Write};

use lexrs::Dawg;

/// Write a sorted (word, count) list to /snapshots/snapshot_<version>.txt atomically.
/// Uses a .tmp file + rename to avoid readers seeing a partial write.
pub async fn write(
    snapshot_dir: &str,
    version: u64,
    words: &[(String, usize)],
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(snapshot_dir).await?;

    let tmp_path  = format!("{snapshot_dir}/snapshot_{version}.tmp");
    let final_path = format!("{snapshot_dir}/snapshot_{version}.txt");

    // Write to temp file
    let mut file = std::fs::File::create(&tmp_path)?;
    for (word, count) in words {
        writeln!(file, "{word} {count}")?;
    }
    file.flush()?;
    drop(file);

    // Atomic rename — readers either see the old file or the complete new one
    std::fs::rename(&tmp_path, &final_path)?;

    Ok(())
}

/// Load a snapshot file into a new DAWG.
/// File format: one "word count" per line, already sorted.
pub async fn load(path: &str) -> Result<Dawg, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut dawg = Dawg::new();
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (word, count) = parse_line(line)?;
        dawg.add(&word, count).map_err(|e| e.to_string())?;
    }
    dawg.reduce();

    Ok(dawg)
}

fn parse_line(line: &str) -> Result<(String, usize), String> {
    match line.rsplit_once(' ') {
        Some((word, count_str)) => {
            let count = count_str.parse::<usize>().map_err(|e| e.to_string())?;
            Ok((word.to_string(), count))
        }
        None => Ok((line.to_string(), 1)),
    }
}
