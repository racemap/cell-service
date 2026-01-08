use tracing::{debug, info};
use warp::{cors::Cors, Filter};

use tokio::sync::oneshot::Receiver;

use crate::{handlers, utils::config::Config};

use super::utils::Promise;

/// Returns the health check route filter.
pub fn health_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("health").map(|| "OK")
}

/// Creates a CORS filter based on the configured origins.
/// If CORS_ORIGINS is set, only those origins are allowed.
/// If CORS_ORIGINS is not set or empty, all origins are allowed.
pub fn cors_filter(cors_origins: Vec<String>) -> Cors {
    let cors = warp::cors().allow_methods(vec!["GET", "OPTIONS"]);

    if cors_origins.is_empty() {
        debug!("CORS configured to allow any origin");
        cors.allow_any_origin().build()
    } else {
        debug!("CORS configured with origins: {:?}", cors_origins);
        cors.allow_origins(
            cors_origins
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<&str>>(),
        )
        .build()
    }
}

pub async fn start_server(shutdown_receiver: Receiver<()>, config: Config) -> Promise<()> {
    let port = config.port;
    let bind = config.bind;
    let cors_origins = config.cors_origins.clone();

    info!("Start server.");
    debug!("Port: {}", port);
    debug!("Bind Address: {:?}", bind);

    let config_filter = warp::any().map(move || config.clone());

    let get_cell = warp::path!("cell")
        .and(warp::query::<handlers::cell::GetCellQuery>())
        .and(config_filter.clone())
        .and_then(
            |query, config| async move { handlers::cell::handle_get_cell(query, config).await },
        );

    let get_cells = warp::path!("cells")
        .and(warp::query::<handlers::cells::GetCellsQuery>())
        .and(config_filter.clone())
        .and_then(
            |query, config| async move { handlers::cells::handle_get_cells(query, config).await },
        );

    let cors = cors_filter(cors_origins);
    let routes = warp::get()
        .and(health_route().or(get_cell).or(get_cells))
        .with(cors);

    let (_, server) = warp::serve(routes).bind_with_graceful_shutdown((bind, port), async {
        shutdown_receiver.await.ok();
    });

    server.await;
    info!("Server stopped.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    mod cors_filter_tests {
        use super::*;
        use warp::http::StatusCode;
        use warp::test::request;

        #[tokio::test]
        async fn test_empty_origins_allows_any_origin() {
            let cors = cors_filter(vec![]);
            let route = warp::get().and(warp::path!("test")).map(|| "OK").with(cors);

            let response = request()
                .method("GET")
                .path("/test")
                .header("Origin", "https://example.com")
                .reply(&route)
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("access-control-allow-origin"),
                Some(&"https://example.com".parse().unwrap())
            );
        }

        #[tokio::test]
        async fn test_empty_origins_allows_different_origins() {
            let cors = cors_filter(vec![]);
            let route = warp::get().and(warp::path!("test")).map(|| "OK").with(cors);

            let response = request()
                .method("GET")
                .path("/test")
                .header("Origin", "https://another-domain.org")
                .reply(&route)
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("access-control-allow-origin"),
                Some(&"https://another-domain.org".parse().unwrap())
            );
        }

        #[tokio::test]
        async fn test_specific_origins_allows_matching_origin() {
            let cors = cors_filter(vec!["https://allowed.com".to_string()]);
            let route = warp::get().and(warp::path!("test")).map(|| "OK").with(cors);

            let response = request()
                .method("GET")
                .path("/test")
                .header("Origin", "https://allowed.com")
                .reply(&route)
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("access-control-allow-origin"),
                Some(&"https://allowed.com".parse().unwrap())
            );
        }

        #[tokio::test]
        async fn test_specific_origins_rejects_non_matching_origin() {
            let cors = cors_filter(vec!["https://allowed.com".to_string()]);
            let route = warp::get().and(warp::path!("test")).map(|| "OK").with(cors);

            let response = request()
                .method("GET")
                .path("/test")
                .header("Origin", "https://notallowed.com")
                .reply(&route)
                .await;

            // The request still succeeds but without CORS header for non-matching origin
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }

        #[tokio::test]
        async fn test_multiple_origins_allows_all_specified() {
            let cors = cors_filter(vec![
                "https://first.com".to_string(),
                "https://second.com".to_string(),
            ]);
            let route = warp::get().and(warp::path!("test")).map(|| "OK").with(cors);

            // Test first origin
            let response = request()
                .method("GET")
                .path("/test")
                .header("Origin", "https://first.com")
                .reply(&route)
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("access-control-allow-origin"),
                Some(&"https://first.com".parse().unwrap())
            );

            // Test second origin
            let response = request()
                .method("GET")
                .path("/test")
                .header("Origin", "https://second.com")
                .reply(&route)
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("access-control-allow-origin"),
                Some(&"https://second.com".parse().unwrap())
            );
        }

        #[tokio::test]
        async fn test_cors_allows_get_method() {
            let cors = cors_filter(vec![]);
            let route = warp::get().and(warp::path!("test")).map(|| "OK").with(cors);

            let response = request()
                .method("OPTIONS")
                .path("/test")
                .header("Origin", "https://example.com")
                .header("Access-Control-Request-Method", "GET")
                .reply(&route)
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            let allow_methods = response
                .headers()
                .get("access-control-allow-methods")
                .map(|v| v.to_str().unwrap_or(""));
            assert!(allow_methods.is_some());
            assert!(allow_methods.unwrap().contains("GET"));
        }

        #[tokio::test]
        async fn test_cors_allows_options_method() {
            let cors = cors_filter(vec![]);
            let route = warp::get().and(warp::path!("test")).map(|| "OK").with(cors);

            let response = request()
                .method("OPTIONS")
                .path("/test")
                .header("Origin", "https://example.com")
                .header("Access-Control-Request-Method", "OPTIONS")
                .reply(&route)
                .await;

            assert_eq!(response.status(), StatusCode::OK);
            let allow_methods = response
                .headers()
                .get("access-control-allow-methods")
                .map(|v| v.to_str().unwrap_or(""));
            assert!(allow_methods.is_some());
            assert!(allow_methods.unwrap().contains("OPTIONS"));
        }
    }
}
