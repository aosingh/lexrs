# docker

Docker Compose setup for running the full lexrs stack locally or in production.

## Architecture

```
                        ┌─────────────────────────────────────────────┐
                        │              Docker Network                  │
                        │                                              │
  ┌──────────┐          │  ┌─────────────────────────────────────┐    │
  │  Client  │          │  │         nginx:alpine                │    │
  │          │──────────┼─▶│         localhost:80                │    │
  └──────────┘  :80     │  └──────────────┬──────────────────────┘    │
                        │                 │                            │
                        │    ┌────────────┴────────────┐              │
                        │    │                         │              │
                        │    ▼  /words /compact        ▼  /search     │
                        │    │  /snapshot              │  /prefix     │
                        │    │                         │  /contains   │
                        │  ┌─┴────────────┐    ┌───────┴──────────┐   │
                        │  │    writer    │    │  reader (×2)     │   │
                        │  │  :3000       │    │  :3001           │   │
                        │  │  (internal)  │    │  (internal)      │   │
                        │  └──────┬───────┘    └───────┬──────────┘   │
                        │         │                    │              │
                        │         │ register+KV put    │ KV watch     │
                        │         │                    │              │
                        │         ▼                    ▼              │
                        │  ┌──────────────────────────────────────┐   │
                        │  │       consul (hashicorp/consul:1.18) │   │
                        │  │       :8500 (UI + API)               │   │
                        │  └──────────────────────────────────────┘   │
                        │                                              │
                        │         │  snapshot files    │              │
                        │         ▼                    ▼              │
                        │  ┌──────────────────────────────────────┐   │
                        │  │       Docker Volume: snapshots       │   │
                        │  │       mounted at /snapshots          │   │
                        │  └──────────────────────────────────────┘   │
                        └─────────────────────────────────────────────┘

  Exposed to host:
    :80    → nginx  (all traffic)
    :3000  → writer (debug only)
    :8500  → consul (UI + API)
```

## Request flow

| Operation | Entry | Routed to | Internal port |
|---|---|---|---|
| Ingest words | `POST localhost/words` | writer | 3000 |
| Force compact | `POST localhost/compact` | writer | 3000 |
| Wildcard search | `GET localhost/search?q=ap*` | reader (round-robin) | 3001 |
| Prefix search | `GET localhost/prefix?q=app` | reader (round-robin) | 3001 |
| Exact lookup | `GET localhost/contains?q=apple` | reader (round-robin) | 3001 |
| Batch membership | `POST localhost/batch/contains` | reader (round-robin) | 3001 |
| Batch wildcard | `POST localhost/batch/search` | reader (round-robin) | 3001 |
| Batch prefix | `POST localhost/batch/prefix` | reader (round-robin) | 3001 |
| Batch fuzzy | `POST localhost/batch/search_within_distance` | reader (round-robin) | 3001 |
| Health check | `GET localhost/health` | reader | 3001 |
| Consul UI | `http://localhost:8500` | consul | 8500 |

## Services

| Service | Image / Build | Port | Description |
|---|---|---|---|
| `consul` | `hashicorp/consul:1.18` | 8500 | Service discovery and KV store |
| `writer` | `Dockerfile` | 3000 | Word ingestion; compaction merges delta Trie with previous snapshot |
| `reader` | `Dockerfile` | 3001 (internal) | Search server (2 replicas by default) |
| `nginx` | `nginx:alpine` | 80 | Reverse proxy — routes reads/writes to the right service |

## Startup order

```
consul (healthy) → writer → reader × 2 → nginx
```

Consul must pass its health check (`consul members`) before writer or reader start. Writer must be started before readers because readers attempt to pull the latest snapshot on boot.

On startup the writer recovers its version counter from Consul KV (`lexrs/snapshot`). If Consul is empty (e.g. after a restart), it falls back to `latest.json` on the shared volume and re-publishes the metadata to Consul. Readers follow the same fallback — Consul first, then `latest.json` — so both services recover their state from disk without losing data.

## Consul restart recovery

The snapshot volume is the durable source of truth. After every compaction the writer atomically writes a `latest.json` file alongside the snapshot files:

```json
{"version": 3, "path": "/snapshots/snapshot_3.txt"}
```

If Consul restarts and loses its KV data, restarting the writer and readers is sufficient — they read `latest.json` from disk to recover and the writer re-publishes to Consul so subsequent watch notifications work normally.

## Nginx routing

| URL pattern | Proxied to |
|---|---|
| `/words`, `/compact`, `/snapshot/*` | `writer:3000` |
| `/search`, `/prefix`, `/contains`, `/stats`, `/batch/*` | `readers:3001` (round-robin) |
| `/health` | `readers:3001` |

All traffic enters on port **80**. Writer is also directly reachable on **3000** for debugging.

## Quick start — API examples

After `docker compose up --build`, ingest a test lexicon and exercise every endpoint type:

```bash
# 1. Ingest words with per-word counts
curl -X POST http://localhost/words \
  -H 'Content-Type: application/json' \
  -d '{
    "words": [
      {"word": "apple",  "count": 10},
      {"word": "apply",  "count": 3},
      {"word": "apt",    "count": 1},
      {"word": "banana", "count": 5},
      "cherry"
    ]
  }'

# 2. Flush to snapshot (readers pick it up within ~30 s)
curl -X POST http://localhost/compact

# 3. Single-word queries
curl 'http://localhost/contains?q=apple'          # {"found":true}
curl 'http://localhost/prefix?q=app'              # ["apple","apply"]
curl 'http://localhost/search?q=ap*'              # ["apple","apply","apt"]
curl 'http://localhost/search?q=aple&dist=1'      # ["apple"]

# 4. Batch membership
curl -X POST http://localhost/batch/contains \
  -H 'Content-Type: application/json' \
  -d '{"words": ["apple", "cherry", "apricot", "apply"]}'
# [true, true, false, true]

# 5. Batch wildcard
curl -X POST http://localhost/batch/search \
  -H 'Content-Type: application/json' \
  -d '{"patterns": ["ap*", "b*", "c?erry"]}'
# [["apple","apply","apt"], ["banana"], ["cherry"]]

# 6. Batch prefix
curl -X POST http://localhost/batch/prefix \
  -H 'Content-Type: application/json' \
  -d '{"prefixes": ["app", "ban", "ch"]}'
# [["apple","apply"], ["banana"], ["cherry"]]

# 7. Batch fuzzy
curl -X POST http://localhost/batch/search_within_distance \
  -H 'Content-Type: application/json' \
  -d '{"words": ["aple", "bannana", "cheery"], "dist": 1}'
# [["apple"], ["banana"], ["cherry"]]
```

## Shared volume

A named Docker volume `snapshots` is mounted at `/snapshots` in both `writer` and `reader` containers. The writer writes snapshot files and `latest.json` there; readers read from the same path. `latest.json` is the fallback source of truth when Consul KV is empty.

## Running

```bash
# Build and start the full stack
docker compose up --build

# Scale readers
docker compose up --build --scale reader=4

# Tear down (removes containers but keeps the snapshots volume)
docker compose down

# Tear down and delete snapshot data
docker compose down -v
```

## Consul UI

Once running, the Consul UI is available at [http://localhost:8500](http://localhost:8500). You can inspect registered services (`lexrs-writer`, `lexrs-reader`) and browse the KV store at `lexrs/snapshot` to see the latest compacted version.

## Environment variables (writer)

| Variable | Default | Description |
|---|---|---|
| `WRITER_HOST` | `0.0.0.0` | Bind address |
| `WRITER_PORT` | `3000` | Listen port |
| `SNAPSHOT_DIR` | `/snapshots` | Shared volume mount |
| `CONSUL_ADDR` | `http://consul:8500` | Consul HTTP API |
| `COMPACT_INTERVAL` | `60` | Seconds between auto-compactions |
| `HOSTNAME` | `writer` | Used to build the health check URL registered with Consul |

## Environment variables (reader)

| Variable | Default | Description |
|---|---|---|
| `READER_HOST` | `0.0.0.0` | Bind address |
| `READER_PORT` | `3001` | Listen port |
| `SNAPSHOT_DIR` | `/snapshots` | Shared volume mount |
| `CONSUL_ADDR` | `http://consul:8500` | Consul HTTP API |

## Dockerfile

A single multi-stage `Dockerfile` builds both `writer` and `reader` binaries in one `cargo build` call, then copies both into a minimal `debian:bookworm-slim` image. Docker Compose selects which binary to run via the `command` field for each service. No Rust toolchain in the final image.
