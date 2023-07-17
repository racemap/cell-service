pub mod models;
pub mod schema;
pub mod utils;

use utils::{
    data::{load_last_diff, load_last_full},
    utils::{FutureError, Promise},
};

use std::{error::Error, process};

async fn run() -> Promise<()> {
    // let input_path = String::from(get_first_arg()?.to_str().unwrap());
    // load_data(input_path)
    load_last_diff().await
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => (),
        a => match a {
            Err(e) => println!("Error: {:?}", e),
            _ => (),
        },
    }
}
