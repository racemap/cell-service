use std::cell::Cell;
use std::env;
use std::io::Error;

use crate::models::LastUpdatesType;
use async_compression::tokio::bufread::GzipDecoder;
use chrono::DateTime;
use chrono::Datelike;
use chrono::TimeZone;
use chrono::Timelike;
use chrono::Utc;
use diesel::RunQueryDsl;
use futures::stream::TryStreamExt;
use tokio::task::yield_now;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use super::db::establish_connection;
use super::db::get_last_update;
use super::db::set_last_update;
use super::utils::Promise;

fn get_url_of_full_package(date: chrono::DateTime<Utc>) -> String {
    let year = date.year();
    let month = date.month();
    let day = date.day();
    format!(
        "https://d2koia3g127518.cloudfront.net/export/MLS-full-cell-export-{:04}-{:02}-{:02}T000000.csv.gz",
        year, month, day
    )
}

fn get_url_of_diff_package(date: chrono::DateTime<Utc>) -> String {
    let year = date.year();
    let month = date.month();
    let day = date.day();
    let hour = date.hour();
    format!(
        "https://d2koia3g127518.cloudfront.net/export/MLS-diff-cell-export-{:04}-{:02}-{:02}T{:02}0000.csv.gz",
        year, month, day, hour
    )
}

async fn load_url(url: String, output: String) -> Promise<()> {
    let response = reqwest::get(url).await?;

    if response.status().as_u16() != 200 {
        return Err("Data not found".into());
    }

    let stream = response
        .bytes_stream()
        .map_err(convert_error)
        .into_async_read()
        .compat();
    let decoder = GzipDecoder::new(stream);
    let mut buf_reader = tokio::io::BufReader::new(decoder);

    let mut file_2 = tokio::fs::File::create(output).await?;
    tokio::io::copy(&mut buf_reader, &mut file_2).await?;

    Ok(())
}

fn convert_error(err: reqwest::Error) -> std::io::Error {
    todo!()
}

fn check_last_update(last_update: DateTime<Utc>, last_update_type: LastUpdatesType) -> bool {
    let today = chrono::offset::Utc::now();
    if last_update.year() != today.year() {
        return false;
    };
    if last_update.month() != today.month() {
        return false;
    };
    if last_update.day() != today.day() {
        return false;
    };
    if last_update_type == LastUpdatesType::Full {
        return true;
    };
    if last_update.hour() != today.hour() {
        return false;
    };
    return true;
}

pub async fn load_last_full() -> Promise<()> {
    let today = chrono::offset::Utc::now();
    let url = get_url_of_full_package(today);
    let output_path = String::from("data/MLS-full-cell-export.csv");

    let last_update = Utc.from_utc_datetime(&get_last_update(LastUpdatesType::Full)?);

    if check_last_update(DateTime::from(last_update), LastUpdatesType::Full) {
        return Ok(());
    }
    println!("Start to load the last full data set.");

    load_url(url, output_path.clone()).await?;
    println!("Load the full raw data set.");
    load_data(output_path)?;
    println!("Upload the data set to the database.");

    set_last_update(LastUpdatesType::Full, today.naive_utc())?;
    println!("Successfully update the full data set.");
    Ok(())
}

pub async fn load_last_diff() -> Promise<()> {
    let today = chrono::offset::Utc::now();
    let url = get_url_of_diff_package(today);
    let output_path = String::from("data/MLS-diff-cell-export.csv");

    let last_update = Utc.from_utc_datetime(&get_last_update(LastUpdatesType::Diff)?);

    if check_last_update(DateTime::from(last_update), LastUpdatesType::Diff) {
        return Ok(());
    }
    println!("Start to load the last diff data set.");

    load_url(url, String::from("data/MLS-diff-cell-export.csv")).await?;
    println!("Load the full raw data set.");
    load_data(output_path)?;
    println!("Upload the data set to the database.");

    set_last_update(LastUpdatesType::Diff, today.naive_utc())?;
    println!("Successfully update the diff data set.");

    Ok(())
}

pub fn load_data(input_path: String) -> Result<(), Error> {
    // TODO: make async
    let full_path = match input_path.starts_with("/") {
        true => input_path,
        false => {
            let mut path = env::current_dir()?;
            path.push(input_path);
            let path_full = path.clone();
            String::from(path_full.to_str().unwrap())
        }
    };
    let connection = &mut establish_connection();

    let res = diesel::sql_query(format!("
    LOAD DATA INFILE {:?}
    REPLACE INTO TABLE cells
    FIELDS TERMINATED BY ','
    LINES TERMINATED BY '\r\n'
    IGNORE 1 LINES
    (radio, mcc, net, area, cell, @unit, lon, lat, cell_range, samples, changeable, @created, @updated, @average_signal)
    SET
    unit = NULLIF(@unit, ''),
    average_signal = NULLIF(@average_signal, ''),
    created = FROM_UNIXTIME(@created),
    updated = FROM_UNIXTIME(@updated);", full_path)).execute(connection);

    match res {
        Ok(writes) => println!("Success: {:?} writes.", writes),
        Err(e) => return Err(Error::new(std::io::ErrorKind::Other, e.to_string())),
    }
    Ok(())
}

pub async fn update_loop(halt: &Cell<bool>) -> Promise<()> {
    println!("Init update loop.");

    while !halt.get() {
        load_last_full().await?;
        load_last_diff().await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }

    Ok(())
}
