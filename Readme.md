TJ Counter in Rust
==================

A lightweight Server-Sent Events (SSE) web app written in Rust with Hyper + Tokio.
The server publishes a live counter every 100ms, and the browser renders it instantly.

## What the counter shows
The counter targets the configured timestamp (`2022-02-11 16:00:00` UTC) and displays:

- `until ...` while the target is in the future
- `since ...` once the target has passed

Format:

`<direction> <days> days HH:MM:SS,t`

Where `t` is tenths of a second.

## Run locally

```bash
cargo run
```

Open: <http://localhost:8182>

## Quality checks

```bash
cargo check
cargo test
```

## Docker

Build image:

```bash
docker build --no-cache -f Dockerfile -t kbalashoff/tjcounter-rust .
```

Run container:

```bash
docker run -d -p 8182:8182 kbalashoff/tjcounter-rust
```

Pull from Docker Hub:

```bash
docker pull kbalashoff/tjcounter-rust
```
