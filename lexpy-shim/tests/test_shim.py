import warnings
import pytest


def test_deprecation_warning_on_import():
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        import lexpy  # noqa: F401
        assert any(issubclass(w.category, DeprecationWarning) for w in caught)


def test_trie_importable():
    from lexpy import Trie
    t = Trie()
    t.add_all(["apple", "apply", "apt"])
    assert "apple" in t
    assert t.get_word_count() == 3


def test_dawg_importable():
    from lexpy import DAWG
    d = DAWG()
    d.add_all(["apple", "apply", "apt"])
    assert "apple" in d
    assert d.get_word_count() == 3


def test_trie_is_lexrs_trie():
    from lexpy import Trie as LexpyTrie
    from lexrs import Trie as LexrsTrie
    assert LexpyTrie is LexrsTrie


def test_dawg_is_lexrs_dawg():
    from lexpy import DAWG as LexpyDAWG
    from lexrs import DAWG as LexrsDAWG
    assert LexpyDAWG is LexrsDAWG
