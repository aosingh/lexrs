# HTTP Server

`lexrs-server` ships two binaries that together form a production-ready search service. The **writer** accepts word ingestion and manages compaction; the **reader** serves search queries from a compressed DAWG, scaling horizontally.

## Install

```bash
cargo install lexrs-server
```

This installs both the `writer` and `reader` binaries.

---

## Architecture

```
            ┌───────────────────────────────┐
  POST      │            writer             │
  /words ──▶│  delta Trie (in-memory)       │──▶ /snapshots/snapshot_N.txt
            └──────────────┬────────────────┘         (shared volume)
                           │                               │
                    Consul KV write                        │
                    lexrs/snapshot                         │
                           │                               │
                           ▼                               ▼
                    ┌─────────────┐              ┌─────────────────┐
                    │   Consul    │──blocking ──▶│    reader × N   │
                    └─────────────┘   query      │  DAWG in memory │──▶ GET /search
                                                 └─────────────────┘    GET /prefix
                                                                         GET /contains
```

**Write path**

1. Clients send words to `POST /words`. The writer inserts them into an in-memory Trie.
2. On a configurable interval (or via `POST /compact`), the writer merges the delta Trie with the existing snapshot file using a streaming sorted zipper (O(1) memory).
3. The new snapshot is written atomically (`.tmp` + rename), then announced via a Consul KV write.

**Read path**

1. Each reader loads the latest snapshot into a DAWG at startup.
2. Readers long-poll Consul (`?wait=30s`) for new snapshot versions.
3. When a new version is announced, the reader loads it and atomically swaps the in-memory DAWG via `arc-swap` — no downtime, no request drops.

---

## writer

### Start

```bash
writer --host 0.0.0.0 --port 3000 \
       --snapshot-dir /snapshots \
       --consul http://localhost:8500 \
       --compact-interval 60
```

All flags can also be set via environment variables.

### Configuration

| Flag | Env var | Default | Description |
|---|---|---|---|
| `--host` | `WRITER_HOST` | `0.0.0.0` | Bind address |
| `--port` | `WRITER_PORT` | `3000` | Listen port |
| `--snapshot-dir` | `SNAPSHOT_DIR` | `/snapshots` | Shared volume path |
| `--consul` | `CONSUL_ADDR` | `http://consul:8500` | Consul HTTP address |
| `--compact-interval` | `COMPACT_INTERVAL` | `60` | Auto-compact interval (seconds) |

### Routes

| Method | Path | Body | Description |
|---|---|---|---|
| `POST` | `/words` | `{"words": [...], "count": 1}` | Ingest words into the live Trie |
| `POST` | `/compact` | — | Trigger compaction immediately |
| `GET` | `/snapshot/:ver` | — | Download snapshot file by version |
| `GET` | `/health` | — | Health check (polled by Consul) |
| `GET` | `/stats` | — | `{"words": N, "nodes": N}` |

### Ingest examples

**Uniform count** — all words get the same frequency:

```bash
curl -X POST http://localhost:3000/words \
  -H 'Content-Type: application/json' \
  -d '{"words": ["apple", "apply", "apt", "banana"]}'
# {"inserted": 4}
```

**Per-word counts** — mix plain strings and `{"word", "count"}` objects:

```bash
curl -X POST http://localhost:3000/words \
  -H 'Content-Type: application/json' \
  -d '{
    "words": [
      {"word": "apple",  "count": 10},
      {"word": "apply",  "count": 3},
      "apt"
    ]
  }'
# {"inserted": 3}
```

**Force compaction** — makes queued words immediately visible to readers:

```bash
curl -X POST http://localhost:3000/compact
# {"status": "ok", "version": 1}
```

**Writer stats** — shows the live delta Trie (words not yet compacted):

```bash
curl http://localhost:3000/stats
# {"words": 42, "nodes": 187}
```

!!! note
    Words in the writer's Trie are **not visible to readers** until compaction runs. Use `POST /compact` to flush immediately.

---

## reader

### Start

```bash
reader --host 0.0.0.0 --port 3001 \
       --snapshot-dir /snapshots \
       --consul http://localhost:8500
```

### Configuration

| Flag | Env var | Default | Description |
|---|---|---|---|
| `--host` | `READER_HOST` | `0.0.0.0` | Bind address |
| `--port` | `READER_PORT` | `3001` | Listen port |
| `--snapshot-dir` | `SNAPSHOT_DIR` | `/snapshots` | Shared volume path |
| `--consul` | `CONSUL_ADDR` | `http://consul:8500` | Consul HTTP address |

### Routes

| Method | Path | Params | Description |
|---|---|---|---|
| `GET` | `/search` | `q=<pattern>[&dist=N][&with_count=true]` | Wildcard or fuzzy search |
| `GET` | `/prefix` | `q=<prefix>[&with_count=true]` | Prefix completion |
| `GET` | `/contains` | `q=<word>` | Exact membership — `{"found": bool}` |
| `GET` | `/health` | — | Health check |
| `GET` | `/stats` | — | `{"words": N, "nodes": N}` |

### Search examples

**Wildcard search:**

```bash
curl 'http://localhost:3001/search?q=ap*'
# ["apple", "apply", "apt"]

curl 'http://localhost:3001/search?q=b????'
# ["bible"] — exactly 5 chars starting with b
```

**Wildcard with counts:**

```bash
curl 'http://localhost:3001/search?q=ap*&with_count=true'
# [{"word": "apple", "count": 10}, {"word": "apply", "count": 3}, {"word": "apt", "count": 1}]
```

**Levenshtein fuzzy search:**

```bash
# words within edit distance 1 of "aple"
curl 'http://localhost:3001/search?q=aple&dist=1'
# ["apple"]

# broader search
curl 'http://localhost:3001/search?q=bannana&dist=2'
# ["banana"]

# fuzzy with counts
curl 'http://localhost:3001/search?q=aple&dist=1&with_count=true'
# [{"word": "apple", "count": 10}]
```

**Prefix completion:**

```bash
curl 'http://localhost:3001/prefix?q=app'
# ["apple", "apply"]

curl 'http://localhost:3001/prefix?q=app&with_count=true'
# [{"word": "apple", "count": 10}, {"word": "apply", "count": 3}]
```

**Exact lookup:**

```bash
curl 'http://localhost:3001/contains?q=apple'
# {"found": true}

curl 'http://localhost:3001/contains?q=appl'
# {"found": false}
```

---

## Snapshot format

Each snapshot is a plain-text file on the shared volume:

```
apple 10
apply 3
apt 1
banana 5
```

One `word count` pair per line, sorted lexicographically. Compaction merges the existing snapshot with the new delta in a single streaming pass — memory usage during compaction is O(1) regardless of lexicon size. If a word appears in both, counts are summed.

Snapshots are named `snapshot_<version>.txt` and are never deleted automatically, making it easy to roll back.
