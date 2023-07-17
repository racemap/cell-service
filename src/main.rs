pub mod models;
pub mod schema;
pub mod utils;

use std::cell::Cell;

use tokio::signal::ctrl_c;
use utils::data::update_loop;

async fn process_handling(halt: &Cell<bool>) {
    while !halt.get() {
        tokio::select! {
            _ = ctrl_c() => {
                println!("Ctrl-C received. Shutting down...");
                halt.set(true);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let halt = Cell::new(false);

    tokio::join!(update_loop(&halt), process_handling(&halt));
}
