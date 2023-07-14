pub mod models;
pub mod schema;

use ::chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::MysqlConnection;
use dotenvy::dotenv;
use serde_with::formats::Flexible;
use serde_with::BoolFromInt;
use serde_with::TimestampSeconds;
use std::{env, error::Error, ffi::OsString, process};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Radio {
    Gsm,
    Umts,
    Lte,
}

#[serde_with::serde_as]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    radio: Radio,
    mcc: u16,
    net: u16,
    area: u16,
    cell: u32,
    unit: Option<u16>,
    lon: f32,
    lat: f32,
    #[serde(alias = "range")]
    cell_range: u32,
    samples: u32,
    #[serde_as(as = "BoolFromInt")]
    changeable: bool,
    #[serde_as(as = "TimestampSeconds<u32, Flexible>")]
    created: DateTime<Utc>,
    #[serde_as(as = "TimestampSeconds<u32, Flexible>")]
    updated: DateTime<Utc>,
    average_signal: Option<i16>,
}

pub fn establish_connection() -> MysqlConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MysqlConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

fn run() -> Result<(), Box<dyn Error>> {
    let file_path = get_first_arg()?;

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path)?;

    for result in rdr.deserialize() {
        let record: Record = result?;
        println!("{:?}", record);
    }
    Ok(())
}

fn get_first_arg() -> Result<OsString, Box<dyn Error>> {
    match env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}

fn main() {
    if let Err(err) = run() {
        println!("{}", err);
        process::exit(1);
    }
}
