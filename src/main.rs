pub mod models;
pub mod schema;

use crate::models::Cell;
use crate::schema::cells;
use diesel::prelude::*;
use diesel::MysqlConnection;
use dotenvy::dotenv;
use std::{env, error::Error, ffi::OsString, process};

pub fn establish_connection() -> MysqlConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MysqlConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

fn run() -> Result<(), Box<dyn Error>> {
    let file_path = get_first_arg()?;
    let connection = &mut establish_connection();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path)?;

    for result in rdr.deserialize() {
        let record: Cell = result?;
        println!("{:?}", record);

        // diesel::insert_into(cells::table)
        //     .values(&record)
        //     .execute(connection)
        //     .expect("Error inserting cell");

        // TODO: insert data directly
        // LOAD Data INFILE '/Users/karlwolffgang/racemap-cells/data/MLS-full-cell-export-2023-07-15T000000.csv'
        // REPLACE INTO TABLE cells
        // FIELDS TERMINATED BY ','
        // LINES TERMINATED BY '\r\n'
        // IGNORE 1 LINES
        // (radio, mcc, net, area, cell, @unit, lon, lat, cell_range, samples, changeable, @created, @updated, @average_signal)
        // SET
        // unit = NULLIF(@unit, ''),
        // average_signal = NULLIF(@average_signal, ''),
        // created = FROM_UNIXTIME(@created),
        // updated = FROM_UNIXTIME(@updated);
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
