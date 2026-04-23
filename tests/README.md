# tests

Integration and API tests for `lexrs`.

## Contents

| File | Type | Description |
|---|---|---|
| `test_python_api.py` | pytest | Full Python API coverage for `Trie` and `DAWG` |
| `trie_tests.rs` | Rust | Rust-level integration tests for `Trie` |
| `dawg_tests.rs` | Rust | Rust-level integration tests for `DAWG` |
| `data/words2.txt` | Fixture | 8-word newline-delimited word list used by file-load tests |

Unit tests for individual modules live alongside the source in `lexrs/tests/`.

## Running

### Python tests

Requires the `lexrs` Python package to be built and installed first.

```bash
# From repo root
maturin develop --features python
pytest tests/
```

### Rust tests

```bash
cargo test
```

This runs both the workspace-level tests in `tests/` and the module-level tests in `lexrs/tests/`.

## Python test coverage

The pytest suite covers:

- **Trie**: `add`, `add_all` (list, set, tuple, generator), `add_from_file`, `contains`, `contains_prefix`, `search_with_prefix`, wildcard `search` (`*`, `?`, combined), `search_within_distance` (Levenshtein), `word_count`, `node_count`, `repr`
- **DAWG**: same surface area as Trie, plus out-of-order insertion raises `ValueError`
- Edge cases: empty structure, empty pattern, special characters, absent words
