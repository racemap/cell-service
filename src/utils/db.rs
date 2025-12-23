use std::io::Error;

use crate::models::{LastUpdates, LastUpdatesType};
use crate::schema::last_updates;
use crate::schema::last_updates::dsl::*;
use crate::utils::config::Config;
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::result::Error::NotFound;
use diesel::{Connection, MysqlConnection, RunQueryDsl};

pub fn establish_connection(config: Config) -> MysqlConnection {
    let database_url = config.db_url;
    MysqlConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn set_last_update(
    target_type: LastUpdatesType,
    date: chrono::NaiveDateTime,
    config: Config,
) -> Result<(), Error> {
    let connection = &mut establish_connection(config);
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

pub fn get_last_update(config: Config) -> Result<NaiveDateTime, diesel::result::Error> {
    let connection = &mut establish_connection(config);
    let last_update: Result<LastUpdates, diesel::result::Error> =
        last_updates.order(value.desc()).first(connection);

    match last_update {
        Ok(last_update) => Ok(last_update.value),
        Err(NotFound) => Ok(DateTime::<Utc>::from_timestamp_micros(0)
            .unwrap()
            .naive_utc()),
        Err(e) => Err(e),
    }
}

/// Establishes a test database connection using DATABASE_URL_TEST env var.
/// Falls back to DATABASE_URL if DATABASE_URL_TEST is not set.
#[cfg(test)]
pub fn establish_test_connection() -> MysqlConnection {
    use std::env;

    let database_url = env::var("DATABASE_URL_TEST")
        .or_else(|_| env::var("DATABASE_URL"))
        .expect("DATABASE_URL_TEST or DATABASE_URL must be set for tests");
    MysqlConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to test database {}", database_url))
}
