use serde::{Deserialize, Serialize};
use tracing::info;
use warp::Filter;

use tokio::sync::oneshot::Receiver;

use crate::{models::*, utils::db::establish_connection};
use diesel::prelude::*;
use diesel::MysqlConnection;

use super::utils::Promise;

#[derive(Deserialize, Serialize)]
pub struct GetCellQuery {
    pub mcc: u16,
    pub net: u16,
    pub area: u32,
    pub cell: u64,
    pub radio: Option<Radio>,
}

/// Queries a cell from the database. Extracted for testability.
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

async fn get_cell(query: GetCellQuery) -> Result<impl warp::Reply, warp::Rejection> {
    let connection = &mut establish_connection();

    match query_cell(&query, connection) {
        Ok(Some(entry)) => Ok(warp::reply::json(&entry)),
        Ok(None) => Ok(warp::reply::json(&serde_json::Value::Null)),
        Err(_) => Ok(warp::reply::json(&serde_json::Value::Null)),
    }
}

/// Returns the health check route filter.
pub fn health_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("health").map(|| "OK")
}

pub async fn start_server(shutdown_receiver: Receiver<()>) -> Promise<()> {
    info!("Start server.");

    let get_cell = warp::path!("cell")
        .and(warp::query::<GetCellQuery>())
        .and_then(|query| async move { get_cell(query).await });
    let routes = warp::get().and(health_route().or(get_cell));

    let (_, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 3000), async {
            shutdown_receiver.await.ok();
        });

    server.await;
    info!("Server stopped.");

    Ok(())
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

    mod health_endpoint {
        use super::*;
        use warp::http::StatusCode;
        use warp::test::request;

        #[tokio::test]
        async fn test_health_returns_ok() {
            let response = request()
                .method("GET")
                .path("/health")
                .reply(&health_route())
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.body(), "OK");
        }

        #[tokio::test]
        async fn test_health_returns_404_for_wrong_path() {
            let response = request()
                .method("GET")
                .path("/healthz")
                .reply(&health_route())
                .await;

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
        use chrono::TimeZone;
        use diesel::Connection;
        use diesel::MysqlConnection;
        use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
        use std::sync::OnceLock;
        use testcontainers::core::ImageExt;
        use testcontainers::runners::SyncRunner;
        use testcontainers::Container;
        use testcontainers_modules::mariadb::Mariadb;

        const MARIADB_VERSION: &str = "11.4";

        pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

        // Shared container across all tests - initialized once
        static TEST_DB: OnceLock<(Container<Mariadb>, String)> = OnceLock::new();

        /// Initialize the shared test database container once.
        /// Returns the database URL for creating connections.
        fn init_test_db() -> &'static str {
            let (_, url) = TEST_DB.get_or_init(|| {
                let container = Mariadb::default()
                    .with_tag(MARIADB_VERSION)
                    .start()
                    .expect("Failed to start MariaDB container. Is Docker running?");

                let host_port = container
                    .get_host_port_ipv4(3306)
                    .expect("Failed to get MySQL port");

                let database_url = format!("mysql://root@127.0.0.1:{}/test", host_port);

                // Run migrations once
                let mut conn = MysqlConnection::establish(&database_url)
                    .expect("Failed to connect to test database");
                conn.run_pending_migrations(MIGRATIONS)
                    .expect("Failed to run migrations");

                (container, database_url)
            });
            url
        }

        /// Get a connection with an open transaction that will be rolled back.
        /// This provides test isolation without needing to truncate tables.
        fn get_test_connection() -> MysqlConnection {
            let url = init_test_db();
            let mut conn =
                MysqlConnection::establish(url).expect("Failed to connect to test database");

            // Start a test transaction - automatically rolled back when connection drops
            conn.begin_test_transaction()
                .expect("Failed to begin test transaction");

            conn
        }

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
            let mut conn = get_test_connection();

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
            let mut conn = get_test_connection();

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
            let mut conn = get_test_connection();

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
            let mut conn = get_test_connection();

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