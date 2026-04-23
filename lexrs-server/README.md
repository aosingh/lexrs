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

- **writer** holds words in a live `Trie` in memory. On a configurable interval (or via `POST /compact`) it serializes the Trie to a snapshot file on the shared volume, then announces the new version to all readers through a Consul KV key.
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

## Source layout

```
lexrs-server/
  src/
    writer.rs   — writer binary: ingest, compact, announce
    reader.rs   — reader binary: search, Consul watch & hot-reload
    consul.rs   — Consul service registration and KV helpers (shared)
    snapshot.rs — snapshot file write/load helpers (shared)
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

Each snapshot is a plain text file, one `word\tcount` pair per line, written by the writer and read by the reader to reconstruct a DAWG. Files are named `snapshot_<version>.txt` and stored on the shared volume.

## Consul integration

- Both binaries register themselves with Consul on startup using an HTTP health check (`GET /health`).
- On compaction, the writer stores `{"version": N, "path": "/snapshots/snapshot_N.txt"}` at `lexrs/snapshot` in the KV store.
- Readers use Consul's blocking query (`?wait=30s&index=<last>`) on that key to be notified of new snapshots without polling.
