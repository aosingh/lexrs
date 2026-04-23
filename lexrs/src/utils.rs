use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

/// Normalize a wildcard pattern:
/// - Replace `*?`, `?*`, `**` (any mix) with a single `*`
/// - Collapse consecutive `?` into a single `?`
///
/// Mirrors Python's `validate_expression` in lexpy/_utils.py.
pub fn validate_expression(expr: &str) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Detect start of a wildcard sequence containing * (and maybe ?)
        if ch == '*' || (ch == '?' && i + 1 < chars.len() && chars[i + 1] == '*') {
            // Consume all consecutive * and ? that include at least one *
            let start = i;
            let mut has_star = ch == '*';
            while i < chars.len() && (chars[i] == '*' || chars[i] == '?') {
                if chars[i] == '*' {
                    has_star = true;
                }
                i += 1;
            }
            if has_star {
                result.push('*');
            } else {
                // Only ?s — but this branch shouldn't be reached given the outer condition
                result.push('?');
                let _ = start;
            }
        } else if ch == '?' {
            // Consecutive ?s collapse to one
            result.push('?');
            i += 1;
            while i < chars.len() && chars[i] == '?' {
                i += 1;
            }
        } else {
            result.push(ch);
            i += 1;
        }
    }

    result.into_iter().collect()
}

/// Read lines from a file, yielding each non-empty trimmed line.
pub fn read_lines_from_file<P: AsRef<Path>>(path: P) -> io::Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader
        .lines()
        .filter_map(|line| line.ok())
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_star_question() {
        assert_eq!(validate_expression("a*?"), "a*");
        assert_eq!(validate_expression("a?*"), "a*");
        assert_eq!(validate_expression("a**"), "a*");
        assert_eq!(validate_expression("a??"), "a?");
        assert_eq!(validate_expression("a*?b"), "a*b");
        assert_eq!(validate_expression("a?b"), "a?b");
        assert_eq!(validate_expression("abc"), "abc");
    }
}
