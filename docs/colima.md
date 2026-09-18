# Colima (containers without Docker Desktop)

Agents and local scripts should use this **mise-pinned** stack instead of Docker Desktop.
It is ~Docker-compatible (Docker Engine API via Colima). It is **not** Podman.

## Stack

```
docker / docker compose  →  Colima (Docker runtime)  →  Lima VM
```

| Piece | Role |
| --- | --- |
| `docker-cli` | `docker` client |
| `docker-compose` | standalone `docker-compose` binary (Compose v5); optional cli-plugin for `docker compose` |
| `colima` | starts/stops the Docker-compatible daemon inside a VM |
| `lima` | VM backend Colima drives (you rarely invoke `limactl` directly) |

Pins live in `mise.toml` (`colima` 0.10.3, `lima` 2.2.0, `docker-cli` 29.8.1, `docker-compose` 5.5.1). Colima and Lima install on Linux and macOS only.

## Install

From the repo root:

```bash
mise install
```

Or install just the stack: `mise install colima lima docker-cli docker-compose`.

mise installs Compose as the **`docker-compose`** binary. That is enough for scripts and agents.
To also get the `docker compose` subcommand, link the plugin once:

```bash
mkdir -p ~/.docker/cli-plugins
ln -sf "$(mise which docker-compose)" ~/.docker/cli-plugins/docker-compose
```

## Start and verify

```bash
colima start
docker context ls          # expect a `colima` context
docker context use colima  # if it is not already current
docker ps                  # must talk to the Colima daemon
```

Agents must prefer `docker` / `docker-compose` (or `docker compose` if the cli-plugin is linked) against Colima. **Do not** require Docker Desktop, and do not tell the user to install it for rtok.

## Scripts that call `docker run`

Recipes in `docs/otel.md` (Jaeger, Grafana `otel-lgtm`, SigNoz compose) and any `docker run` / `docker compose` / `docker-compose` one-liners work unchanged **once Colima is up** and the active Docker context points at it.

## Stop

```bash
colima stop
```

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `Cannot connect to the Docker daemon` / daemon not running | `colima start`, then `docker context use colima` |
| `docker: command not found` | `mise install` (or `mise exec -- docker …`) |
| `colima: command not found` | same — pins are in `mise.toml` |
| Wrong engine / Desktop still selected | `docker context ls` and `docker context use colima` |

## Compatibility note

This stack targets **Docker CLI + Compose** talking to Colima’s Docker runtime. Do not substitute Podman or assume rootless Podman sockets unless a separate task says so.
