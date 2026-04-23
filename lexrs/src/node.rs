use std::collections::BTreeMap;

/// A node in the arena-allocated FSA (Trie or DAWG).
/// Children are stored as `BTreeMap<char, usize>` where the value is
/// the index of the child node in the arena (`Vec<Node>`).
/// BTreeMap guarantees sorted iteration order, which is required for
/// stable DAWG node signatures.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: usize,
    pub val: char,
    pub children: BTreeMap<char, usize>,
    pub eow: bool,
    pub count: usize,
}

impl Node {
    pub fn new(id: usize, val: char) -> Self {
        Node {
            id,
            val,
            children: BTreeMap::new(),
            eow: false,
            count: 0,
        }
    }

    /// Compute the signature string used for DAWG node deduplication.
    /// Format: val + count + ("1"|"0") + for each (ch, child_id): ch + child_id
    /// The `nodes` arena is needed to look up child IDs.
    pub fn signature(&self, nodes: &[Node]) -> String {
        let mut s = String::new();
        s.push(self.val);
        s.push_str(&self.count.to_string());
        s.push(if self.eow { '1' } else { '0' });
        for (&ch, &child_idx) in &self.children {
            s.push(ch);
            s.push_str(&nodes[child_idx].id.to_string());
        }
        s
    }
}
