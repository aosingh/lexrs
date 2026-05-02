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

    let tmp_path = format!("{snapshot_dir}/snapshot_{version}.tmp");
    let final_path = format!("{snapshot_dir}/snapshot_{version}.txt");

    let mut out = BufWriter::new(std::fs::File::create(&tmp_path)?);

    if let Some(path) = existing_path {
        let file = std::fs::File::open(path)?;
        let mut file_iter = BufReader::new(file)
            .lines()
            .filter_map(|l| l.ok())
            .filter_map(|l| {
                let l = l.trim().to_string();
                if l.is_empty() {
                    None
                } else {
                    parse_line(&l).ok()
                }
            })
            .peekable();

        let mut new_iter = new_words.iter().peekable();

        loop {
            let ord = match (file_iter.peek(), new_iter.peek()) {
                (None, None) => break,
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some((fw, _)), Some((nw, _))) => fw.as_str().cmp(nw.as_str()),
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
                    writeln!(out, "{w} {}", fc + *nc)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_line_with_count() {
        let (word, count) = parse_line("apple 5").unwrap();
        assert_eq!(word, "apple");
        assert_eq!(count, 5);
    }

    #[test]
    fn test_parse_line_without_count() {
        let (word, count) = parse_line("apple").unwrap();
        assert_eq!(word, "apple");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_parse_line_uses_last_space() {
        // rsplit_once splits on the last space
        let (word, count) = parse_line("hello world 3").unwrap();
        assert_eq!(word, "hello world");
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_merge_no_existing_snapshot() {
        let dir = tempdir().unwrap();
        let snap_dir = dir.path().to_str().unwrap();
        let new_words = vec![("apple".to_string(), 3usize), ("apply".to_string(), 1usize)];

        merge_and_write(snap_dir, 1, None, &new_words)
            .await
            .unwrap();

        let content = fs::read_to_string(format!("{snap_dir}/snapshot_1.txt")).unwrap();
        assert_eq!(content, "apple 3\napply 1\n");
    }

    #[tokio::test]
    async fn test_merge_with_existing_snapshot() {
        let dir = tempdir().unwrap();
        let snap_dir = dir.path().to_str().unwrap();
        let existing = format!("{snap_dir}/snapshot_1.txt");
        fs::write(&existing, "apple 3\nbanana 2\n").unwrap();

        let new_words = vec![
            ("apply".to_string(), 1usize),
            ("cherry".to_string(), 4usize),
        ];
        merge_and_write(snap_dir, 2, Some(&existing), &new_words)
            .await
            .unwrap();

        let content = fs::read_to_string(format!("{snap_dir}/snapshot_2.txt")).unwrap();
        assert_eq!(content, "apple 3\napply 1\nbanana 2\ncherry 4\n");
    }

    #[tokio::test]
    async fn test_merge_sums_duplicate_counts() {
        let dir = tempdir().unwrap();
        let snap_dir = dir.path().to_str().unwrap();
        let existing = format!("{snap_dir}/snapshot_1.txt");
        fs::write(&existing, "apple 3\nbanana 2\n").unwrap();

        let new_words = vec![("apple".to_string(), 7usize)];
        merge_and_write(snap_dir, 2, Some(&existing), &new_words)
            .await
            .unwrap();

        let content = fs::read_to_string(format!("{snap_dir}/snapshot_2.txt")).unwrap();
        assert_eq!(content, "apple 10\nbanana 2\n");
    }

    #[tokio::test]
    async fn test_load_snapshot() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot_1.txt");
        fs::write(&path, "apple 3\napply 1\napt 2\n").unwrap();

        let dawg = load(path.to_str().unwrap()).await.unwrap();
        assert!(dawg.contains("apple"));
        assert!(dawg.contains("apply"));
        assert!(dawg.contains("apt"));
        assert!(!dawg.contains("banana"));
    }

    #[tokio::test]
    async fn test_load_empty_snapshot() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot_empty.txt");
        fs::write(&path, "").unwrap();

        let dawg = load(path.to_str().unwrap()).await.unwrap();
        assert_eq!(dawg.word_count(), 0);
    }
}
