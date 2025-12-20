use serde::{Deserialize, Serialize};
use tracing::info;
use warp::Filter;

use tokio::sync::oneshot::Receiver;

use crate::{models::*, utils::db::establish_connection};
use diesel::prelude::*;

use super::utils::Promise;

#[derive(Deserialize, Serialize)]
struct GetCellQuery {
    mcc: u16,
    net: u16,
    area: u32,
    cell: u64,
    radio: Option<Radio>,
}

async fn get_cell(query: GetCellQuery) -> Result<impl warp::Reply, warp::Rejection> {
    use crate::schema::cells::dsl::*;

    let connection = &mut establish_connection();
    let mut db_query = cells.into_boxed();
    let search_radio = query.radio;

    db_query = db_query
        .filter(mcc.eq(&query.mcc))
        .filter(net.eq(&query.net))
        .filter(area.eq(&query.area))
        .filter(cell.eq(&query.cell));

    if search_radio.is_some() {
        db_query = db_query.filter(radio.eq(search_radio.unwrap()));
    }

    let result: Result<Cell, _> = db_query.first(connection);

    match result {
        Ok(entry) => Ok(warp::reply::json(&entry)),
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
}
