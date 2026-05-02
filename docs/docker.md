# Docker Setup

The `docker/` directory has a Compose file that brings up the full stack in one command. This page walks through what each service does, how they connect, and how to interact with the running system.

## Prerequisites

- Docker with the Compose plugin (v2.x — the `docker compose` command, not `docker-compose`)
- Ports 80, 3000, and 8500 available on your machine

---

## Start

```bash
cd docker
docker compose up -d
```

The first run builds the `lexrs-server` image from source (this takes a minute or two). Subsequent starts use the cached image.

Services start in dependency order. Consul must pass its health check before the writer starts, and the writer must pass its health check before the readers start. You can watch this happen:

```bash
docker compose logs -f
```

You should see something like:

```
consul   | ==> Consul agent running!
writer   | [startup] no snapshot found, starting fresh
writer   | lexrs-writer listening on http://0.0.0.0:3000
reader-1 | No snapshot found, starting with empty DAWG
reader-1 | lexrs-reader listening on http://0.0.0.0:3001
reader-2 | No snapshot found, starting with empty DAWG
reader-2 | lexrs-reader listening on http://0.0.0.0:3001
nginx    | ... ready
```

---

## What is running

```
your machine :80
       │
       ▼
    nginx
    ├── /words, /compact  ──────────────▶  writer :3000
    │                                          │
    │                                    compacts every 60s
    │                                          │
    │                                          ▼
    │                                   /snapshots volume
    │                                          │
    └── /search, /prefix, /contains ──▶  reader-1 :3001 ◀─── hot-reload via Consul
                   (round-robin)         reader-2 :3001 ◀─── hot-reload via Consul
```

**Consul** runs in dev mode on port 8500. It holds two things: service registrations (so Consul knows which instances are healthy) and a single KV entry (`lexrs/snapshot`) that the writer updates after every compaction and the readers watch via long-poll.

**The shared volume** (`snapshots`) is mounted into both the writer and readers. The writer writes snapshot files there; readers read from it. They never talk to each other directly.

**nginx** routes by URL path. Write endpoints go to the writer; read endpoints are distributed round-robin across all healthy readers. You don't need to know which reader handled a request — they all serve from the same snapshot.

---

## Try it out

### 1. Ingest some words

```bash
curl -s -X POST http://localhost/words \
  -H 'Content-Type: application/json' \
  -d '{
    "words": [
      {"word": "apple",   "count": 10},
      {"word": "apply",   "count": 3},
      {"word": "apt",     "count": 1},
      {"word": "banana",  "count": 7},
      {"word": "band",    "count": 4},
      {"word": "bandana", "count": 2}
    ]
  }'
```

```json
{"inserted": 6}
```

At this point the words are in the writer's in-memory Trie. Search queries will return empty results because the readers haven't received a snapshot yet.

### 2. Compact

```bash
curl -s -X POST http://localhost/compact
```

```json
{"status": "ok", "version": 1}
```

The writer merged the Trie with the (empty) previous snapshot, wrote `snapshot_1.txt` to the shared volume, and updated the Consul KV entry. You'll see in the logs:

```
writer   | [compact] v1: merged 6 new words
reader-1 | [watch] new snapshot v1 at /snapshots/snapshot_1.txt
reader-1 | [watch] reloaded DAWG from v1
reader-2 | [watch] new snapshot v1 at /snapshots/snapshot_1.txt
reader-2 | [watch] reloaded DAWG from v1
```

### 3. Search

```bash
# Wildcard
curl -s 'http://localhost/search?q=ap*'
# ["apple","apply","apt"]

# Wildcard with counts
curl -s 'http://localhost/search?q=ap*&with_count=true'
# [{"word":"apple","count":10},{"word":"apply","count":3},{"word":"apt","count":1}]

# Fuzzy — "aple" is one edit away from "apple"
curl -s 'http://localhost/search?q=aple&dist=1'
# ["apple"]

# Prefix completion
curl -s 'http://localhost/prefix?q=ban'
# ["banana","band","bandana"]

# Exact lookup
curl -s 'http://localhost/contains?q=apple'
# {"found":true}

curl -s 'http://localhost/contains?q=apricot'
# {"found":false}
```

### 4. Ingest more words and watch the reload

```bash
curl -s -X POST http://localhost/words \
  -H 'Content-Type: application/json' \
  -d '{"words": ["cherry", "cranberry", "citrus"]}'

curl -s -X POST http://localhost/compact
```

Both readers will log the reload. The new words are immediately searchable:

```bash
curl -s 'http://localhost/search?q=c*'
# ["cherry","citrus","cranberry"]
```

---

## Inspect the snapshot

The snapshot file is plain text — you can read it directly from inside the writer container:

```bash
docker compose exec writer cat /snapshots/snapshot_1.txt
```

```
apple 10
apply 3
apt 1
band 4
bandana 2
banana 7
```

Words are sorted alphabetically. Counts accumulate across compactions — if you post `apple` again and compact, you'll see `apple 20` in the next version.

---

## Consul UI

Open [http://localhost:8500](http://localhost:8500) in your browser while the stack is running.

- **Services tab**: shows `lexrs-writer` and `lexrs-reader` with green health indicators.
- **Key/Value tab → lexrs → snapshot**: shows the JSON the writer published after the last compaction, e.g. `{"version":1,"path":"/snapshots/snapshot_1.txt"}`. This is exactly what each reader receives when its long-poll fires.

---

## Scale readers

To run more reader replicas:

```bash
docker compose up -d --scale reader=4
```

nginx picks up the new instances automatically and starts routing to them. Each new reader loads the latest snapshot from the shared volume on startup — no manual configuration needed.

---

## Stop

```bash
# Stop containers, keep the snapshot volume (data preserved)
docker compose down

# Stop containers and delete the snapshot volume
docker compose down -v
```

After `docker compose down -v`, the next `up` starts fresh with an empty lexicon.

---

## Automatic compaction

In production you would not call `POST /compact` manually. The writer compacts automatically every `COMPACT_INTERVAL` seconds (default: 60). To change it, set the environment variable in `docker-compose.yml`:

```yaml
writer:
  environment:
    COMPACT_INTERVAL: "30"   # compact every 30 seconds
```

Choose an interval that balances write latency (how long before words are visible to readers) against compaction overhead (merging two sorted files on every cycle).
