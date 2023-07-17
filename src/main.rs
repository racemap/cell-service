pub mod models;
pub mod schema;
pub mod utils;

use std::sync::Arc;

use tokio::signal::ctrl_c;
use tokio::sync::Mutex;
use utils::data::update_loop;
use utils::utils::{flatten, FutureError};

async fn process_handling(halt: &Arc<Mutex<bool>>) -> Result<(), FutureError> {
    loop {
        if *halt.lock().await {
            return Ok(());
        }

        tokio::select! {
            _ = ctrl_c() => {
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

    let process = tokio::spawn(process_handling(&HALT));
    let update = tokio::spawn(update_loop(&HALT));

    match tokio::try_join!(flatten(update), flatten(process)) {
        Ok(_) => {}
        Err(err) => {
            println!("Failed with {}.", err);
        }
    }
}
