pub mod models;
pub mod schema;
pub mod utils;

use utils::utils::get_first_arg;

use crate::utils::data::load_data;
use std::{error::Error, process};

fn run() -> Result<(), Box<dyn Error>> {
    let input_path = String::from(get_first_arg()?.to_str().unwrap());
    load_data(input_path)
}

fn main() {
    if let Err(err) = run() {
        println!("{}", err);
        process::exit(1);
    }
}
