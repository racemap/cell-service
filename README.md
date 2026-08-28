# Cell Service

A Rust-based service for storing and querying cell tower location data. The service automatically syncs cell tower data from [OpenCellID](https://opencellid.org/) and provides a REST API for querying cell information.

## Features

- **Automatic Data Sync**: Periodically downloads and updates cell tower data from OpenCellID
- **REST API**: Query individual cells or fetch multiple cells with filtering and pagination
- **Geofence Filtering**: Filter cells by geographic bounding box
- **Network Filtering**: Filter by MCC (Mobile Country Code) and MNC (Mobile Network Code)
- **Radio Type Filtering**: Filter by radio technology (GSM, UMTS, CDMA, LTE, NR)
- **Cursor-based Pagination**: Efficiently paginate through large result sets
- **Carrier Lookup**: Resolves MCC/MNC to a human-readable operator, country and ISO country code

## Data Synchronization

The service automatically synchronizes cell tower data from [OpenCellID](https://opencellid.org/).

### Update Schedule

- **Check interval**: Every 10 minutes
- **Update window**: After 4:00 AM UTC (OpenCellID publishes new data at ~3:00 AM UTC)
- **Update types**:
  - **Full update**: Downloads the complete dataset (~2GB compressed). Triggered on first run, after gaps of more than 24 hours, or at month/year boundaries.
  - **Diff update**: Downloads only changes from the previous day (~few MB). Used for daily incremental updates when the last update was within 24 hours.

### How It Works

1. The service checks for updates every 10 minutes
2. Before 4:00 AM UTC, updates are skipped to wait for OpenCellID's daily data refresh
3. After 4:00 AM UTC, the service determines the update type based on the last successful update:
   - Same day: No update needed
   - Yesterday (within 24h): Download today's diff file
   - Older: Download full dataset

## Carrier Lookup

`/cell` and `/cells` add three human-readable fields to every cell:

| Field         | Derived from                                                                  | Example    |
| ------------- | ----------------------------------------------------------------------------- | ---------- |
| `operator`    | `mcc` + `net`                                                                 | `Vodafone` |
| `country`     | `mcc` + `net`, falling back to the country most of that MCC's networks are in | `Germany`  |
| `countryCode` | same as `country`, ISO 3166-1 alpha-2                                         | `DE`       |

Unknown values are `null` — never an empty string and never a guess. The three degrade
independently:

- **MCC and MNC both known** — all three populated.
- **MCC known, MNC not** — `operator` is `null`; `country` and `countryCode` still come from the
  MCC fallback.
- **MCC unknown** — all three `null`.

Clients MUST fall back to showing the raw numeric identifiers for whichever fields are `null`.

`country` and `countryCode` degrade independently: 107 rows carry a country with no code, because
the upstream code is a multi-territory list (`AU/CX/CC/NF`) or a subdivision (`GE-AB`) rather than
an alpha-2. Australia is the largest affected country. Do not treat a present `country` as a
guarantee that `countryCode` is present.

The table is compiled into the binary from `src/utils/mcc-mnc.csv`, which is **generated — do not
hand-edit**. It derives from the MIT-licensed, Wikipedia-sourced
[mcc-mnc-list](https://github.com/cavoq/mcc-mnc-list). To refresh:

```bash
python3 scripts/update-mcc-mnc.py     # rewrites src/utils/mcc-mnc.csv
git diff src/utils/mcc-mnc.csv        # review like any other change
```

Where one MNC is registered across several territories (Airtel-Vodafone in Guernsey, Jersey and
the UK; Docomo in Guam, the Northern Marianas and the USA) no MNC can disambiguate them, so the
table reports the umbrella country rather than guessing a territory.

## Requirements

- Rust 1.98.0 (pinned in `rust-toolchain.toml`; requires rustup)
- MySQL/MariaDB database
- Docker (optional, for containerized deployment)

## Environment Variables

| Variable                | Description                                                                        | Example                                 |
| ----------------------- | ---------------------------------------------------------------------------------- | --------------------------------------- |
| `DATABASE_URL`          | MySQL connection string                                                            | `mysql://user:pass@localhost/cells`     |
| `RUST_LOG`              | Log level                                                                          | `info`                                  |
| `DOWNLOAD_SOURCE_TOKEN` | API key for OpenCellID downloads                                                   | `your-api-key`                          |
| `CORS_ORIGINS`          | Comma-separated list of allowed CORS origins (if not set, all origins are allowed) | `https://example.com,https://other.com` |

## Getting Started

### Local Development

1. **Clone the repository**
   ```bash
   git clone https://github.com/racemap/cell-service.git
   cd cell-service
   ```

2. **Set up environment**
   ```bash
   cp .env.example .env
   # Edit .env with your database credentials and API key
   ```

3. **Set up the database**
   ```bash
   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/diesel-rs/diesel/releases/latest/download/diesel_cli-installer.sh | sh   
   diesel setup
   ```

4. **Run the service**
   ```bash
   cargo run
   ```

### Docker

```bash
docker build -t cell-service .
docker run -e DATABASE_URL="mysql://user:pass@host/db" -e DOWNLOAD_SOURCE_TOKEN="key" -p 3000:3000 cell-service
```

## API Reference

The service runs on port `3000` by default.

### Health Check

Check if the service is running.

```
GET /health
```

**Response:** `OK`

---

### Get Single Cell

Retrieve a specific cell tower by its identifiers.

```
GET /cell?mcc=<mcc>&net=<mnc>&area=<lac>&cell=<cid>[&radio=<radio>]
```

**Parameters:**

| Parameter | Type    | Required | Description                                    |
| --------- | ------- | -------- | ---------------------------------------------- |
| `mcc`     | integer | Yes      | Mobile Country Code                            |
| `net`     | integer | Yes      | Mobile Network Code                            |
| `area`    | integer | Yes      | Location Area Code                             |
| `cell`    | integer | Yes      | Cell ID                                        |
| `radio`   | string  | No       | Radio type: `GSM`, `UMTS`, `CDMA`, `LTE`, `NR` |

**Example:**
```bash
curl "http://localhost:3000/cell?mcc=262&net=1&area=12345&cell=67890"
```

**Response:**
```json
{
  "radio": "LTE",
  "mcc": 262,
  "net": 1,
  "area": 12345,
  "cell": 67890,
  "unit": 1,
  "lon": 13.405,
  "lat": 52.52,
  "cellRange": 1000,
  "samples": 50,
  "changeable": 1,
  "created": "2024-01-15T10:30:00Z",
  "updated": "2025-12-20T14:00:00Z",
  "averageSignal": -85,
  "operator": "Telekom",
  "country": "Germany",
  "countryCode": "DE"
}
```

Returns `null` if no cell is found.

---

### Get Carrier

Resolve the operator and country for an MCC/MNC pair, without needing a cell.

```
GET /carrier?mcc=<mcc>&net=<mnc>
```

**Parameters:**

| Parameter | Type    | Required | Description                                       |
| --------- | ------- | -------- | ------------------------------------------------- |
| `mcc`     | integer | Yes      | Mobile Country Code                               |
| `net`     | integer | Yes      | Mobile Network Code. Also accepted as `mnc`       |

**Example:**
```bash
curl "http://localhost:3000/carrier?mcc=262&net=2"
```

**Response:**
```json
{
  "operator": "Vodafone",
  "country": "Germany",
  "countryCode": "DE"
}
```

Served from the compiled-in table described under [Carrier Lookup](#carrier-lookup); no database
access, so the same null contract applies. An MNC that is unknown under an MCC that has a country
fallback row still returns `200` with `operator: null` and the country populated — 232 of the 238
MCCs in the table have such a row.

**This endpoint returns `404` with a body of `null` when the lookup resolves nothing at all** —
either the MCC is absent from the table, or it is one of the six MCCs that have no country fallback
row (`1`, `901`, `902`, `991`, `995`, `999`: test, satellite and internal-use ranges) and the MNC
did not match either. In both cases there is no country to report, so a `200` would carry three
nulls.

This differs from `/cell`, which answers a miss with `200` and a body of `null`. Do not infer one
from the other.

---

### Get Multiple Cells

Retrieve multiple cells with optional filtering and cursor-based pagination.

```
GET /cells?[mcc=<mcc>][&mnc=<mnc>][&min_lat=<lat>][&max_lat=<lat>][&min_lon=<lon>][&max_lon=<lon>][&radio=<radio>][&cursor=<cursor>][&limit=<limit>]
```

**Parameters:**

| Parameter | Type    | Required | Description                                              |
| --------- | ------- | -------- | -------------------------------------------------------- |
| `mcc`     | integer | No       | Filter by Mobile Country Code                            |
| `mnc`     | integer | No       | Filter by Mobile Network Code                            |
| `min_lat` | float   | No       | Minimum latitude (geofence)                              |
| `max_lat` | float   | No       | Maximum latitude (geofence)                              |
| `min_lon` | float   | No       | Minimum longitude (geofence)                             |
| `max_lon` | float   | No       | Maximum longitude (geofence)                             |
| `radio`   | string  | No       | Filter by radio type: `GSM`, `UMTS`, `CDMA`, `LTE`, `NR` |
| `cursor`  | string  | No       | Pagination cursor from previous response                 |
| `limit`   | integer | No       | Results per page (default: 100, max: 1000)               |

**Example - Get all cells in Germany (MCC 262):**
```bash
curl "http://localhost:3000/cells?mcc=262&limit=100"
```

**Example - Get LTE cells in Berlin area:**
```bash
curl "http://localhost:3000/cells?mcc=262&min_lat=52.3&max_lat=52.7&min_lon=13.1&max_lon=13.8&radio=LTE&limit=50"
```

**Response:**
```json
{
  "cells": [
    {
      "radio": "LTE",
      "mcc": 262,
      "net": 1,
      "area": 12345,
      "cell": 67890,
      "unit": 1,
      "lon": 13.405,
      "lat": 52.52,
      "cellRange": 1000,
      "samples": 50,
      "changeable": 1,
      "created": "2024-01-15T10:30:00Z",
      "updated": "2025-12-20T14:00:00Z",
      "averageSignal": -85,
      "operator": "Telekom",
      "country": "Germany",
      "countryCode": "DE"
    }
  ],
  "nextCursor": "TFRFOjI2MjoxOjEyMzQ1OjY3ODkw",
  "hasMore": true
}
```

**Pagination:**

To fetch the next page, include the `nextCursor` value from the previous response along with the same filter parameters:

```bash
# First request
curl "http://localhost:3000/cells?mcc=262&min_lat=52.0&max_lat=53.0&limit=100"

# Next page (use same filters + cursor)
curl "http://localhost:3000/cells?mcc=262&min_lat=52.0&max_lat=53.0&limit=100&cursor=TFRFOjI2MjoxOjEyMzQ1OjY3ODkw"
```

When `hasMore` is `false`, there are no more results.

---

### Lookup Multiple Cells (Batch)

> **Not implemented.** No such route is registered in `src/utils/server.rs`; requests to it will
> 404. This section is a design sketch for a planned endpoint, kept for reference. Do not build
> against it.

Look up multiple cells by `(mcc, mnc, lac, cid)` in a single request.

This endpoint returns **one best match per input key**, aligned 1:1 with the request order.

```
POST /cells/lookup
```

**Request Body:**

```json
{
  "cells": [
    {"mcc": 262, "mnc": 1, "lac": 12345, "cid": 67890},
    {"mcc": 262, "mnc": 1, "lac": 124, "cid": 457}
  ]
}
```

**Notes / Constraints:**

- Max keys per request: **50**. If more are sent, the response is padded with `null` for the excess entries.
- If multiple rows exist for the same `(mcc, mnc, lac, cid)` (e.g. different radios), the service picks a single deterministic “best” row:
  - Higher `samples`
  - Newer `updated`
  - Higher radio generation (`NR` > `LTE` > `UMTS` > `GSM` > `CDMA`)

**Example:**

```bash
curl -X POST "http://localhost:3000/cells/lookup" \
  -H "Content-Type: application/json" \
  -d '{
    "cells": [
      {"mcc": 262, "mnc": 1, "lac": 12345, "cid": 67890},
      {"mcc": 999, "mnc": 999, "lac": 999, "cid": 999}
    ]
  }'
```

**Response:**

```json
{
  "cells": [
    {
      "radio": "LTE",
      "mcc": 262,
      "net": 1,
      "area": 12345,
      "cell": 67890,
      "unit": 1,
      "lon": 13.405,
      "lat": 52.52,
      "cellRange": 1000,
      "samples": 50,
      "changeable": 1,
      "created": "2024-01-15T10:30:00Z",
      "updated": "2025-12-20T14:00:00Z",
      "averageSignal": -85,
      "operator": "Telekom",
      "country": "Germany",
      "countryCode": "DE"
    },
    null
  ]
}
```

## Running Tests

```bash
# Unit tests
cargo test

# Integration tests (requires Docker)
cargo test --features integration_tests
```

## License

See [LICENSE](LICENSE) for details.
