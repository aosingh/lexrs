# lexrs-server

Production HTTP server for the `lexrs` lexicon library. Compiled as two separate binaries — **writer** and **reader** — that together form a read-scale architecture backed by Consul for service discovery and snapshot coordination.

## Architecture

```
           ┌───────────┐
 writes ──▶│  writer   │──▶ shared volume (snapshots/)
           └─────┬─────┘         │
                 │ Consul KV     │ (snapshot path)
                 ▼               ▼
           ┌─────────────┐   ┌──────────┐
           │   Consul    │──▶│  reader  │ × N  ──▶ read queries
           └─────────────┘   └──────────┘
```

- **writer** holds a **delta Trie** in memory — only words ingested since the last compaction. On a configurable interval (or via `POST /compact`) it merges the delta with the previous snapshot file into a new one, then announces the new version via Consul KV. The merge is a streaming sorted zipper (O(1) memory, no full reload). On restart the Trie starts empty; only the version counter is recovered from Consul.
- **reader** (horizontally scalable) loads a `DAWG` from the latest snapshot at startup and uses Consul's blocking-query long-poll to detect and hot-reload new snapshots without downtime. DAWG swaps are atomic via `arc-swap`.
- **Consul** provides service registration with HTTP health checks and acts as the pub/sub bus for snapshot version announcements.

## Binaries

### writer

Accepts word ingestion and drives compaction.

**Routes**

| Method | Path | Body / Params | Description |
|---|---|---|---|
| `POST` | `/words` | `{"words": [...], "count": 1}` | Ingest words into the live Trie |
| `POST` | `/compact` | — | Trigger compaction immediately |
| `GET` | `/snapshot/:ver` | — | Download snapshot file by version number |
| `GET` | `/health` | — | Health check (polled by Consul) |
| `GET` | `/stats` | — | `{"words": N, "nodes": N}` |

**Options** (flag or env var, flag takes precedence)

| Flag | Env var | Default | Description |
|---|---|---|---|
| `--host` | `WRITER_HOST` | `0.0.0.0` | Bind address |
| `--port` | `WRITER_PORT` | `3000` | Listen port |
| `--snapshot-dir` | `SNAPSHOT_DIR` | `/snapshots` | Shared volume path |
| `--consul` | `CONSUL_ADDR` | `http://consul:8500` | Consul HTTP address |
| `--compact-interval` | `COMPACT_INTERVAL` | `60` | Auto-compact interval in seconds |

### reader

Serves searches from a DAWG loaded from the shared volume. Watches Consul for new snapshots and reloads atomically.

**Routes**

| Method | Path | Params | Description |
|---|---|---|---|
| `GET` | `/search` | `q=<pattern>[&dist=N][&with_count=true]` | Wildcard or Levenshtein search |
| `GET` | `/prefix` | `q=<prefix>[&with_count=true]` | Prefix completion |
| `GET` | `/contains` | `q=<word>` | Exact membership — returns `{"found": bool}` |
| `GET` | `/health` | — | Health check |
| `GET` | `/stats` | — | `{"words": N, "nodes": N}` |

**Options**

| Flag | Env var | Default | Description |
|---|---|---|---|
| `--host` | `READER_HOST` | `0.0.0.0` | Bind address |
| `--port` | `READER_PORT` | `3001` | Listen port |
| `--snapshot-dir` | `SNAPSHOT_DIR` | `/snapshots` | Shared volume path |
| `--consul` | `CONSUL_ADDR` | `http://consul:8500` | Consul HTTP address |

## Examples

All examples assume the stack is running via Docker Compose. Write routes go to `:3000` (writer direct) or `:80` (via nginx). Read routes go to `:80` (nginx load-balances across readers).

### Writer

**Ingest words (uniform count)**
```bash
curl -s -X POST http://localhost/words \
  -H "Content-Type: application/json" \
  -d '{"words": ["apple", "apply", "apt", "banana", "band"], "count": 1}'
```

**Ingest words with per-word counts**
```bash
curl -s -X POST http://localhost/words \
  -H "Content-Type: application/json" \
  -d '{"words": [{"word": "apple", "count": 10}, {"word": "apply", "count": 3}, "apt"]}'
```

**Force compaction** (makes words visible to readers immediately)
```bash
curl -s -X POST http://localhost/compact
```

**Writer stats** (live Trie — words ingested since last compaction, not yet visible to readers)
```bash
curl -s http://localhost:3000/stats
```

### Reader

**Wildcard search**
```bash
curl -s "http://localhost/search?q=ap*"
curl -s "http://localhost/search?q=b????"
```

**Wildcard search with counts**
```bash
curl -s "http://localhost/search?q=ap*&with_count=true"
```

**Fuzzy search (Levenshtein)**
```bash
# words within edit distance 1 of "aple"
curl -s "http://localhost/search?q=aple&dist=1"

# broader — edit distance 2
curl -s "http://localhost/search?q=bannana&dist=2"

# fuzzy with counts
curl -s "http://localhost/search?q=aple&dist=1&with_count=true"
```

**Prefix completion**
```bash
curl -s "http://localhost/prefix?q=app"
curl -s "http://localhost/prefix?q=ban&with_count=true"
```

**Exact lookup**
```bash
curl -s "http://localhost/contains?q=apple"   # {"found": true}
curl -s "http://localhost/contains?q=appl"    # {"found": false}
```

**Reader stats** (DAWG loaded from latest snapshot — all compacted words visible to search)
```bash
curl -s "http://localhost/stats"
```

**Health**
```bash
curl -s "http://localhost/health"
```

> **Note:** words ingested via `POST /words` are held in the writer's in-memory Trie and are not visible to readers until compaction runs (automatically every `COMPACT_INTERVAL` seconds, or manually via `POST /compact`).

## Source layout

```
lexrs-server/
  src/
    writer.rs   — writer binary: ingest, compact, announce
    reader.rs   — reader binary: search, Consul watch & hot-reload
    consul.rs   — Consul service registration and KV helpers (shared)
    snapshot.rs — merge_and_write (streaming sorted merge), load into DAWG (shared)
```

## Building

```bash
# Both binaries
cargo build --release -p lexrs-server

# Individual
cargo build --release -p lexrs-server --bin writer
cargo build --release -p lexrs-server --bin reader
```

For containerized deployment see the `docker/` directory.

## Snapshot format

Each snapshot is a plain-text file named `snapshot_<version>.txt`, stored on the shared volume. One `word count` pair per line, sorted lexicographically.

**Compaction is merge-based** — the writer streams the previous snapshot and the new delta Trie simultaneously (both sorted), writing a merged output line by line. Memory usage during compaction is O(1) with respect to lexicon size. If the same word appears in both, counts are summed.

On restart the writer only recovers the version counter from Consul — the Trie starts empty. The full lexicon is always reconstructable from the latest snapshot file.

## Consul integration

- Both binaries register themselves with Consul on startup using an HTTP health check (`GET /health`).
- On compaction, the writer stores `{"version": N, "path": "/snapshots/snapshot_N.txt"}` at `lexrs/snapshot` in the KV store.
- Readers use Consul's blocking query (`?wait=30s&index=<last>`) on that key to be notified of new snapshots without polling.
