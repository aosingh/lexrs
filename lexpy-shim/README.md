# Lexpy

[![lexpy](https://github.com/aosingh/lexpy/actions/workflows/lexpy_build.yaml/badge.svg)](https://github.com/aosingh/lexpy/actions)
[![Downloads](https://pepy.tech/badge/lexpy)](https://pepy.tech/project/lexpy)
[![PyPI version](https://badge.fury.io/py/lexpy.svg)](https://pypi.python.org/pypi/lexpy)

[![Python 3.10](https://img.shields.io/badge/python-3.10-blue.svg)](https://www.python.org/downloads/release/python-31019/)
[![Python 3.11](https://img.shields.io/badge/python-3.11-blue.svg)](https://www.python.org/downloads/release/python-31114/)
[![Python 3.12](https://img.shields.io/badge/python-3.12-blue.svg)](https://www.python.org/downloads/release/python-31212/)
[![Python 3.13](https://img.shields.io/badge/python-3.13-blue.svg)](https://www.python.org/downloads/release/python-31312/)

> ## lexpy 2.0 is now powered by [lexrs](https://github.com/aosingh/lexrs)
>
> `lexrs` is the Rust-backed successor to `lexpy`. The API is identical — no code changes required.
>
> **Why lexrs is better:**
>
> - **10–100× faster** — core data structures (Trie, DAWG) are implemented in Rust. Wildcard and
>   Levenshtein search show the biggest gains on large lexicons.
> - **Production HTTP server** — `lexrs` ships a built-in reader/writer server. The **writer** ingests
>   words over HTTP and compacts them to snapshots; multiple **reader** replicas serve searches from a
>   DAWG loaded in memory. Scale reads horizontally without any extra infrastructure.
> - **Same API** — `Trie`, `DAWG`, all search methods, wildcard syntax, `with_count` flag — everything
>   works exactly as before.
>
> `lexpy` 2.x is a thin compatibility shim. When you are ready, migrate to `lexrs` directly to drop the shim layer.
>
> **Migrating from lexpy 1.x** — the module namespaces have changed:
>
> ```python
> # lexpy 1.x
> from lexpy.trie import Trie
> from lexpy.dawg import DAWG
>
> # lexpy 2.x / lexrs
> from lexpy import Trie, DAWG   # shim — works, shows DeprecationWarning
> from lexrs import Trie, DAWG   # recommended
> ```

---

- A lexicon is a data-structure which stores a set of words. The difference between
a dictionary and a lexicon is that in a lexicon there are **no values** associated with the words.

- A lexicon is similar to a list or a set of words, but the internal representation is different and optimized
for faster searches of words, prefixes and wildcard patterns.

- Given a word, precisely, the search time is O(W) where W is the length of the word.

- 2 important lexicon data-structures are **_Trie_** and **_Directed Acyclic Word Graph (DAWG)_**.

# Install

```commandline
pip install lexpy        # installs lexrs automatically as a dependency
pip install lexrs        # install the Rust-backed library directly (recommended)
```

# Interface

| **Interface Description**                                                                                                     	| **Trie**                           	| **DAWG**                           	|
|-------------------------------------------------------------------------------------------------------------------------------	|------------------------------------------	|------------------------------------------	|
| Add a single word                                                                                                             	| `add('apple', count=2)`                            	| `add('apple', count=2)`                            	|
| Add multiple words                                                                                                            	| `add_all(['advantage', 'courage'])`       	| `add_all(['advantage', 'courage'])`       	|
| Check if exists?                                                                                                              	| `in` operator                             	| `in` operator                             	|
| Search using wildcard expression                                                                                              	| `search('a?b*', with_count=True)`            | `search('a?b*', with_count=True)`             |
| Search for prefix matches                                                                                                     	| `search_with_prefix('bar', with_count=True)` | `search_with_prefix('bar')`               	|
| Search for similar words within  given edit distance. Here, the notion of edit distance  is same as Levenshtein distance 	| `search_within_distance('apble', dist=1, with_count=True)` 	| `search_within_distance('apble', dist=1, with_count=True)` 	|
| Get the number of nodes in the automaton 	| `len(trie)` 	| `len(dawg)` 	|


# Examples

## Trie

### Build from an input list, set, or tuple of words.

```python
from lexpy import Trie

trie = Trie()

input_words = ['ampyx', 'abuzz', 'athie', 'athie', 'athie', 'amato', 'amato', 'aneto', 'aneto', 'aruba',
               'arrow', 'agony', 'altai', 'alisa', 'acorn', 'abhor', 'aurum', 'albay', 'arbil', 'albin',
               'almug', 'artha', 'algin', 'auric', 'sore', 'quilt', 'psychotic', 'eyes', 'cap', 'suit',
               'tank', 'common', 'lonely', 'likeable' 'language', 'shock', 'look', 'pet', 'dime', 'small'
               'dusty', 'accept', 'nasty', 'thrill', 'foot', 'steel', 'steel', 'steel', 'steel', 'abuzz']

trie.add_all(input_words) # You can pass any sequence types or a file-like object here

print(trie.get_word_count())

>>> 48
```

### Build from a file or file path.

In the file, words should be newline separated.

```python
from lexpy import Trie

trie = Trie()
trie.add_from_file('/path/to/file.txt')
```

### Check if exists using the `in` operator

```python
print('ampyx' in trie)

>>> True
```

### Prefix search

```python
print(trie.search_with_prefix('ab'))

>>> ['abhor', 'abuzz']
```

```python
print(trie.search_with_prefix('ab', with_count=True))

>>> [('abuzz', 2), ('abhor', 1)]
```

### Wildcard search using `?` and `*`

- `?` = exactly one character
- `*` = zero or more characters

```python
print(trie.search('a*o*'))

>>> ['amato', 'abhor', 'aneto', 'arrow', 'agony', 'acorn']

print(trie.search('a*o*', with_count=True))

>>> [('amato', 2), ('abhor', 1), ('aneto', 2), ('arrow', 1), ('agony', 1), ('acorn', 1)]

print(trie.search('su?t'))

>>> ['suit']
```

### Search for similar words using the notion of Levenshtein distance

```python
print(trie.search_within_distance('arie', dist=2))

>>> ['athie', 'arbil', 'auric']

print(trie.search_within_distance('arie', dist=2, with_count=True))

>>> [('athie', 3), ('arbil', 1), ('auric', 1)]
```

### Increment word count

```python
trie.add('athie', count=1000)

print(trie.search_within_distance('arie', dist=2, with_count=True))

>>> [('athie', 1003), ('arbil', 1), ('auric', 1)]
```

# Directed Acyclic Word Graph (DAWG)

- DAWG supports the same set of operations as a Trie. The difference is the number of nodes in a DAWG is always
less than or equal to the number of nodes in Trie.

- They both are Deterministic Finite State Automata. However, DAWG is a minimized version of the Trie DFA.

- In a Trie, prefix redundancy is removed. In a DAWG, both prefix and suffix redundancies are removed.

- In the current implementation of DAWG, the insertion order of the words should be **alphabetical**.

```python
from lexpy import Trie, DAWG

trie = Trie()
trie.add_all(['advantageous', 'courageous'])

dawg = DAWG()
dawg.add_all(['advantageous', 'courageous'])

len(trie)  # Number of nodes in Trie
>>> 23

len(dawg)  # Number of nodes in DAWG (suffix-compressed)
>>> 21
```

## DAWG

The APIs are exactly the same as the Trie APIs.

### Build a DAWG

```python
from lexpy import DAWG

dawg = DAWG()

input_words = ['ampyx', 'abuzz', 'athie', 'athie', 'athie', 'amato', 'amato', 'aneto', 'aneto', 'aruba',
               'arrow', 'agony', 'altai', 'alisa', 'acorn', 'abhor', 'aurum', 'albay', 'arbil', 'albin',
               'almug', 'artha', 'algin', 'auric', 'sore', 'quilt', 'psychotic', 'eyes', 'cap', 'suit',
               'tank', 'common', 'lonely', 'likeable' 'language', 'shock', 'look', 'pet', 'dime', 'small'
               'dusty', 'accept', 'nasty', 'thrill', 'foot', 'steel', 'steel', 'steel', 'steel', 'abuzz']

dawg.add_all(input_words)

dawg.get_word_count()

>>> 48
```

### Check if exists using the `in` operator

```python
print('ampyx' in dawg)

>>> True
```

### Prefix search

```python
print(dawg.search_with_prefix('ab'))

>>> ['abhor', 'abuzz']

print(dawg.search_with_prefix('ab', with_count=True))

>>> [('abuzz', 2), ('abhor', 1)]
```

### Wildcard search using `?` and `*`

```python
print(dawg.search('a*o*'))

>>> ['amato', 'abhor', 'aneto', 'arrow', 'agony', 'acorn']

print(dawg.search('a*o*', with_count=True))

>>> [('amato', 2), ('abhor', 1), ('aneto', 2), ('arrow', 1), ('agony', 1), ('acorn', 1)]
```

### Search for similar words using the notion of Levenshtein distance

```python
print(dawg.search_within_distance('arie', dist=2))

>>> ['athie', 'arbil', 'auric']

print(dawg.search_within_distance('arie', dist=2, with_count=True))

>>> [('athie', 3), ('arbil', 1), ('auric', 1)]
```

### Alphabetical order insertion

If you insert a word which is lexicographically out-of-order, `ValueError` will be raised.

```python
dawg.add('athie', count=1000)
# ValueError: Words should be inserted in Alphabetical order.
```

## Special Characters

Special characters, except `?` and `*`, are matched literally.

```python
from lexpy import Trie
t = Trie()
t.add('a©')

t.search('a©')  # ['a©']
t.search('a?')  # ['a©']
t.search('?©')  # ['a©']
```

## Trie vs DAWG

![Number of nodes comparison](https://github.com/aosingh/lexpy/blob/main/lexpy_trie_dawg_nodes.png)

![Build time comparison](https://github.com/aosingh/lexpy/blob/main/lexpy_trie_dawg_time.png)

# Fun Facts
1. The 45-letter word pneumonoultramicroscopicsilicovolcanoconiosis is the longest English word that appears in a major dictionary.
So for all English words, the search time is bounded by O(45).
2. The longest technical word (not in dictionary) is the name of a protein called [titin](https://en.wikipedia.org/wiki/Titin). It has 189,819
letters and it is disputed whether it is a word.
