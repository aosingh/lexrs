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

Consul must pass its health check (`consul members`) before writer or reader start. Writer must be started before readers because readers attempt to pull the latest snapshot on boot. The writer starts with an empty Trie and recovers only its version counter from Consul — no snapshot loading on startup.

## Nginx routing

| URL pattern | Proxied to |
|---|---|
| `/words`, `/compact`, `/snapshot/*` | `writer:3000` |
| `/search`, `/prefix`, `/contains`, `/stats` | `readers:3001` (round-robin) |
| `/health` | `readers:3001` |

All traffic enters on port **80**. Writer is also directly reachable on **3000** for debugging.

## Shared volume

A named Docker volume `snapshots` is mounted at `/snapshots` in both `writer` and `reader` containers. The writer writes snapshot files there; readers read from the same path.

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
