pub mod models;
pub mod schema;
pub mod utils;

use std::sync::Arc;

use dotenvy::dotenv;
use tokio::{
    signal::{
        ctrl_c,
        unix::{signal, SignalKind},
    },
    sync::{
        oneshot::{self, Sender},
        Mutex,
    },
};
use tracing::info;
use tracing_subscriber::{
    layer::SubscriberExt,
    {self, EnvFilter},
};
use utils::{
    data::update_loop,
    server::start_server,
    utils::{flatten, FutureError},
};

async fn process_handling(
    halt: &Arc<Mutex<bool>>,
    shutdown_sender: Sender<()>,
) -> Result<(), FutureError> {
    loop {
        if *halt.lock().await {
            shutdown_sender.send(()).unwrap();
            return Ok(());
        }
        let mut sigterm = signal(SignalKind::terminate()).unwrap();
        let mut sigint = signal(SignalKind::interrupt()).unwrap();

        tokio::select! {
            _ = ctrl_c() => {
                info!("Ctrl-C received. Shutting down...");
                let mut lock = halt.lock().await;
                *lock = true;
            }
            _ = sigterm.recv() => {
                info!("Hangup received. Shutting down...");
                let mut lock = halt.lock().await;
                *lock = true;
            }
            _ = sigint.recv() => {
                info!("Interrupt received. Shutting down...");
                let mut lock = halt.lock().await;
                *lock = true;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let subscriber = tracing_subscriber::registry()
        .with(tracing_logfmt::layer())
        .with(EnvFilter::from_default_env());
    tracing::subscriber::set_global_default(subscriber)
        .expect("Global logger has already been set!");

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
            info!("Failed with {}.", err);
        }
    }
}
