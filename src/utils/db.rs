use std::env;
use std::io::Error;

use crate::models::{LastUpdates, LastUpdatesType};
use crate::schema::last_updates;
use crate::schema::last_updates::dsl::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::result::Error::NotFound;
use diesel::{Connection, MysqlConnection, RunQueryDsl};

pub fn establish_connection() -> MysqlConnection {
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

pub fn get_last_update() -> Result<NaiveDateTime, diesel::result::Error> {
    let connection = &mut establish_connection();
    let last_update: Result<LastUpdates, diesel::result::Error> =
        last_updates.order(value.desc()).first(connection);

    match last_update {
        Ok(last_update) => Ok(last_update.value),
        Err(NotFound) => Ok(NaiveDateTime::from_timestamp_micros(0).unwrap()),
        Err(e) => Err(e),
    }
}
