pub mod models;
pub mod schema;
pub mod utils;

use std::sync::Arc;

use tokio::signal::ctrl_c;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Sender;
use tokio::sync::Mutex;
use utils::data::update_loop;
use utils::server::start_server;
use utils::utils::{flatten, FutureError};

async fn process_handling(
    halt: &Arc<Mutex<bool>>,
    shutdown_sender: Sender<()>,
) -> Result<(), FutureError> {
    loop {
        if *halt.lock().await {
            shutdown_sender.send(()).unwrap();
            return Ok(());
        }

        tokio::select! {
            _ = ctrl_c() => {
                println!();
                println!("Ctrl-C received. Shutting down...");
                let mut lock = halt.lock().await;
                *lock = true;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    lazy_static::lazy_static! {
        static ref HALT: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    }
    let (tx, rx) = oneshot::channel();

    let process = tokio::spawn(process_handling(&HALT, tx));
    let update = tokio::spawn(update_loop(&HALT));
    let server = tokio::spawn(start_server(rx));

    match tokio::try_join!(flatten(update), flatten(process), flatten(server)) {
        Ok(_) => {}
        Err(err) => {
            println!("Failed with {}.", err);
        }
    }
}
