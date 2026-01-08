use tracing::{debug, info};
use warp::{cors::Cors, Filter};

use tokio::sync::oneshot::Receiver;

use crate::{handlers, utils::config::Config};

use super::utils::Promise;

/// Returns the health check route filter.
pub fn health_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("health").map(|| "OK")
}

/// Creates a CORS filter based on the configured origin.
/// If CORS_ORIGIN is set, only that origin is allowed.
/// If CORS_ORIGIN is not set, all origins are allowed.
pub fn cors_filter(cors_origin: Option<String>) -> Cors {
    let cors = warp::cors().allow_methods(vec!["GET", "OPTIONS"]);

    match cors_origin {
        Some(origin) => {
            debug!("CORS configured with origin: {}", origin);
            cors.allow_origin(origin.as_str()).build()
        }
        None => {
            debug!("CORS configured to allow any origin");
            cors.allow_any_origin().build()
        }
    }
}

pub async fn start_server(shutdown_receiver: Receiver<()>, config: Config) -> Promise<()> {
    let port = config.port;
    let bind = config.bind;
    let cors_origin = config.cors_origin.clone();

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

    let cors = cors_filter(cors_origin);
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
}
