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
