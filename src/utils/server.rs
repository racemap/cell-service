use tracing::info;
use warp::Filter;

use tokio::sync::oneshot::Receiver;

use crate::{handlers, utils::config::Config};

use super::utils::Promise;

/// Returns the health check route filter.
pub fn health_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("health").map(|| "OK")
}

pub async fn start_server(shutdown_receiver: Receiver<()>, config: Config) -> Promise<()> {
    info!("Start server.");

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

    let routes = warp::get().and(health_route().or(get_cell).or(get_cells));

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
