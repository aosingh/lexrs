use crate::error::LexError;
use crate::node::Node;
use crate::utils::{read_lines_from_file, validate_expression};

/// Standard Trie implementation using arena allocation.
/// All nodes are stored in a `Vec<Node>`; children are indices into that vec.
pub struct Trie {
    nodes: Vec<Node>,
    num_of_words: usize,
}

impl Trie {
    pub fn new() -> Self {
        let root = Node::new(0, '\0');
        Trie {
            nodes: vec![root],
            num_of_words: 0,
        }
    }

    /// Total number of nodes (including root).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of distinct words stored.
    pub fn word_count(&self) -> usize {
        self.num_of_words
    }

    /// Returns true if `word` exists in the trie.
    pub fn contains(&self, word: &str) -> bool {
        if word.is_empty() {
            return true;
        }
        let mut node_idx = 0;
        let chars: Vec<char> = word.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            match self.nodes[node_idx].children.get(&ch) {
                Some(&next_idx) => {
                    node_idx = next_idx;
                    if i == chars.len() - 1 && self.nodes[node_idx].eow {
                        return true;
                    }
                }
                None => return false,
            }
        }
        false
    }

    /// Returns true if `prefix` is a prefix of any word in the trie.
    pub fn contains_prefix(&self, prefix: &str) -> bool {
        self.prefix_node(prefix).is_some()
    }

    /// Internal: returns the node index at the end of `prefix`, or None.
    fn prefix_node(&self, prefix: &str) -> Option<usize> {
        if prefix.is_empty() {
            return Some(0);
        }
        let mut node_idx = 0;
        for ch in prefix.chars() {
            match self.nodes[node_idx].children.get(&ch) {
                Some(&next_idx) => node_idx = next_idx,
                None => return None,
            }
        }
        Some(node_idx)
    }

    /// Add a word with an optional count (default 1).
    pub fn add(&mut self, word: &str, count: usize) -> Result<(), LexError> {
        let mut node_idx = 0;
        let chars: Vec<char> = word.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            if !self.nodes[node_idx].children.contains_key(&ch) {
                let new_id = self.nodes.len();
                let new_node = Node::new(new_id, ch);
                self.nodes.push(new_node);
                self.nodes[node_idx].children.insert(ch, new_id);
            }
            let next_idx = *self.nodes[node_idx].children.get(&ch).unwrap();
            node_idx = next_idx;
            if i == chars.len() - 1 {
                self.nodes[node_idx].eow = true;
                self.nodes[node_idx].count += count;
                self.num_of_words += count;
            }
        }
        Ok(())
    }

    /// Add all words from an iterator.
    pub fn add_all<I: IntoIterator<Item = String>>(&mut self, words: I) -> Result<(), LexError> {
        for word in words {
            self.add(&word, 1)?;
        }
        Ok(())
    }

    /// Add all words from a file (one word per line).
    pub fn add_from_file(&mut self, path: &str) -> Result<(), LexError> {
        let lines = read_lines_from_file(path)?;
        for word in lines {
            self.add(&word, 1)?;
        }
        Ok(())
    }

    /// Return all words matching the wildcard pattern.
    /// `?` matches exactly one character; `*` matches zero or more.
    pub fn search(&self, pattern: &str) -> Result<Vec<String>, LexError> {
        if pattern.is_empty() {
            return Ok(vec![]);
        }
        let pattern = validate_expression(pattern);
        let pat_chars: Vec<char> = pattern.chars().collect();
        let mut results = Vec::new();
        let mut current = String::new();
        words_with_wildcard(&self.nodes, 0, &pat_chars, 0, &mut current, &mut results);
        Ok(results.into_iter().map(|(w, _)| w).collect())
    }

    /// Like `search` but also returns word counts.
    pub fn search_with_count(&self, pattern: &str) -> Result<Vec<(String, usize)>, LexError> {
        if pattern.is_empty() {
            return Ok(vec![]);
        }
        let pattern = validate_expression(pattern);
        let pat_chars: Vec<char> = pattern.chars().collect();
        let mut results = Vec::new();
        let mut current = String::new();
        words_with_wildcard(&self.nodes, 0, &pat_chars, 0, &mut current, &mut results);
        Ok(results)
    }

    /// Return all words with the given prefix.
    pub fn search_with_prefix(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() {
            return vec![];
        }
        match self.prefix_node(prefix) {
            None => vec![],
            Some(node_idx) => {
                let pat = ['*'];
                let mut results = Vec::new();
                let mut current = prefix.to_string();
                words_with_wildcard(&self.nodes, node_idx, &pat, 0, &mut current, &mut results);
                results.into_iter().map(|(w, _)| w).collect()
            }
        }
    }

    /// Like `search_with_prefix` but also returns counts.
    pub fn search_with_prefix_count(&self, prefix: &str) -> Vec<(String, usize)> {
        if prefix.is_empty() {
            return vec![];
        }
        match self.prefix_node(prefix) {
            None => vec![],
            Some(node_idx) => {
                let pat = ['*'];
                let mut results = Vec::new();
                let mut current = prefix.to_string();
                words_with_wildcard(&self.nodes, node_idx, &pat, 0, &mut current, &mut results);
                results
            }
        }
    }

    /// Return all words within Levenshtein `dist` of `word`.
    pub fn search_within_distance(&self, word: &str, dist: usize) -> Vec<String> {
        let target: Vec<char> = word.chars().collect();
        let row: Vec<usize> = (0..=target.len()).collect();
        let mut results = Vec::new();
        for (&ch, &child_idx) in &self.nodes[0].children {
            let mut current_word = ch.to_string();
            search_within_distance_inner(
                &self.nodes,
                child_idx,
                &target,
                ch,
                &mut current_word,
                &row,
                dist,
                &mut results,
            );
        }
        results.into_iter().map(|(w, _)| w).collect()
    }

    /// Like `search_within_distance` but also returns counts.
    pub fn search_within_distance_count(&self, word: &str, dist: usize) -> Vec<(String, usize)> {
        let target: Vec<char> = word.chars().collect();
        let row: Vec<usize> = (0..=target.len()).collect();
        let mut results = Vec::new();
        for (&ch, &child_idx) in &self.nodes[0].children {
            let mut current_word = ch.to_string();
            search_within_distance_inner(
                &self.nodes,
                child_idx,
                &target,
                ch,
                &mut current_word,
                &row,
                dist,
                &mut results,
            );
        }
        results
    }
}

impl Default for Trie {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursive wildcard search over the arena.
/// `node_idx` is the current node; `index` is position in `pattern`.
/// `current` accumulates the word built so far (passed by mutable reference for efficiency).
pub(crate) fn words_with_wildcard(
    nodes: &[Node],
    node_idx: usize,
    pattern: &[char],
    index: usize,
    current: &mut String,
    results: &mut Vec<(String, usize)>,
) {
    let node = &nodes[node_idx];

    // If we've consumed the whole pattern and this is an end-of-word node, emit
    if node.eow && index >= pattern.len() && !current.is_empty() {
        results.push((current.clone(), node.count));
        // Don't return yet — `*` at end can still need to check children below
    }

    if index >= pattern.len() {
        return;
    }

    match pattern[index] {
        '?' => {
            // Match exactly one character — recurse into each child
            for (&ch, &child_idx) in &node.children {
                current.push(ch);
                words_with_wildcard(nodes, child_idx, pattern, index + 1, current, results);
                current.pop();
            }
        }
        '*' => {
            // Skip the `*` (match zero chars at this level)
            words_with_wildcard(nodes, node_idx, pattern, index + 1, current, results);

            // Consume one char via each child, staying at same `*` index
            for (&ch, &child_idx) in &node.children {
                current.push(ch);
                words_with_wildcard(nodes, child_idx, pattern, index, current, results);
                current.pop();
            }
        }
        literal => {
            if let Some(&child_idx) = node.children.get(&literal) {
                current.push(literal);
                words_with_wildcard(nodes, child_idx, pattern, index + 1, current, results);
                current.pop();
            }
        }
    }
}

/// Recursive Levenshtein distance search.
/// `prev_row` is the DP row from the parent call.
pub(crate) fn search_within_distance_inner(
    nodes: &[Node],
    node_idx: usize,
    target: &[char],
    letter: char,
    current_word: &mut String,
    prev_row: &[usize],
    dist: usize,
    results: &mut Vec<(String, usize)>,
) {
    let cols = target.len() + 1;
    let mut curr_row = Vec::with_capacity(cols);
    curr_row.push(prev_row[0] + 1);

    for col in 1..cols {
        let insert_cost = curr_row[col - 1] + 1;
        let delete_cost = prev_row[col] + 1;
        let replace_cost = if target[col - 1] == letter {
            prev_row[col - 1]
        } else {
            prev_row[col - 1] + 1
        };
        curr_row.push(insert_cost.min(delete_cost).min(replace_cost));
    }

    let node = &nodes[node_idx];

    if *curr_row.last().unwrap() <= dist && node.eow {
        results.push((current_word.clone(), node.count));
    }

    if *curr_row.iter().min().unwrap() <= dist {
        for (&ch, &child_idx) in &node.children {
            current_word.push(ch);
            search_within_distance_inner(
                nodes,
                child_idx,
                target,
                ch,
                current_word,
                &curr_row,
                dist,
                results,
            );
            current_word.pop();
        }
    }
}
