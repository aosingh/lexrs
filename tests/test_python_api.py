"""
Python API tests for lexrs (Trie and DAWG).
Run with: pytest tests/test_python_api.py
"""

import pytest
from lexrs import Trie, DAWG


# ── Trie: word count ──────────────────────────────────────────────────────────

def test_trie_word_count():
    t = Trie()
    t.add_all(["ash", "ashley", "ashes"])
    assert t.get_word_count() == 3

def test_trie_word_count_empty():
    t = Trie()
    assert t.get_word_count() == 0


# ── Trie: contains ───────────────────────────────────────────────────────────

def test_trie_contains():
    t = Trie()
    t.add_all(["ash", "ashley"])
    assert "ash" in t
    assert "ashley" in t
    assert "salary" not in t
    assert "as" not in t

def test_trie_empty_string_always_present():
    t = Trie()
    assert "" in t


# ── Trie: add ────────────────────────────────────────────────────────────────

def test_trie_add_single():
    t = Trie()
    t.add("axe")
    assert "axe" in t

def test_trie_add_with_count():
    t = Trie()
    t.add("hello", 5)
    results = t.search("*", with_count=True)
    assert results == [("hello", 5)]

def test_trie_add_all_list():
    t = Trie()
    t.add_all(["axe", "kick"])
    assert "axe" in t
    assert "kick" in t
    assert t.get_word_count() == 2

def test_trie_add_all_set():
    t = Trie()
    t.add_all({"axe", "kick"})
    assert "axe" in t
    assert "kick" in t

def test_trie_add_all_tuple():
    t = Trie()
    t.add_all(("axe", "kick"))
    assert "axe" in t
    assert "kick" in t

def test_trie_add_all_generator():
    t = Trie()
    t.add_all(w for w in ["ash", "ashley", "simpson"])
    assert "ash" in t
    assert "ashley" in t
    assert "simpson" in t
    assert t.get_word_count() == 3

def test_trie_add_from_file():
    t = Trie()
    t.add_from_file("tests/data/words2.txt")
    assert "ash" in t
    assert "ashley" in t
    assert "simpson" in t
    assert t.get_word_count() == 8


# ── Trie: node count ─────────────────────────────────────────────────────────

def test_trie_node_count():
    t = Trie()
    t.add_all(["ash", "ashley"])
    # root + a + s + h + l + e + y = 7
    assert len(t) == 7


# ── Trie: prefix ─────────────────────────────────────────────────────────────

def test_trie_prefix_exists():
    t = Trie()
    t.add_all(["ash", "ashley"])
    assert t.contains_prefix("a")
    assert t.contains_prefix("as")
    assert t.contains_prefix("ash")

def test_trie_prefix_not_exists():
    t = Trie()
    t.add_all(["ash", "ashley"])
    assert not t.contains_prefix("sh")
    assert not t.contains_prefix("xmas")


# ── Trie: prefix search ───────────────────────────────────────────────────────

def test_trie_search_with_prefix():
    t = Trie()
    t.add_all(["ashlame", "ashley", "ashlo", "askoiu"])
    assert sorted(t.search_with_prefix("ash")) == ["ashlame", "ashley", "ashlo"]

def test_trie_search_with_prefix_no_match():
    t = Trie()
    t.add_all(["ash", "ashley"])
    assert t.search_with_prefix("xyz") == []

def test_trie_search_with_prefix_with_count():
    t = Trie()
    t.add("ash", 3)
    t.add("ashley", 1)
    results = {w: c for w, c in t.search_with_prefix_count("ash")}
    assert results["ash"] == 3
    assert results["ashley"] == 1


# ── Trie: wildcard search ────────────────────────────────────────────────────

def test_trie_search_star():
    t = Trie()
    t.add_all(["ash", "ashley"])
    assert sorted(t.search("a*")) == ["ash", "ashley"]

def test_trie_search_star_variants():
    t = Trie()
    t.add_all(["ash", "ashley"])
    # a?*, a*?, a*** all normalize to a*
    assert sorted(t.search("a?*")) == ["ash", "ashley"]
    assert sorted(t.search("a*?")) == ["ash", "ashley"]
    assert sorted(t.search("a***")) == ["ash", "ashley"]

def test_trie_search_question():
    t = Trie()
    t.add_all(["ab", "as", "ash", "ashley"])
    assert sorted(t.search("a?")) == ["ab", "as"]

def test_trie_search_combined():
    t = Trie()
    t.add_all(["ab", "as", "ash", "ashley"])
    assert sorted(t.search("*a******?")) == ["ab", "as", "ash", "ashley"]

def test_trie_search_with_count():
    t = Trie()
    t.add("ash", 2)
    t.add("ashley", 1)
    results = dict(t.search("a*", with_count=True))
    assert results["ash"] == 2
    assert results["ashley"] == 1

def test_trie_search_empty_pattern():
    t = Trie()
    t.add_all(["ash", "ashley"])
    assert t.search("") == []

def test_trie_search_special_chars():
    t = Trie()
    t.add_all(["ash", "#$%^a"])
    assert "#$%^a" in t


# ── Trie: Levenshtein search ─────────────────────────────────────────────────

def test_trie_search_within_distance_exact():
    t = Trie()
    t.add_all(["ash", "ashe", "ashley"])
    assert t.search_within_distance("ash", 0) == ["ash"]

def test_trie_search_within_distance_1():
    t = Trie()
    t.add_all(["ash", "ashe", "ashley"])
    assert sorted(t.search_within_distance("ash", 1)) == ["ash", "ashe"]

def test_trie_search_within_distance_with_count():
    t = Trie()
    t.add("ash", 3)
    results = dict(t.search_within_distance("ash", 0, with_count=True))
    assert results["ash"] == 3


# ── DAWG: word count ──────────────────────────────────────────────────────────

def test_dawg_word_count():
    d = DAWG()
    d.add_all(["ash", "ashes", "ashley"])
    assert d.get_word_count() == 3

def test_dawg_word_count_empty():
    d = DAWG()
    assert d.get_word_count() == 0


# ── DAWG: contains ───────────────────────────────────────────────────────────

def test_dawg_contains():
    d = DAWG()
    d.add_all(["ash", "ashley"])
    assert "ash" in d
    assert "ashley" in d
    assert "salary" not in d

def test_dawg_empty_string_always_present():
    d = DAWG()
    assert "" in d


# ── DAWG: add ────────────────────────────────────────────────────────────────

def test_dawg_add_single():
    d = DAWG()
    d.add("axe")
    assert "axe" in d

def test_dawg_order_violation():
    d = DAWG()
    d.add("zebra")
    with pytest.raises(ValueError, match="alphabetical order"):
        d.add("apple")

def test_dawg_add_all_list():
    d = DAWG()
    d.add_all(["axe", "kick"])
    assert "axe" in d
    assert "kick" in d
    assert d.get_word_count() == 2

def test_dawg_add_all_set():
    d = DAWG()
    d.add_all({"axe", "kick"})
    assert "axe" in d
    assert "kick" in d

def test_dawg_add_all_generator():
    d = DAWG()
    d.add_all(w for w in ["ash", "ashley", "simpson"])
    assert "ash" in d
    assert "simpson" in d
    assert d.get_word_count() == 3

def test_dawg_add_from_file():
    d = DAWG()
    d.add_from_file("tests/data/words2.txt")
    assert "ash" in d
    assert "ashley" in d
    assert "simpson" in d
    assert d.get_word_count() == 8


# ── DAWG: prefix ─────────────────────────────────────────────────────────────

def test_dawg_prefix_exists():
    d = DAWG()
    d.add_all(["ash", "ashley"])
    assert d.contains_prefix("a")
    assert d.contains_prefix("as")
    assert d.contains_prefix("ash")

def test_dawg_prefix_not_exists():
    d = DAWG()
    d.add_all(["ash", "ashley"])
    assert not d.contains_prefix("sh")
    assert not d.contains_prefix("xmas")


# ── DAWG: prefix search ───────────────────────────────────────────────────────

def test_dawg_search_with_prefix():
    d = DAWG()
    d.add_all(["ashlame", "ashley", "ashlo", "askoiu"])
    assert sorted(d.search_with_prefix("ash")) == ["ashlame", "ashley", "ashlo"]

def test_dawg_search_with_prefix_no_match():
    d = DAWG()
    d.add_all(["ash", "ashley"])
    assert d.search_with_prefix("xyz") == []


# ── DAWG: wildcard search ────────────────────────────────────────────────────

def test_dawg_search_star():
    d = DAWG()
    d.add_all(["ash", "ashley"])
    assert sorted(d.search("a*")) == ["ash", "ashley"]

def test_dawg_search_question():
    d = DAWG()
    d.add_all(["ab", "as", "ash", "ashley"])
    assert sorted(d.search("a?")) == ["ab", "as"]

def test_dawg_search_combined():
    d = DAWG()
    d.add_all(["ab", "as", "ash", "ashley"])
    assert sorted(d.search("*a******?")) == ["ab", "as", "ash", "ashley"]

def test_dawg_search_empty_pattern():
    d = DAWG()
    d.add_all(["ash"])
    assert d.search("") == []


# ── DAWG: Levenshtein search ─────────────────────────────────────────────────

def test_dawg_search_within_distance():
    d = DAWG()
    input_words = [
        "abhor", "abuzz", "accept", "acorn", "agony", "albay", "albin", "algin",
        "alisa", "almug", "altai", "amato", "ampyx", "aneto", "arbil", "arrow",
        "artha", "aruba", "athie", "auric", "aurum", "cap", "common", "dime",
        "eyes", "foot", "likeablelanguage", "lonely", "look", "nasty", "pet",
        "psychotic", "quilt", "shock", "smalldusty", "sore", "steel", "suit",
        "tank", "thrill",
    ]
    d.add_all(input_words)
    assert sorted(d.search_within_distance("arie", dist=2)) == ["arbil", "athie", "auric"]


# ── DAWG: repr ───────────────────────────────────────────────────────────────

def test_repr():
    t = Trie()
    t.add("hello")
    assert "Trie" in repr(t)

    d = DAWG()
    d.add_all(["hello"])
    assert "DAWG" in repr(d)
