# Docker fixtures

Sample outputs of the read-only Docker commands Freeboard runs, in the JSON
shapes of Docker CLI 29.x (`--format '{{json .}}'` line output for `ps`,
`image ls`, `volume ls`, `system df`; JSON arrays for `container inspect` and
`image inspect`). `version_down.json` was captured from the installed Docker
29.8.2 CLI with the daemon stopped. Tests copy these into a temporary state
directory read by a fake `docker` executable; no real Docker command that
changes anything is ever run.

Scenario: `web` runs and mounts `web_data`; `old-cache` stopped on 2026-09-01
and references `redis:7` and `cache_vol`; `recent` stopped 3 days before the
test clock and references dangling image `dd3`. Expected suggestions: build
cache (Safe), dangling `dd1` + `dd2` (Safe), `postgres:14` (Review), stopped
`old-cache` (Review), volume `billing_pgdata` (Risky, needs acknowledgment).
