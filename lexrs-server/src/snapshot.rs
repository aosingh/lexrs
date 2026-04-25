use std::io::{BufRead, BufReader, BufWriter, Write};

use lexrs::Dawg;

/// Write a new snapshot by merging an existing snapshot (if any) with new words.
/// Both sources are sorted — the merge is a streaming O(1)-memory zipper.
/// Uses .tmp + rename for atomic replacement.
pub async fn merge_and_write(
    snapshot_dir: &str,
    version: u64,
    existing_path: Option<&str>,
    new_words: &[(String, usize)],
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(snapshot_dir).await?;

    let tmp_path   = format!("{snapshot_dir}/snapshot_{version}.tmp");
    let final_path = format!("{snapshot_dir}/snapshot_{version}.txt");

    let mut out = BufWriter::new(std::fs::File::create(&tmp_path)?);

    if let Some(path) = existing_path {
        let file = std::fs::File::open(path)?;
        let mut file_iter = BufReader::new(file)
            .lines()
            .filter_map(|l| l.ok())
            .filter_map(|l| {
                let l = l.trim().to_string();
                if l.is_empty() { None } else { parse_line(&l).ok() }
            })
            .peekable();

        let mut new_iter = new_words.iter()
            .map(|(w, c)| (w.clone(), *c))
            .peekable();

        loop {
            let ord = match (file_iter.peek(), new_iter.peek()) {
                (None, None)             => break,
                (Some(_), None)          => std::cmp::Ordering::Less,
                (None, Some(_))          => std::cmp::Ordering::Greater,
                (Some((fw, _)), Some((nw, _))) => fw.cmp(nw),
            };
            match ord {
                std::cmp::Ordering::Less => {
                    let (w, c) = file_iter.next().unwrap();
                    writeln!(out, "{w} {c}")?;
                }
                std::cmp::Ordering::Greater => {
                    let (w, c) = new_iter.next().unwrap();
                    writeln!(out, "{w} {c}")?;
                }
                std::cmp::Ordering::Equal => {
                    let (w, fc) = file_iter.next().unwrap();
                    let (_, nc) = new_iter.next().unwrap();
                    writeln!(out, "{w} {}", fc + nc)?;
                }
            }
        }
    } else {
        for (w, c) in new_words {
            writeln!(out, "{w} {c}")?;
        }
    }

    out.flush()?;
    drop(out);
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
