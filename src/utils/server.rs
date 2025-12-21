use tracing::info;
use warp::Filter;

use tokio::sync::oneshot::Receiver;

use crate::handlers;

use super::utils::Promise;

/// Returns the health check route filter.
pub fn health_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("health").map(|| "OK")
}

pub async fn start_server(shutdown_receiver: Receiver<()>) -> Promise<()> {
    info!("Start server.");

    let get_cell = warp::path!("cell")
        .and(warp::query::<handlers::cell::GetCellQuery>())
        .and_then(|query| async move { handlers::cell::handle_get_cell(query).await });

    let get_cells = warp::path!("cells")
        .and(warp::query::<handlers::cells::GetCellsQuery>())
        .and_then(|query| async move { handlers::cells::handle_get_cells(query).await });

    let lookup_cells = warp::path!("cells" / "lookup")
        .and(warp::post())
        .and(warp::body::json::<handlers::lookup::LookupCellsRequest>())
        .and_then(|req| async move { handlers::lookup::handle_lookup_cells(req).await });

    let get_routes = warp::get().and(health_route().or(get_cell).or(get_cells));
    let routes = get_routes.or(lookup_cells);

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
