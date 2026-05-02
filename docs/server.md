# HTTP Server

The `lexrs-server` crate compiles to two binaries: **writer** and **reader**. They are designed to run together as a search service, but they have no shared code path at runtime — they communicate only through files on a shared volume and a Consul KV entry.

## Install

```bash
cargo install lexrs-server
```

---

## Why two binaries?

Search reads and word writes have very different performance profiles.

Writes need a mutable data structure (Trie) and can tolerate some latency — the caller just posted a word and moved on. Reads need an immutable, highly-compressed structure (DAWG) that can serve many concurrent queries without locking.

Splitting into two processes lets you:

- **Scale readers independently.** Run 1 writer and 10 readers if your query volume demands it.
- **Isolate faults.** A crash in the writer does not affect in-flight search queries.
- **Reload without downtime.** Readers swap their in-memory DAWG atomically when a new snapshot arrives — no restart, no dropped requests.

---

## How a word goes from `POST /words` to a search result

Understanding this flow makes it easier to configure and operate the server.

**Step 1 — Ingest.** A client posts words to the writer. The writer inserts them into an in-memory Trie. At this point the words are not yet visible to readers.

**Step 2 — Compact.** Every `COMPACT_INTERVAL` seconds (or immediately via `POST /compact`), the writer:

1. Reads all words out of its Trie.
2. Opens the previous snapshot file (a sorted `word count` text file).
3. Merges the two sorted streams line by line — like a merge sort merge step. If a word appears in both, counts are summed. Memory usage during this step is O(1); neither the snapshot nor the Trie is loaded in full.
4. Writes the merged output to a `.tmp` file, then renames it atomically to `snapshot_N.txt`.
5. Clears the Trie. It now holds only the delta since the last compaction.

**Step 3 — Announce.** The writer stores `{"version": N, "path": "/snapshots/snapshot_N.txt"}` at the key `lexrs/snapshot` in Consul's KV store.

**Step 4 — Reload.** Each reader runs a background loop that long-polls Consul on that key (`?wait=30s`). When the version changes, the reader:

1. Opens the new snapshot file.
2. Loads all words into a new DAWG.
3. Calls `reduce()` to finalise minimisation.
4. Atomically swaps the new DAWG into the serving path using `arc-swap`. In-flight requests against the old DAWG complete normally.

```
  client                writer                consul            reader(s)
    │                     │                     │                  │
    │  POST /words         │                     │                  │
    │────────────────────▶│                     │                  │
    │  {"inserted": N}     │                     │                  │
    │◀────────────────────│                     │                  │
    │                     │                     │                  │
    │  (60s passes)        │                     │                  │
    │                     │ compact + write      │                  │
    │                     │──snapshot_2.txt────▶ volume            │
    │                     │                     │                  │
    │                     │ PUT lexrs/snapshot   │                  │
    │                     │────────────────────▶│                  │
    │                     │                     │  long-poll fires │
    │                     │                     │─────────────────▶│
    │                     │                     │  version=2       │
    │                     │                     │◀─────────────────│
    │                     │                     │                  │ load + reduce
    │                     │                     │                  │ arc-swap
    │  GET /search?q=ap*  │                     │                  │
    │────────────────────────────────────────────────────────────▶│
    │  ["apple","apply"]  │                     │                  │
    │◀────────────────────────────────────────────────────────────│
```

---

## Running the writer

```bash
writer \
  --host 0.0.0.0 \
  --port 3000 \
  --snapshot-dir /snapshots \
  --consul http://localhost:8500 \
  --compact-interval 60
```

Every flag has a corresponding environment variable:

| Flag | Env var | Default |
|---|---|---|
| `--host` | `WRITER_HOST` | `0.0.0.0` |
| `--port` | `WRITER_PORT` | `3000` |
| `--snapshot-dir` | `SNAPSHOT_DIR` | `/snapshots` |
| `--consul` | `CONSUL_ADDR` | `http://consul:8500` |
| `--compact-interval` | `COMPACT_INTERVAL` | `60` |

### Ingesting words

Send a JSON object with a `words` array. Each element can be a plain string (uses the top-level `count`) or an object with its own count:

```bash
# All words get count = 1 (the default)
curl -X POST http://localhost:3000/words \
  -H 'Content-Type: application/json' \
  -d '{"words": ["apple", "apply", "apt"]}'

# All words get count = 3
curl -X POST http://localhost:3000/words \
  -H 'Content-Type: application/json' \
  -d '{"words": ["apple", "apply", "apt"], "count": 3}'

# Per-word counts — mix strings and objects freely
curl -X POST http://localhost:3000/words \
  -H 'Content-Type: application/json' \
  -d '{
    "words": [
      {"word": "apple", "count": 10},
      {"word": "apply", "count": 3},
      "apt"
    ]
  }'
```

Response:

```json
{"inserted": 3}
```

### Triggering compaction

By default the writer compacts every 60 seconds. To flush immediately — useful after a bulk load or in tests:

```bash
curl -X POST http://localhost:3000/compact
```

```json
{"status": "ok", "version": 2}
```

The version number increments with each compaction. After this call, readers will pick up the new snapshot within one Consul poll cycle (≤ 30 seconds).

### Checking writer stats

Stats reflect the **live delta Trie** — words ingested since the last compaction, not yet visible to readers:

```bash
curl http://localhost:3000/stats
```

```json
{"words": 47, "nodes": 203}
```

If both numbers are 0 after a compaction, all words have been flushed to the snapshot and readers have everything.

---

## Running the reader

```bash
reader \
  --host 0.0.0.0 \
  --port 3001 \
  --snapshot-dir /snapshots \
  --consul http://localhost:8500
```

| Flag | Env var | Default |
|---|---|---|
| `--host` | `READER_HOST` | `0.0.0.0` |
| `--port` | `READER_PORT` | `3001` |
| `--snapshot-dir` | `SNAPSHOT_DIR` | `/snapshots` |
| `--consul` | `CONSUL_ADDR` | `http://consul:8500` |

### Wildcard search

```bash
# Zero or more characters
curl 'http://localhost:3001/search?q=ap*'
# ["apple", "apply", "apt"]

# Exactly one character
curl 'http://localhost:3001/search?q=appl?'
# ["apple", "apply"]

# With frequency counts
curl 'http://localhost:3001/search?q=ap*&with_count=true'
# [{"word":"apple","count":10}, {"word":"apply","count":3}, {"word":"apt","count":1}]
```

### Fuzzy search

```bash
# Levenshtein distance ≤ 1
curl 'http://localhost:3001/search?q=aple&dist=1'
# ["apple"]

# Distance ≤ 2 with counts
curl 'http://localhost:3001/search?q=bannana&dist=2&with_count=true'
# [{"word":"banana","count":5}]
```

### Prefix completion

```bash
curl 'http://localhost:3001/prefix?q=app'
# ["apple", "apply"]

curl 'http://localhost:3001/prefix?q=app&with_count=true'
# [{"word":"apple","count":10}, {"word":"apply","count":3}]
```

### Exact lookup

```bash
curl 'http://localhost:3001/contains?q=apple'
# {"found": true}

curl 'http://localhost:3001/contains?q=appl'
# {"found": false}
```

### Reader stats

Stats reflect the DAWG loaded from the latest snapshot — the full compacted lexicon:

```bash
curl 'http://localhost:3001/stats'
# {"words": 1250000, "nodes": 420000}
```

---

## Snapshot format

Snapshots are plain UTF-8 text files on the shared volume, one entry per line:

```
apple 10
apply 3
apt 1
banana 5
```

Lines are sorted lexicographically. The format is intentionally simple — you can inspect, diff, or modify snapshots with standard Unix tools. The filename is `snapshot_<version>.txt`; old versions are not deleted automatically, so you can roll back by pointing Consul at an older version.
