use warp::Filter;

use tokio::sync::oneshot::Receiver;

use super::utils::Promise;

pub async fn start_server(shutdown_receiver: Receiver<()>) -> Promise<()> {
    println!("Start server.");

    let routes = warp::path!("health").map(|| "OK");
    let (_, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 3000), async {
            shutdown_receiver.await.ok();
        });

    server.await;
    println!("Server stopped.");

    Ok(())
}
