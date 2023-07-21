use serde::{Deserialize, Serialize};
use tracing::info;
use warp::{http::Error, Filter};

use tokio::sync::oneshot::Receiver;

use crate::{models::*, utils::db::establish_connection};
use diesel::prelude::*;

use super::utils::Promise;

#[derive(Deserialize, Serialize)]
struct GetCellQuery {
    mcc: u16,
    net: u16,
    area: u16,
    cell: u32,
}

async fn get_cell(query: GetCellQuery) -> Result<impl warp::Reply, warp::Rejection> {
    use crate::schema::cells::dsl::*;

    let connection = &mut establish_connection();
    let result: Cell = cells
        .filter(mcc.eq(&query.mcc))
        .filter(net.eq(&query.net))
        .filter(area.eq(&query.area))
        .filter(cell.eq(&query.cell))
        .first(connection)
        .expect("Error loading cell");

    Ok(warp::reply::json(&result))
}

pub async fn start_server(shutdown_receiver: Receiver<()>) -> Promise<()> {
    info!("Start server.");

    let health = warp::path!("health").map(|| "OK");
    let get_cell = warp::path!("cell")
        .and(warp::query::<GetCellQuery>())
        .and_then(|query| async move { get_cell(query).await });
    let routes = warp::get().and(health.or(get_cell));

    let (_, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 3000), async {
            shutdown_receiver.await.ok();
        });

    server.await;
    info!("Server stopped.");

    Ok(())
}
