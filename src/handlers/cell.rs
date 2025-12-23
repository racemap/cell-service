use serde::{Deserialize, Serialize};

use crate::{models::*, utils::db::establish_connection};
use diesel::prelude::*;
use diesel::MysqlConnection;
use tracing::instrument;

#[derive(Deserialize, Serialize, Debug)]
pub struct GetCellQuery {
    pub mcc: u16,
    pub net: u16,
    pub area: u32,
    pub cell: u64,
    pub radio: Option<Radio>,
}

/// Queries a cell from the database. Extracted for testability.
#[instrument(skip(connection))]
pub fn query_cell(
    query: &GetCellQuery,
    connection: &mut MysqlConnection,
) -> Result<Option<Cell>, diesel::result::Error> {
    use crate::schema::cells::dsl::*;

    let mut db_query = cells.into_boxed();

    db_query = db_query
        .filter(mcc.eq(&query.mcc))
        .filter(net.eq(&query.net))
        .filter(area.eq(&query.area))
        .filter(cell.eq(&query.cell));

    if let Some(ref search_radio) = query.radio {
        db_query = db_query.filter(radio.eq(search_radio));
    }

    match db_query.first(connection) {
        Ok(entry) => Ok(Some(entry)),
        Err(diesel::result::Error::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

#[instrument]
pub async fn handle_get_cell(query: GetCellQuery) -> Result<impl warp::Reply, warp::Rejection> {
    let connection = &mut establish_connection();

    match query_cell(&query, connection) {
        Ok(Some(entry)) => Ok(warp::reply::json(&entry)),
        Ok(None) => Ok(warp::reply::json(&serde_json::Value::Null)),
        Err(_) => Ok(warp::reply::json(&serde_json::Value::Null)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod get_cell_query {
        use super::*;

        #[test]
        fn test_deserialize_all_fields() {
            let json = r#"{
                "mcc": 262,
                "net": 1,
                "area": 12345,
                "cell": 67890,
                "radio": "LTE"
            }"#;

            let query: GetCellQuery = serde_json::from_str(json).unwrap();

            assert_eq!(query.mcc, 262);
            assert_eq!(query.net, 1);
            assert_eq!(query.area, 12345);
            assert_eq!(query.cell, 67890);
            assert!(matches!(query.radio, Some(Radio::Lte)));
        }

        #[test]
        fn test_deserialize_without_optional_radio() {
            let json = r#"{
                "mcc": 262,
                "net": 1,
                "area": 100,
                "cell": 200
            }"#;

            let query: GetCellQuery = serde_json::from_str(json).unwrap();

            assert_eq!(query.mcc, 262);
            assert!(query.radio.is_none());
        }

        #[test]
        fn test_deserialize_with_null_radio() {
            let json = r#"{
                "mcc": 262,
                "net": 1,
                "area": 100,
                "cell": 200,
                "radio": null
            }"#;

            let query: GetCellQuery = serde_json::from_str(json).unwrap();

            assert!(query.radio.is_none());
        }

        #[test]
        fn test_deserialize_from_query_string() {
            let query_string = "mcc=262&net=1&area=12345&cell=67890&radio=GSM";

            let query: GetCellQuery = serde_urlencoded::from_str(query_string).unwrap();

            assert_eq!(query.mcc, 262);
            assert_eq!(query.net, 1);
            assert_eq!(query.area, 12345);
            assert_eq!(query.cell, 67890);
            assert!(matches!(query.radio, Some(Radio::Gsm)));
        }

        #[test]
        fn test_deserialize_from_query_string_without_radio() {
            let query_string = "mcc=310&net=410&area=1000&cell=999";

            let query: GetCellQuery = serde_urlencoded::from_str(query_string).unwrap();

            assert_eq!(query.mcc, 310);
            assert_eq!(query.net, 410);
            assert_eq!(query.area, 1000);
            assert_eq!(query.cell, 999);
            assert!(query.radio.is_none());
        }

        #[test]
        fn test_deserialize_fails_without_required_fields() {
            let json = r#"{
                "mcc": 262,
                "net": 1
            }"#;

            let result: Result<GetCellQuery, _> = serde_json::from_str(json);

            assert!(result.is_err());
        }

        #[test]
        fn test_serialize_roundtrip() {
            let query = GetCellQuery {
                mcc: 262,
                net: 1,
                area: 100,
                cell: 200,
                radio: Some(Radio::Umts),
            };

            let json = serde_json::to_string(&query).unwrap();
            let deserialized: GetCellQuery = serde_json::from_str(&json).unwrap();

            assert_eq!(deserialized.mcc, query.mcc);
            assert_eq!(deserialized.cell, query.cell);
            assert!(matches!(deserialized.radio, Some(Radio::Umts)));
        }
    }

    /// Integration tests for query_cell using testcontainers.
    /// These tests automatically spin up a MariaDB container.
    ///
    /// Prerequisites:
    /// - Docker must be running
    /// - Run `docker pull mariadb:11.4` to pre-pull the image (optional but faster)
    ///
    /// Run with: cargo test --features integration_tests query_cell_integration
    #[cfg(feature = "integration_tests")]
    mod query_cell_integration {
        use super::*;
        use crate::schema::cells;
        use crate::utils::test_db::get_test_connection;
        use chrono::TimeZone;

        fn sample_cell(
            mcc_val: u16,
            net_val: u16,
            area_val: u32,
            cell_val: u64,
            radio_val: Radio,
        ) -> Cell {
            Cell {
                radio: radio_val,
                mcc: mcc_val,
                net: net_val,
                area: area_val,
                cell: cell_val,
                unit: Some(1),
                lon: 13.405,
                lat: 52.52,
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
        fn test_query_cell_returns_matching_cell() {
            let (_container, mut conn) = get_test_connection();

            // Insert test data
            let test_cell = sample_cell(262, 1, 12345, 67890, Radio::Lte);
            diesel::insert_into(cells::table)
                .values(&test_cell)
                .execute(&mut conn)
                .unwrap();

            // Query
            let query = GetCellQuery {
                mcc: 262,
                net: 1,
                area: 12345,
                cell: 67890,
                radio: None,
            };
            let result = query_cell(&query, &mut conn).unwrap();

            // Assert
            assert!(result.is_some());
            let cell = result.unwrap();
            assert_eq!(cell.mcc, 262);
            assert_eq!(cell.cell, 67890);
            assert!(matches!(cell.radio, Radio::Lte));
        }

        #[test]
        fn test_query_cell_returns_none_when_not_found() {
            let (_container, mut conn) = get_test_connection();

            let query = GetCellQuery {
                mcc: 999,
                net: 999,
                area: 999,
                cell: 999,
                radio: None,
            };
            let result = query_cell(&query, &mut conn).unwrap();

            assert!(result.is_none());
        }

        #[test]
        fn test_query_cell_filters_by_radio_type() {
            let (_container, mut conn) = get_test_connection();

            // Insert two cells with same identifiers but different radio types
            let lte_cell = sample_cell(262, 1, 100, 200, Radio::Lte);
            let gsm_cell = sample_cell(262, 1, 100, 201, Radio::Gsm);
            diesel::insert_into(cells::table)
                .values(&lte_cell)
                .execute(&mut conn)
                .unwrap();
            diesel::insert_into(cells::table)
                .values(&gsm_cell)
                .execute(&mut conn)
                .unwrap();

            // Query for LTE specifically
            let query = GetCellQuery {
                mcc: 262,
                net: 1,
                area: 100,
                cell: 200,
                radio: Some(Radio::Lte),
            };
            let result = query_cell(&query, &mut conn).unwrap();

            assert!(result.is_some());
            assert!(matches!(result.unwrap().radio, Radio::Lte));

            // Query for GSM - should not find the LTE cell
            let query_gsm = GetCellQuery {
                mcc: 262,
                net: 1,
                area: 100,
                cell: 200,
                radio: Some(Radio::Gsm),
            };
            let result_gsm = query_cell(&query_gsm, &mut conn).unwrap();
            assert!(result_gsm.is_none());
        }

        #[test]
        fn test_query_cell_matches_all_filter_fields() {
            let (_container, mut conn) = get_test_connection();

            let test_cell = sample_cell(310, 410, 5000, 6000, Radio::Umts);
            diesel::insert_into(cells::table)
                .values(&test_cell)
                .execute(&mut conn)
                .unwrap();

            // Wrong mcc
            let query = GetCellQuery {
                mcc: 999,
                net: 410,
                area: 5000,
                cell: 6000,
                radio: None,
            };
            assert!(query_cell(&query, &mut conn).unwrap().is_none());

            // Wrong net
            let query = GetCellQuery {
                mcc: 310,
                net: 999,
                area: 5000,
                cell: 6000,
                radio: None,
            };
            assert!(query_cell(&query, &mut conn).unwrap().is_none());

            // Wrong area
            let query = GetCellQuery {
                mcc: 310,
                net: 410,
                area: 9999,
                cell: 6000,
                radio: None,
            };
            assert!(query_cell(&query, &mut conn).unwrap().is_none());

            // Wrong cell
            let query = GetCellQuery {
                mcc: 310,
                net: 410,
                area: 5000,
                cell: 9999,
                radio: None,
            };
            assert!(query_cell(&query, &mut conn).unwrap().is_none());

            // All correct
            let query = GetCellQuery {
                mcc: 310,
                net: 410,
                area: 5000,
                cell: 6000,
                radio: None,
            };
            assert!(query_cell(&query, &mut conn).unwrap().is_some());
        }
    }
}
