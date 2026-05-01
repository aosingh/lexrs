use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::dawg::Dawg;
use crate::trie::Trie;

// ── Trie ─────────────────────────────────────────────────────────────────────

/// A prefix tree (Trie) lexicon.
///
/// Supports any insertion order. Search time is O(W) where W is word length.
///
/// Example::
///
///     from lexrs import Trie
///     t = Trie()
///     t.add_all(["apple", "apply", "application"])
///     assert "apple" in t
///     print(t.search("app*"))
#[pyclass(name = "Trie")]
pub struct PyTrie {
    inner: Trie,
}

#[pymethods]
impl PyTrie {
    #[new]
    fn new() -> Self {
        PyTrie { inner: Trie::new() }
    }

    /// Returns True if word is present in the Trie.
    fn __contains__(&self, word: &str) -> bool {
        self.inner.contains(word)
    }

    /// Returns the number of nodes in the Trie.
    fn __len__(&self) -> usize {
        self.inner.node_count()
    }

    fn __repr__(&self) -> String {
        format!(
            "Trie(words={}, nodes={})",
            self.inner.word_count(),
            self.inner.node_count()
        )
    }

    /// Add a word with an optional count (default 1).
    #[pyo3(signature = (word, count=1))]
    fn add(&mut self, word: &str, count: usize) -> PyResult<()> {
        self.inner
            .add(word, count)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Add words from any Python iterable (list, set, tuple, generator).
    fn add_all(&mut self, source: &Bound<'_, PyAny>) -> PyResult<()> {
        let iter = source.try_iter()?;
        for item in iter {
            let word: String = item?.extract()?;
            self.inner
                .add(&word, 1)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
        }
        Ok(())
    }

    /// Add words from a file (one word per line).
    fn add_from_file(&mut self, path: &str) -> PyResult<()> {
        self.inner
            .add_from_file(path)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Returns True if prefix is a prefix of any word in the Trie.
    fn contains_prefix(&self, prefix: &str) -> bool {
        self.inner.contains_prefix(prefix)
    }

    /// Returns the number of words stored.
    fn get_word_count(&self) -> usize {
        self.inner.word_count()
    }

    /// Return all words matching the wildcard pattern.
    ///
    /// ``?`` matches exactly one character; ``*`` matches zero or more.
    /// If ``with_count=True``, returns ``list[(word, count)]``.
    #[pyo3(signature = (pattern, with_count=false))]
    fn search(&self, py: Python<'_>, pattern: &str, with_count: bool) -> PyResult<Py<PyAny>> {
        if with_count {
            let results = self
                .inner
                .search_with_count(pattern)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(results.into_pyobject(py)?.unbind().into_any())
        } else {
            let results = self
                .inner
                .search(pattern)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(results.into_pyobject(py)?.unbind().into_any())
        }
    }

    /// Return all words sharing the given prefix.
    ///
    /// If ``with_count=True``, returns ``list[(word, count)]``.
    #[pyo3(signature = (prefix, with_count=false))]
    fn search_with_prefix(
        &self,
        py: Python<'_>,
        prefix: &str,
        with_count: bool,
    ) -> PyResult<Py<PyAny>> {
        if with_count {
            Ok(self
                .inner
                .search_with_prefix_count(prefix)
                .into_pyobject(py)?
                .into())
        } else {
            Ok(self
                .inner
                .search_with_prefix(prefix)
                .into_pyobject(py)?
                .into())
        }
    }

    /// Return all words within Levenshtein ``dist`` of ``word``.
    ///
    /// If ``with_count=True``, returns ``list[(word, count)]``.
    #[pyo3(signature = (word, dist=0, with_count=false))]
    fn search_within_distance(
        &self,
        py: Python<'_>,
        word: &str,
        dist: usize,
        with_count: bool,
    ) -> PyResult<Py<PyAny>> {
        if with_count {
            Ok(self
                .inner
                .search_within_distance_count(word, dist)
                .into_pyobject(py)?
                .into())
        } else {
            Ok(self
                .inner
                .search_within_distance(word, dist)
                .into_pyobject(py)?
                .into())
        }
    }
}

// ── DAWG ─────────────────────────────────────────────────────────────────────

/// A Directed Acyclic Word Graph (DAWG) lexicon.
///
/// Compresses both shared prefixes and shared suffixes.
/// Words must be inserted in alphabetical order, or use ``add_all`` which
/// sorts automatically. Call ``reduce()`` after manual insertions.
///
/// Example::
///
///     from lexrs import DAWG
///     d = DAWG()
///     d.add_all(["ash", "ashes", "ashley"])
///     assert "ash" in d
///     print(d.search_with_prefix("ash"))
#[pyclass(name = "DAWG")]
pub struct PyDAWG {
    inner: Dawg,
}

#[pymethods]
impl PyDAWG {
    #[new]
    fn new() -> Self {
        PyDAWG { inner: Dawg::new() }
    }

    /// Returns True if word is present in the DAWG.
    fn __contains__(&self, word: &str) -> bool {
        self.inner.contains(word)
    }

    /// Returns the number of minimized nodes in the DAWG.
    fn __len__(&self) -> usize {
        self.inner.node_count()
    }

    fn __repr__(&self) -> String {
        format!(
            "DAWG(words={}, nodes={})",
            self.inner.word_count(),
            self.inner.node_count()
        )
    }

    /// Add a word (must be >= the previous word alphabetically).
    ///
    /// Call ``reduce()`` when done with manual insertions.
    #[pyo3(signature = (word, count=1))]
    fn add(&mut self, word: &str, count: usize) -> PyResult<()> {
        self.inner
            .add(word, count)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Add words from any Python iterable. Sorts automatically.
    fn add_all(&mut self, source: &Bound<'_, PyAny>) -> PyResult<()> {
        let iter = source.try_iter()?;
        let mut words: Vec<String> = iter
            .map(|item| item.and_then(|i| i.extract::<String>()))
            .collect::<PyResult<_>>()?;
        words.sort();
        for word in words {
            self.inner
                .add(&word, 1)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
        }
        self.inner.reduce();
        Ok(())
    }

    /// Add words from a file (one word per line). Sorts automatically.
    fn add_from_file(&mut self, path: &str) -> PyResult<()> {
        self.inner
            .add_from_file(path)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Finalize minimization. Call after all manual ``add()`` calls.
    fn reduce(&mut self) {
        self.inner.reduce();
    }

    /// Returns True if prefix is a prefix of any word in the DAWG.
    fn contains_prefix(&self, prefix: &str) -> bool {
        self.inner.contains_prefix(prefix)
    }

    /// Returns the number of words stored.
    fn get_word_count(&self) -> usize {
        self.inner.word_count()
    }

    /// Return all words matching the wildcard pattern.
    ///
    /// ``?`` matches exactly one character; ``*`` matches zero or more.
    /// If ``with_count=True``, returns ``list[(word, count)]``.
    #[pyo3(signature = (pattern, with_count=false))]
    fn search(&self, py: Python<'_>, pattern: &str, with_count: bool) -> PyResult<Py<PyAny>> {
        if with_count {
            let results = self
                .inner
                .search_with_count(pattern)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(results.into_pyobject(py)?.unbind().into_any())
        } else {
            let results = self
                .inner
                .search(pattern)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(results.into_pyobject(py)?.unbind().into_any())
        }
    }

    /// Return all words sharing the given prefix.
    ///
    /// If ``with_count=True``, returns ``list[(word, count)]``.
    #[pyo3(signature = (prefix, with_count=false))]
    fn search_with_prefix(
        &self,
        py: Python<'_>,
        prefix: &str,
        with_count: bool,
    ) -> PyResult<Py<PyAny>> {
        if with_count {
            Ok(self
                .inner
                .search_with_prefix_count(prefix)
                .into_pyobject(py)?
                .into())
        } else {
            Ok(self
                .inner
                .search_with_prefix(prefix)
                .into_pyobject(py)?
                .into())
        }
    }

    /// Return all words within Levenshtein ``dist`` of ``word``.
    ///
    /// If ``with_count=True``, returns ``list[(word, count)]``.
    #[pyo3(signature = (word, dist=0, with_count=false))]
    fn search_within_distance(
        &self,
        py: Python<'_>,
        word: &str,
        dist: usize,
        with_count: bool,
    ) -> PyResult<Py<PyAny>> {
        if with_count {
            Ok(self
                .inner
                .search_within_distance_count(word, dist)
                .into_pyobject(py)?
                .into())
        } else {
            Ok(self
                .inner
                .search_within_distance(word, dist)
                .into_pyobject(py)?
                .into())
        }
    }
}

// ── Module registration ───────────────────────────────────────────────────────

#[pymodule]
fn lexrs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTrie>()?;
    m.add_class::<PyDAWG>()?;
    Ok(())
}
