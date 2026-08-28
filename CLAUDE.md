# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Racemap Cell Service

Rust service that mirrors the OpenCellID cell tower dataset into MySQL/MariaDB and serves it over a warp HTTP API.

## Code standards

- Short sentences. RFC 2119 keywords for obligations.
- Commit = imperative subject; body only for a fact the diff cannot show.
- Comments only where code needs clarification — bare minimum (1-2 lines) — never narration.

## Commands

```bash
cargo run                                   # start service (needs .env, see below)
cargo test                                  # unit tests only
cargo test --features integration_tests     # + testcontainer tests (needs Docker running)
cargo test test_load_data_imports_csv_file  # single test by name filter
cargo test --features integration_tests query_cells_integration  # one integration module

diesel setup && diesel migration run        # create db + apply migrations
diesel print-schema > src/schema.rs         # regenerate schema after a migration
```

`CONFIG` panics at startup unless `DATABASE_URL` and `DOWNLOAD_SOURCE_TOKEN` are set. Copy `.env.example` to `.env` first.

Other env vars (all optional, `src/utils/config.rs`): `TEMP_FOLDER`, `DOWNLOAD_SOURCE_URL`, `PORT` (3000), `BIND` (0.0.0.0), `CORS_ORIGINS`, `SERVICE_NAME`, `RUST_LOG`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_TRACES_COLLECTOR_URL`, `OTEL_DEBUG_TRACES`.

## Architecture

`main.rs` spawns three tokio tasks and joins them; any one failing brings the process down:

1. `utils::data::update_loop` — ticks every second, checks for a dataset update every 600 ticks.
2. `process_handling` — waits on SIGTERM/SIGINT/Ctrl-C, flips the shared `HALT` mutex, then fires a oneshot that triggers warp's graceful shutdown.
3. `utils::server::start_server` — warp routes `/health`, `/cell`, `/cells`, `/carrier`.

### Sync pipeline

`update_type::get_update_type(last_update, now)` is a pure function deciding `None` / `Full` / `Diff` (skips before 4:00 UTC, full on first run or gaps > 1 day or month/year rollover). `url_builder` builds the OpenCellID URL, `data::load_url` streams the gzip response straight to a CSV file in `TEMP_FOLDER`, and `data::load_data_with_connection` imports it with a raw `LOAD DATA INFILE ... REPLACE INTO cells` statement — not Diesel inserts, because the full export is ~40M rows. The `last_updates` table records the timestamp and type of each successful import.

Consequence: the CSV must be readable **by the database server**, not the service. Integration tests copy the fixture into the container at `/var/lib/mysql-files/`, and `LOAD DATA INFILE` cannot run inside a transaction.

### Handlers

Each handler splits a pure `query_*(query, &mut MysqlConnection)` from the thin async `handle_*` warp wrapper, so the query logic is unit-testable against a container connection. Connections are established per request (`establish_connection`); there is no pool.

`/cells` uses cursor pagination: `CellCursor` base64-encodes the composite key `radio:mcc:net:area:cell` and `query_cells` fetches `limit + 1` rows to compute `hasMore`. The cursor comparison chain and the `ORDER BY` must stay in the same key order or pages will skip or repeat rows. Note the latest migration reordered the table primary key to `(mcc, net, area, cell, radio)` while `query_cells` still orders `radio` first — ordering no longer follows the index.

The README documents a `POST /cells/lookup` batch endpoint; it is not implemented in `src/`.

### Model conventions

- `Radio` serializes as `SCREAMING_SNAKE_CASE` in JSON but lowercase in MySQL. Adding a variant means: migration altering the enum, `ToSql`/`FromSql` arms in `models.rs`, and `CellCursor::encode`/`decode` arms.
- `Cell` is camelCase in JSON, accepts `range` as an alias for `cellRange`, and reads `changeable` from an int via `BoolFromInt`.
- `schema.rs` is Diesel-generated — edit migrations, then regenerate.
- `CellWithCarrier` wraps `Cell` for the wire (`#[serde(flatten)]` + `Deref`), adding `operator`,
  `country`, `countryCode`. `Cell` stays the Diesel row type and MUST keep matching the `cells`
  columns — never add response-only fields to it. The wrapper's inner field is `inner`, not `cell`,
  because `Cell` has its own `cell` field that a matching name would shadow.

### Carrier lookup

`GET /carrier?mcc=&net=` (alias `mnc`) exposes the lookup directly. It is the one handler that does
not follow the `query_*`/`handle_*` split: no DB, no `Config`, sync, and its route
(`server::carrier_route`) is extracted like `health_route` so `warp::test::request` can assert
status codes. Unknown MCC is `404` + `null`; unknown MNC under a known MCC stays `200`.

`utils::carrier::lookup(mcc, net)` resolves the human-readable operator and country from
`src/utils/mcc-mnc.csv`, compiled in via `include_str!` and parsed once into a `HashMap` on first
call. Deliberately not a DB table: ~3,600 rows changing a few times a year do not justify a
migration, a join, or a second download loop.

The CSV is **generated — never hand-edit**. Refresh with `python3 scripts/update-mcc-mnc.py` and
review the diff. A row with an empty `mnc` is that MCC's fallback country, used when the pair is
unknown. The script resolves every ambiguity at generation time (duplicate keys, non-alpha-2
country codes, multi-territory MNCs), so the Rust side is a plain map lookup.

## Testing

- Unit tests live in `#[cfg(test)] mod tests` inside the file they cover, grouped in nested `mod` blocks by subject.
- Integration tests are gated behind `#[cfg(feature = "integration_tests")]` and use `utils::test_db`, which starts a MariaDB 11.4 testcontainer with a uniquely named database per test and runs the embedded migrations. Keep the returned container bound (`let (_container, mut conn) = ...`) or it is dropped mid-test.
- `get_test_connection()` wraps the connection in a test transaction; pass `use_test_transaction: false` for `LOAD DATA INFILE`.
- CI runs `cargo test --features integration_tests`, so integration tests must pass before the Docker image builds.
