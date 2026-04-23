use std::collections::HashMap;

use crate::error::LexError;
use crate::node::Node;
use crate::trie::{search_within_distance_inner, words_with_wildcard};
use crate::utils::{read_lines_from_file, validate_expression};

/// Directed Acyclic Word Graph (DAWG) — a minimized Trie that compresses
/// shared suffixes as well as shared prefixes.
///
/// **Constraint:** words must be inserted in alphabetical (lexicographic) order.
/// Call `reduce()` after all insertions to finalize minimization.
pub struct Dawg {
    pub(crate) nodes: Vec<Node>,
    num_of_words: usize,
    prev_word: String,
    prev_node_idx: usize,
    /// Canonical nodes: signature string → node index in arena
    minimized_nodes: HashMap<String, usize>,
    /// Stack of nodes awaiting minimization: (parent_idx, edge_char, child_idx)
    unchecked_nodes: Vec<(usize, char, usize)>,
}

impl Dawg {
    pub fn new() -> Self {
        let root = Node::new(0, '\0');
        Dawg {
            nodes: vec![root],
            num_of_words: 0,
            prev_word: String::new(),
            prev_node_idx: 0,
            minimized_nodes: HashMap::new(),
            unchecked_nodes: Vec::new(),
        }
    }

    /// Number of minimized (canonical) nodes.
    pub fn node_count(&self) -> usize {
        self.minimized_nodes.len()
    }

    /// Number of distinct words stored.
    pub fn word_count(&self) -> usize {
        self.num_of_words
    }

    /// Insert a word (must be >= alphabetically to the previous word).
    pub fn add(&mut self, word: &str, count: usize) -> Result<(), LexError> {
        if word < self.prev_word.as_str() {
            return Err(LexError::OrderViolation {
                prev: self.prev_word.clone(),
                curr: word.to_string(),
            });
        }

        if word == self.prev_word.as_str() {
            self.nodes[self.prev_node_idx].count += count;
            self.num_of_words += count;
            self.prev_word = word.to_string();
            return Ok(());
        }

        // Find common prefix length with previous word
        let common_prefix_len = word
            .chars()
            .zip(self.prev_word.chars())
            .take_while(|(a, b)| a == b)
            .count();

        self._reduce(common_prefix_len);

        // Start building from: root (if unchecked is empty) or last unchecked child
        let mut node_idx = if self.unchecked_nodes.is_empty() {
            0 // root
        } else {
            self.unchecked_nodes.last().unwrap().2
        };

        let suffix: Vec<char> = word.chars().skip(common_prefix_len).collect();
        for &ch in &suffix {
            let new_id = self.nodes.len();
            let new_node = Node::new(new_id, ch);
            self.nodes.push(new_node);
            self.nodes[node_idx].children.insert(ch, new_id);
            self.unchecked_nodes.push((node_idx, ch, new_id));
            node_idx = new_id;
        }

        self.nodes[node_idx].eow = true;
        self.nodes[node_idx].count += count;
        self.prev_node_idx = node_idx;
        self.num_of_words += count;
        self.prev_word = word.to_string();
        Ok(())
    }

    /// Finalize minimization. Call this after all words have been inserted.
    pub fn reduce(&mut self) {
        self._reduce(0);
    }

    /// Internal minimization: process unchecked_nodes from end down to index `to`.
    fn _reduce(&mut self, to: usize) {
        let len = self.unchecked_nodes.len();
        for i in (to..len).rev() {
            let (parent_idx, letter, child_idx) = self.unchecked_nodes[i];
            let sig = self.nodes[child_idx].signature(&self.nodes);

            if !self.nodes[child_idx].children.is_empty() && self.minimized_nodes.contains_key(&sig)
            {
                // Replace with the canonical (already minimized) node
                let canonical_idx = self.minimized_nodes[&sig];
                self.nodes[parent_idx]
                    .children
                    .insert(letter, canonical_idx);
            } else {
                self.minimized_nodes.insert(sig, child_idx);
            }
        }
        self.unchecked_nodes.truncate(to);
    }

    /// Add all words from an iterator (sorts them first to satisfy alphabetical requirement).
    pub fn add_all<I: IntoIterator<Item = String>>(&mut self, words: I) -> Result<(), LexError> {
        let mut sorted: Vec<String> = words.into_iter().collect();
        sorted.sort();
        for word in sorted {
            self.add(&word, 1)?;
        }
        self.reduce();
        Ok(())
    }

    /// Add all words from a file (one word per line). File must be sorted or will be sorted.
    pub fn add_from_file(&mut self, path: &str) -> Result<(), LexError> {
        let lines: Vec<String> = read_lines_from_file(path)?.collect();
        // Check if already sorted; if not, sort
        let mut sorted = lines;
        sorted.sort();
        for word in sorted {
            self.add(&word, 1)?;
        }
        self.reduce();
        Ok(())
    }

    /// Returns true if `word` exists in the DAWG.
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

    /// Returns true if `prefix` is a prefix of any word in the DAWG.
    pub fn contains_prefix(&self, prefix: &str) -> bool {
        self.prefix_node(prefix).is_some()
    }

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

    /// Return all words matching the wildcard pattern.
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

impl Default for Dawg {
    fn default() -> Self {
        Self::new()
    }
}
