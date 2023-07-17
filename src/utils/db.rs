use std::env;
use std::io::Error;

use crate::models::{LastUpdates, LastUpdatesType};
use crate::schema::last_updates;
use crate::schema::last_updates::dsl::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::{Connection, MysqlConnection, RunQueryDsl};
use dotenvy::dotenv;

pub fn establish_connection() -> MysqlConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MysqlConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn set_last_update(
    target_type: LastUpdatesType,
    date: chrono::NaiveDateTime,
) -> Result<(), Error> {
    let connection = &mut establish_connection();
    let new_last_update = LastUpdates {
        update_type: target_type,
        value: date,
    };

    let insert_count = diesel::replace_into(last_updates::table)
        .values(&new_last_update)
        .execute(connection)
        .unwrap();

    if insert_count < 1 {
        panic!("Error inserting last update");
    }

    Ok(())
}

pub fn get_last_update(target_type: LastUpdatesType) -> Result<NaiveDateTime, Error> {
    let connection = &mut establish_connection();
    let last_update: LastUpdates = last_updates
        .filter(last_updates::update_type.eq(target_type))
        .first(connection)
        .unwrap();

    Ok(last_update.value)
}
