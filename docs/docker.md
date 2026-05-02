# Docker Setup

The `docker/` directory contains a Docker Compose file that brings up the full stack — Consul, writer, two reader replicas, and an nginx reverse proxy — in a single command.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) with the Compose plugin (v2.x)
- Ports 80, 3000, and 8500 free on your machine

---

## Start the stack

```bash
cd docker
docker compose up -d
```

This builds the `lexrs-server` image and starts:

| Service | Role | Port |
|---|---|---|
| `consul` | Service registry and KV store | 8500 |
| `writer` | Word ingestion and compaction | 3000 |
| `reader` (× 2) | DAWG search, auto-reloads snapshots | 3001 |
| `nginx` | Reverse proxy — routes writes to writer, reads to readers | 80 |

The services start in dependency order: Consul must be healthy before the writer, and the writer must be healthy before the readers.

---

## Stack layout

```
nginx :80
  ├── /words, /compact, /snapshot  ──▶  writer :3000
  └── /search, /prefix, /contains  ──▶  reader :3001 (round-robin)

writer ──▶ /snapshots (Docker volume) ◀──  reader × 2
writer ──▶ Consul KV  ──blocking query──▶  reader × 2
```

nginx routes by path prefix:

- Write paths (`/words`, `/compact`, `/snapshot`) → writer
- Read paths (`/search`, `/prefix`, `/contains`, `/stats`) → readers (round-robin)

---

## Using the stack

All examples below use port 80 (nginx). You can also talk to the writer directly on port 3000.

### Ingest words

```bash
# Uniform count
curl -X POST http://localhost/words \
  -H 'Content-Type: application/json' \
  -d '{"words": ["apple", "apply", "apt", "banana", "band", "bandana"]}'

# Per-word counts
curl -X POST http://localhost/words \
  -H 'Content-Type: application/json' \
  -d '{
    "words": [
      {"word": "apple",   "count": 10},
      {"word": "apply",   "count": 3},
      {"word": "banana",  "count": 7},
      "apt"
    ]
  }'
```

### Trigger compaction

Compaction merges the in-memory delta Trie with the previous snapshot and notifies all readers via Consul. Words become visible to search immediately after compaction.

```bash
curl -X POST http://localhost/compact
# {"status": "ok", "version": 1}
```

!!! tip "Automatic compaction"
    The writer compacts automatically every `COMPACT_INTERVAL` seconds (default: 60). Use `POST /compact` to flush immediately during testing or after a bulk load.

### Search

```bash
# Wildcard
curl 'http://localhost/search?q=ap*'
# ["apple", "apply", "apt"]

# Wildcard with counts
curl 'http://localhost/search?q=ap*&with_count=true'
# [{"word":"apple","count":10}, {"word":"apply","count":3}, {"word":"apt","count":1}]

# Fuzzy (Levenshtein ≤ 1)
curl 'http://localhost/search?q=aple&dist=1'
# ["apple"]

# Prefix completion
curl 'http://localhost/prefix?q=ban'
# ["banana", "band", "bandana"]

# Exact lookup
curl 'http://localhost/contains?q=apple'
# {"found": true}
```

### Stats

```bash
# Reader stats (compacted DAWG — all words visible to search)
curl 'http://localhost/stats'
# {"words": 21, "nodes": 84}

# Writer stats (live delta Trie — words pending next compaction)
curl 'http://localhost:3000/stats'
# {"words": 0, "nodes": 1}
```

---

## Configuration

Environment variables are set in `docker-compose.yml`. The defaults are:

```yaml
writer:
  environment:
    WRITER_HOST:       "0.0.0.0"
    WRITER_PORT:       "3000"
    SNAPSHOT_DIR:      "/snapshots"
    CONSUL_ADDR:       "http://consul:8500"
    COMPACT_INTERVAL:  "60"       # seconds between auto-compactions

reader:
  environment:
    READER_HOST:  "0.0.0.0"
    READER_PORT:  "3001"
    SNAPSHOT_DIR: "/snapshots"
    CONSUL_ADDR:  "http://consul:8500"
```

---

## Scaling readers

To run more reader replicas, use the `--scale` flag:

```bash
docker compose up -d --scale reader=4
```

nginx automatically load-balances across all healthy reader instances. New readers pick up the latest snapshot from Consul on startup — no manual intervention needed.

---

## Stopping the stack

```bash
docker compose down
```

To also remove the snapshot volume (all ingested data):

```bash
docker compose down -v
```

---

## Consul UI

While the stack is running, the Consul web UI is available at [http://localhost:8500](http://localhost:8500). You can inspect:

- Registered services (`lexrs-writer`, `lexrs-reader`)
- Health check status for each instance
- The KV store entry at `lexrs/snapshot` — this shows the current snapshot version and path that readers are watching
