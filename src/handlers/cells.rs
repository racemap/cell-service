use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::utils::config::Config;
use crate::{models::*, utils::db::establish_connection};
use diesel::prelude::*;
use diesel::MysqlConnection;

/// Query parameters for fetching multiple cells with pagination and filtering.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GetCellsQuery {
    /// Mobile Country Code filter
    pub mcc: Option<u16>,
    /// Mobile Network Code filter
    pub mnc: Option<u16>,
    /// Minimum latitude for geofence
    pub min_lat: Option<f32>,
    /// Maximum latitude for geofence
    pub max_lat: Option<f32>,
    /// Minimum longitude for geofence
    pub min_lon: Option<f32>,
    /// Maximum longitude for geofence
    pub max_lon: Option<f32>,
    /// Radio type filter
    pub radio: Option<Radio>,
    /// Cursor for pagination (cell ID to start after)
    pub cursor: Option<String>,
    /// Number of items per page (default: 100, max: 1000)
    pub limit: Option<u32>,
}

/// Response for paginated cells endpoint.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetCellsResponse {
    /// The list of cells
    pub cells: Vec<Cell>,
    /// The cursor for the next page, if there are more results
    pub next_cursor: Option<String>,
    /// Whether there are more results
    pub has_more: bool,
}

/// Represents a cursor for pagination, encoding the composite primary key.
#[derive(Debug, Clone)]
pub struct CellCursor {
    pub radio: Radio,
    pub mcc: u16,
    pub net: u16,
    pub area: u32,
    pub cell: u64,
}

impl CellCursor {
    /// Encode the cursor as a base64 string.
    pub fn encode(&self) -> String {
        let radio_str = match self.radio {
            Radio::Gsm => "GSM",
            Radio::Umts => "UMTS",
            Radio::Cdma => "CDMA",
            Radio::Lte => "LTE",
            Radio::Nr => "NR",
        };
        let raw = format!(
            "{}:{}:{}:{}:{}",
            radio_str, self.mcc, self.net, self.area, self.cell
        );
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        URL_SAFE_NO_PAD.encode(raw.as_bytes())
    }

    /// Decode a cursor from a base64 string.
    pub fn decode(encoded: &str) -> Option<Self> {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
        let raw = String::from_utf8(bytes).ok()?;
        let parts: Vec<&str> = raw.split(':').collect();
        if parts.len() != 5 {
            return None;
        }

        let radio = match parts[0] {
            "GSM" => Radio::Gsm,
            "UMTS" => Radio::Umts,
            "CDMA" => Radio::Cdma,
            "LTE" => Radio::Lte,
            "NR" => Radio::Nr,
            _ => return None,
        };

        Some(CellCursor {
            radio,
            mcc: parts[1].parse().ok()?,
            net: parts[2].parse().ok()?,
            area: parts[3].parse().ok()?,
            cell: parts[4].parse().ok()?,
        })
    }

    /// Create a cursor from a Cell.
    pub fn from_cell(cell: &Cell) -> Self {
        CellCursor {
            radio: cell.radio.clone(),
            mcc: cell.mcc,
            net: cell.net,
            area: cell.area,
            cell: cell.cell,
        }
    }
}

const DEFAULT_PAGE_SIZE: u32 = 100;
const MAX_PAGE_SIZE: u32 = 1000;

/// Queries multiple cells from the database with pagination and filtering.
#[instrument(skip(connection))]
pub fn query_cells(
    query: &GetCellsQuery,
    connection: &mut MysqlConnection,
) -> Result<GetCellsResponse, diesel::result::Error> {
    use crate::schema::cells::dsl::*;

    let page_limit = query.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    // Fetch one extra to check if there are more results
    let fetch_limit = (page_limit + 1) as i64;

    let mut db_query = cells.into_boxed();

    // Apply MCC filter
    if let Some(mcc_filter) = query.mcc {
        db_query = db_query.filter(mcc.eq(mcc_filter));
    }

    // Apply MNC filter (net column)
    if let Some(mnc_filter) = query.mnc {
        db_query = db_query.filter(net.eq(mnc_filter));
    }

    // Apply radio filter
    if let Some(ref radio_filter) = query.radio {
        db_query = db_query.filter(radio.eq(radio_filter));
    }

    // Apply geofence filters
    if let Some(min_lat_filter) = query.min_lat {
        db_query = db_query.filter(lat.ge(min_lat_filter));
    }
    if let Some(max_lat_filter) = query.max_lat {
        db_query = db_query.filter(lat.le(max_lat_filter));
    }
    if let Some(min_lon_filter) = query.min_lon {
        db_query = db_query.filter(lon.ge(min_lon_filter));
    }
    if let Some(max_lon_filter) = query.max_lon {
        db_query = db_query.filter(lon.le(max_lon_filter));
    }

    // Apply cursor-based pagination
    // We order by the composite primary key (radio, mcc, net, area, cell)
    // and use tuple comparison for cursor
    if let Some(ref cursor_str) = query.cursor {
        if let Some(cursor) = CellCursor::decode(cursor_str) {
            // For cursor pagination with composite keys, we need to find rows
            // that come after the cursor in the sorted order.
            // Using tuple comparison: (radio, mcc, net, area, cell) > (cursor values)
            let cursor_radio = cursor.radio.clone();
            let cursor_mcc = cursor.mcc;
            let cursor_net = cursor.net;
            let cursor_area = cursor.area;
            let cursor_cell = cursor.cell;

            db_query = db_query.filter(
                radio
                    .gt(cursor_radio.clone())
                    .or(radio.eq(cursor_radio.clone()).and(mcc.gt(cursor_mcc)))
                    .or(radio
                        .eq(cursor_radio.clone())
                        .and(mcc.eq(cursor_mcc))
                        .and(net.gt(cursor_net)))
                    .or(radio
                        .eq(cursor_radio.clone())
                        .and(mcc.eq(cursor_mcc))
                        .and(net.eq(cursor_net))
                        .and(area.gt(cursor_area)))
                    .or(radio
                        .eq(cursor_radio)
                        .and(mcc.eq(cursor_mcc))
                        .and(net.eq(cursor_net))
                        .and(area.eq(cursor_area))
                        .and(cell.gt(cursor_cell))),
            );
        }
    }

    // Order by composite primary key for consistent pagination
    db_query = db_query
        .order((radio.asc(), mcc.asc(), net.asc(), area.asc(), cell.asc()))
        .limit(fetch_limit);

    let mut results: Vec<Cell> = db_query.load(connection)?;

    // Check if there are more results
    let has_more = results.len() > page_limit as usize;
    if has_more {
        results.pop(); // Remove the extra item
    }

    // Generate next cursor from the last item
    let next_cursor = if has_more {
        results.last().map(|c| CellCursor::from_cell(c).encode())
    } else {
        None
    };

    Ok(GetCellsResponse {
        cells: results,
        next_cursor,
        has_more,
    })
}

#[instrument]
pub async fn handle_get_cells(
    query: GetCellsQuery,
    config: Config,
) -> Result<impl warp::Reply, warp::Rejection> {
    let connection = &mut establish_connection(config.clone());

    match query_cells(&query, connection) {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(_) => Ok(warp::reply::json(&GetCellsResponse {
            cells: vec![],
            next_cursor: None,
            has_more: false,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod cell_cursor {
        use super::*;

        #[test]
        fn test_encode_decode_roundtrip() {
            let cursor = CellCursor {
                radio: Radio::Lte,
                mcc: 262,
                net: 1,
                area: 12345,
                cell: 67890,
            };

            let encoded = cursor.encode();
            let decoded = CellCursor::decode(&encoded).unwrap();

            assert!(matches!(decoded.radio, Radio::Lte));
            assert_eq!(decoded.mcc, 262);
            assert_eq!(decoded.net, 1);
            assert_eq!(decoded.area, 12345);
            assert_eq!(decoded.cell, 67890);
        }

        #[test]
        fn test_decode_invalid_base64() {
            let result = CellCursor::decode("not-valid-base64!!!");
            assert!(result.is_none());
        }

        #[test]
        fn test_decode_invalid_format() {
            use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
            let invalid = URL_SAFE_NO_PAD.encode(b"only:two:parts");
            let result = CellCursor::decode(&invalid);
            assert!(result.is_none());
        }

        #[test]
        fn test_decode_invalid_radio() {
            use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
            let invalid = URL_SAFE_NO_PAD.encode(b"INVALID:262:1:100:200");
            let result = CellCursor::decode(&invalid);
            assert!(result.is_none());
        }

        #[test]
        fn test_decode_invalid_numbers() {
            use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
            let invalid = URL_SAFE_NO_PAD.encode(b"LTE:abc:1:100:200");
            let result = CellCursor::decode(&invalid);
            assert!(result.is_none());
        }

        #[test]
        fn test_all_radio_types() {
            let radio_types = vec![
                (Radio::Gsm, "GSM"),
                (Radio::Umts, "UMTS"),
                (Radio::Cdma, "CDMA"),
                (Radio::Lte, "LTE"),
                (Radio::Nr, "NR"),
            ];

            for (radio, expected_str) in radio_types {
                let cursor = CellCursor {
                    radio: radio.clone(),
                    mcc: 1,
                    net: 2,
                    area: 3,
                    cell: 4,
                };
                let encoded = cursor.encode();
                let decoded = CellCursor::decode(&encoded).unwrap();

                // Verify the encoded string contains the expected radio type
                use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
                let raw = String::from_utf8(URL_SAFE_NO_PAD.decode(&encoded).unwrap()).unwrap();
                assert!(raw.starts_with(expected_str));

                // Verify roundtrip works
                assert_eq!(decoded.mcc, 1);
            }
        }

        #[test]
        fn test_from_cell() {
            let cell = Cell {
                radio: Radio::Nr,
                mcc: 310,
                net: 410,
                area: 5000,
                cell: 6000,
                unit: Some(1),
                lon: 13.0,
                lat: 52.0,
                cell_range: 500,
                samples: 10,
                changeable: false,
                created: chrono::NaiveDateTime::default(),
                updated: chrono::NaiveDateTime::default(),
                average_signal: None,
            };

            let cursor = CellCursor::from_cell(&cell);

            assert!(matches!(cursor.radio, Radio::Nr));
            assert_eq!(cursor.mcc, 310);
            assert_eq!(cursor.net, 410);
            assert_eq!(cursor.area, 5000);
            assert_eq!(cursor.cell, 6000);
        }
    }

    mod get_cells_query {
        use super::*;

        #[test]
        fn test_deserialize_all_fields() {
            let json = r#"{
                "mcc": 262,
                "mnc": 1,
                "min_lat": 52.0,
                "max_lat": 53.0,
                "min_lon": 13.0,
                "max_lon": 14.0,
                "radio": "LTE",
                "cursor": "abc123",
                "limit": 50
            }"#;

            let query: GetCellsQuery = serde_json::from_str(json).unwrap();

            assert_eq!(query.mcc, Some(262));
            assert_eq!(query.mnc, Some(1));
            assert_eq!(query.min_lat, Some(52.0));
            assert_eq!(query.max_lat, Some(53.0));
            assert_eq!(query.min_lon, Some(13.0));
            assert_eq!(query.max_lon, Some(14.0));
            assert!(matches!(query.radio, Some(Radio::Lte)));
            assert_eq!(query.cursor, Some("abc123".to_string()));
            assert_eq!(query.limit, Some(50));
        }

        #[test]
        fn test_deserialize_empty_query() {
            let json = r#"{}"#;

            let query: GetCellsQuery = serde_json::from_str(json).unwrap();

            assert!(query.mcc.is_none());
            assert!(query.mnc.is_none());
            assert!(query.min_lat.is_none());
            assert!(query.max_lat.is_none());
            assert!(query.min_lon.is_none());
            assert!(query.max_lon.is_none());
            assert!(query.radio.is_none());
            assert!(query.cursor.is_none());
            assert!(query.limit.is_none());
        }

        #[test]
        fn test_deserialize_from_query_string() {
            let query_string =
                "mcc=262&mnc=1&min_lat=52.0&max_lat=53.0&min_lon=13.0&max_lon=14.0&limit=100";

            let query: GetCellsQuery = serde_urlencoded::from_str(query_string).unwrap();

            assert_eq!(query.mcc, Some(262));
            assert_eq!(query.mnc, Some(1));
            assert_eq!(query.min_lat, Some(52.0));
            assert_eq!(query.max_lat, Some(53.0));
            assert_eq!(query.min_lon, Some(13.0));
            assert_eq!(query.max_lon, Some(14.0));
            assert_eq!(query.limit, Some(100));
        }

        #[test]
        fn test_deserialize_partial_geofence() {
            let query_string = "min_lat=52.0&max_lat=53.0";

            let query: GetCellsQuery = serde_urlencoded::from_str(query_string).unwrap();

            assert_eq!(query.min_lat, Some(52.0));
            assert_eq!(query.max_lat, Some(53.0));
            assert!(query.min_lon.is_none());
            assert!(query.max_lon.is_none());
        }
    }

    /// Integration tests for query_cells using testcontainers.
    #[cfg(feature = "integration_tests")]
    mod query_cells_integration {
        use super::*;
        use crate::schema::cells;
        use crate::utils::test_db::get_test_connection;
        use chrono::TimeZone;

        fn sample_cell_with_location(
            mcc_val: u16,
            net_val: u16,
            area_val: u32,
            cell_val: u64,
            radio_val: Radio,
            lat_val: f32,
            lon_val: f32,
        ) -> Cell {
            Cell {
                radio: radio_val,
                mcc: mcc_val,
                net: net_val,
                area: area_val,
                cell: cell_val,
                unit: Some(1),
                lon: lon_val,
                lat: lat_val,
                cell_range: 1000,
                samples: 50,
                changeable: true,
                created: chrono::Utc
                    .with_ymd_and_hms(2024, 1, 15, 10, 30, 0)
                    .unwrap()
                    .naive_utc(),
                updated: chrono::Utc
                    .with_ymd_and_hms(2025, 12, 20, 14, 0, 0)
                    .unwrap()
                    .naive_utc(),
                average_signal: Some(-85),
            }
        }

        #[test]
        fn test_query_cells_returns_all_cells_when_no_filters() {
            let (_container, mut conn) = get_test_connection();

            // Insert test cells
            for i in 1..=5 {
                let cell = sample_cell_with_location(262, 1, 100, i, Radio::Lte, 52.0, 13.0);
                diesel::insert_into(cells::table)
                    .values(&cell)
                    .execute(&mut conn)
                    .unwrap();
            }

            let query = GetCellsQuery {
                mcc: None,
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: None,
                limit: None,
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert_eq!(result.cells.len(), 5);
            assert!(!result.has_more);
            assert!(result.next_cursor.is_none());
        }

        #[test]
        fn test_query_cells_filters_by_mcc() {
            let (_container, mut conn) = get_test_connection();

            // Insert cells with different MCCs
            let cell1 = sample_cell_with_location(262, 1, 100, 1, Radio::Lte, 52.0, 13.0);
            let cell2 = sample_cell_with_location(310, 1, 100, 2, Radio::Lte, 52.0, 13.0);
            diesel::insert_into(cells::table)
                .values(&cell1)
                .execute(&mut conn)
                .unwrap();
            diesel::insert_into(cells::table)
                .values(&cell2)
                .execute(&mut conn)
                .unwrap();

            let query = GetCellsQuery {
                mcc: Some(262),
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: None,
                limit: None,
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert_eq!(result.cells.len(), 1);
            assert_eq!(result.cells[0].mcc, 262);
        }

        #[test]
        fn test_query_cells_filters_by_mnc() {
            let (_container, mut conn) = get_test_connection();

            let cell1 = sample_cell_with_location(262, 1, 100, 1, Radio::Lte, 52.0, 13.0);
            let cell2 = sample_cell_with_location(262, 2, 100, 2, Radio::Lte, 52.0, 13.0);
            diesel::insert_into(cells::table)
                .values(&cell1)
                .execute(&mut conn)
                .unwrap();
            diesel::insert_into(cells::table)
                .values(&cell2)
                .execute(&mut conn)
                .unwrap();

            let query = GetCellsQuery {
                mcc: None,
                mnc: Some(2),
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: None,
                limit: None,
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert_eq!(result.cells.len(), 1);
            assert_eq!(result.cells[0].net, 2);
        }

        #[test]
        fn test_query_cells_filters_by_geofence() {
            let (_container, mut conn) = get_test_connection();

            // Berlin area
            let cell1 = sample_cell_with_location(262, 1, 100, 1, Radio::Lte, 52.52, 13.405);
            // Munich area
            let cell2 = sample_cell_with_location(262, 1, 100, 2, Radio::Lte, 48.137, 11.576);
            // Hamburg area
            let cell3 = sample_cell_with_location(262, 1, 100, 3, Radio::Lte, 53.551, 9.993);

            diesel::insert_into(cells::table)
                .values(&cell1)
                .execute(&mut conn)
                .unwrap();
            diesel::insert_into(cells::table)
                .values(&cell2)
                .execute(&mut conn)
                .unwrap();
            diesel::insert_into(cells::table)
                .values(&cell3)
                .execute(&mut conn)
                .unwrap();

            // Query for Berlin area (roughly)
            let query = GetCellsQuery {
                mcc: None,
                mnc: None,
                min_lat: Some(52.0),
                max_lat: Some(53.0),
                min_lon: Some(13.0),
                max_lon: Some(14.0),
                radio: None,
                cursor: None,
                limit: None,
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert_eq!(result.cells.len(), 1);
            assert_eq!(result.cells[0].cell, 1);
        }

        #[test]
        fn test_query_cells_pagination_with_limit() {
            let (_container, mut conn) = get_test_connection();

            // Insert 10 cells
            for i in 1..=10 {
                let cell = sample_cell_with_location(262, 1, 100, i, Radio::Lte, 52.0, 13.0);
                diesel::insert_into(cells::table)
                    .values(&cell)
                    .execute(&mut conn)
                    .unwrap();
            }

            let query = GetCellsQuery {
                mcc: None,
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: None,
                limit: Some(5),
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert_eq!(result.cells.len(), 5);
            assert!(result.has_more);
            assert!(result.next_cursor.is_some());
        }

        #[test]
        fn test_query_cells_cursor_pagination() {
            let (_container, mut conn) = get_test_connection();

            // Insert 10 cells
            for i in 1..=10 {
                let cell = sample_cell_with_location(262, 1, 100, i, Radio::Lte, 52.0, 13.0);
                diesel::insert_into(cells::table)
                    .values(&cell)
                    .execute(&mut conn)
                    .unwrap();
            }

            // First page
            let query1 = GetCellsQuery {
                mcc: None,
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: None,
                limit: Some(5),
            };

            let result1 = query_cells(&query1, &mut conn).unwrap();
            assert_eq!(result1.cells.len(), 5);
            assert!(result1.has_more);

            // Second page using cursor
            let query2 = GetCellsQuery {
                mcc: None,
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: result1.next_cursor.clone(),
                limit: Some(5),
            };

            let result2 = query_cells(&query2, &mut conn).unwrap();
            assert_eq!(result2.cells.len(), 5);
            assert!(!result2.has_more);
            assert!(result2.next_cursor.is_none());

            // Verify no overlap between pages
            let page1_ids: Vec<u64> = result1.cells.iter().map(|c| c.cell).collect();
            let page2_ids: Vec<u64> = result2.cells.iter().map(|c| c.cell).collect();
            for id in &page2_ids {
                assert!(!page1_ids.contains(id));
            }
        }

        #[test]
        fn test_query_cells_filters_by_radio() {
            let (_container, mut conn) = get_test_connection();

            let cell1 = sample_cell_with_location(262, 1, 100, 1, Radio::Lte, 52.0, 13.0);
            let cell2 = sample_cell_with_location(262, 1, 100, 2, Radio::Gsm, 52.0, 13.0);
            diesel::insert_into(cells::table)
                .values(&cell1)
                .execute(&mut conn)
                .unwrap();
            diesel::insert_into(cells::table)
                .values(&cell2)
                .execute(&mut conn)
                .unwrap();

            let query = GetCellsQuery {
                mcc: None,
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: Some(Radio::Gsm),
                cursor: None,
                limit: None,
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert_eq!(result.cells.len(), 1);
            assert!(matches!(result.cells[0].radio, Radio::Gsm));
        }

        #[test]
        fn test_query_cells_combined_filters() {
            let (_container, mut conn) = get_test_connection();

            // Insert various cells
            let cells_to_insert = vec![
                sample_cell_with_location(262, 1, 100, 1, Radio::Lte, 52.52, 13.405), // Berlin, DE, LTE
                sample_cell_with_location(262, 2, 100, 2, Radio::Lte, 52.52, 13.405), // Berlin, DE, different MNC
                sample_cell_with_location(310, 1, 100, 3, Radio::Lte, 52.52, 13.405), // Different MCC
                sample_cell_with_location(262, 1, 100, 4, Radio::Lte, 48.137, 11.576), // Munich
                sample_cell_with_location(262, 1, 100, 5, Radio::Gsm, 52.52, 13.405), // Berlin, GSM
            ];

            for cell in cells_to_insert {
                diesel::insert_into(cells::table)
                    .values(&cell)
                    .execute(&mut conn)
                    .unwrap();
            }

            // Query for LTE cells in Berlin with MCC 262 and MNC 1
            let query = GetCellsQuery {
                mcc: Some(262),
                mnc: Some(1),
                min_lat: Some(52.0),
                max_lat: Some(53.0),
                min_lon: Some(13.0),
                max_lon: Some(14.0),
                radio: Some(Radio::Lte),
                cursor: None,
                limit: None,
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert_eq!(result.cells.len(), 1);
            assert_eq!(result.cells[0].cell, 1);
        }

        #[test]
        fn test_query_cells_respects_max_limit() {
            let (_container, mut conn) = get_test_connection();

            // Insert more than MAX_PAGE_SIZE cells
            for i in 1..=1005 {
                let cell = sample_cell_with_location(262, 1, 100, i, Radio::Lte, 52.0, 13.0);
                diesel::insert_into(cells::table)
                    .values(&cell)
                    .execute(&mut conn)
                    .unwrap();
            }

            // Request more than max
            let query = GetCellsQuery {
                mcc: None,
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: None,
                limit: Some(2000), // Exceeds MAX_PAGE_SIZE
            };

            let result = query_cells(&query, &mut conn).unwrap();

            // Should be capped at MAX_PAGE_SIZE (1000)
            assert_eq!(result.cells.len(), 1000);
            assert!(result.has_more);
        }

        #[test]
        fn test_query_cells_empty_result() {
            let (_container, mut conn) = get_test_connection();

            let query = GetCellsQuery {
                mcc: Some(999),
                mnc: None,
                min_lat: None,
                max_lat: None,
                min_lon: None,
                max_lon: None,
                radio: None,
                cursor: None,
                limit: None,
            };

            let result = query_cells(&query, &mut conn).unwrap();

            assert!(result.cells.is_empty());
            assert!(!result.has_more);
            assert!(result.next_cursor.is_none());
        }
    }
}
