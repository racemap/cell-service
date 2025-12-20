use std::env;
use std::io::Error;
use std::sync::Arc;

use crate::models::LastUpdatesType;
use async_compression::tokio::bufread::GzipDecoder;
use chrono::DateTime;
use chrono::Datelike;
use chrono::TimeZone;
use chrono::Utc;

use super::update_type::get_update_type;
use diesel::RunQueryDsl;
use futures::stream::TryStreamExt;
use lazy_static::lazy_static;
use tokio::sync::Mutex;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, info};

#[derive(serde::Deserialize)]
struct ErrorResponse {
    message: String,
}

use super::db::establish_connection;
use super::db::get_last_update;
use super::db::set_last_update;
use super::utils::Promise;

lazy_static! {
    static ref OUTPUT_FOLDER: String =
        env::var("TEMP_FOLDER").unwrap_or(String::from("/tmp/racemap-cell-service/data"));
}

fn get_url_of_full_package() -> String {
    let basic_url = env::var("DOWNLOAD_SOURCE_URL")
        .unwrap_or(String::from("https://opencellid.org/ocid/downloads"));
    let token = env::var("DOWNLOAD_SOURCE_TOKEN").expect("DOWNLOAD_SOURCE_TOKEN must be set");
    format!(
        "{}?token={}&type=full&file=cell_towers.csv.gz",
        basic_url, token
    )
}

fn get_url_of_diff_package(date: chrono::DateTime<Utc>) -> String {
    let basic_url = env::var("DOWNLOAD_SOURCE_URL")
        .unwrap_or(String::from("https://opencellid.org/ocid/downloads"));
    let token = env::var("DOWNLOAD_SOURCE_TOKEN").expect("DOWNLOAD_SOURCE_TOKEN must be set");
    let year = date.year();
    let month = date.month();
    let day = date.day();
    format!(
        "{}?token={}&type=diff&file=OCID-diff-cell-export-{:04}-{:02}-{:02}-T000000.csv.gz",
        basic_url, token, year, month, day
    )
}

async fn load_url(url: String, output: String) -> Promise<()> {
    let response = reqwest::get(url.clone()).await?;
    let status_code = response.status();
    let content_type = response.headers().get("Content-Type").unwrap().to_str()?;

    match content_type {
        "application/json" => {
            let error_message = response.json::<ErrorResponse>().await?;
            return Err(error_message.message.into());
        }
        "application/gzip" => {}
        _ => {
            return Err(format!("Request failed status: {}", status_code).into());
        }
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

fn convert_error(_err: reqwest::Error) -> std::io::Error {
    todo!()
}

pub async fn load_last_full() -> Promise<()> {
    let url = get_url_of_full_package();
    let output_path = String::from(format!("{}/full-cell-export.csv", *OUTPUT_FOLDER));
    info!("Start to load the last full data set.");

    match load_url(url, output_path.clone()).await {
        Ok(_) => {}
        Err(e) => {
            info!("Load Data Error: {}", e);
            return Ok(());
        }
    }
    info!("Load the full raw data set.");
    load_data(output_path)?;
    info!("Upload the data set to the database.");

    let today = chrono::offset::Utc::now();
    set_last_update(LastUpdatesType::Full, today.naive_utc())?;
    info!("Successfully update the full data set.");
    Ok(())
}

pub async fn load_last_diff() -> Promise<()> {
    let today = chrono::offset::Utc::now();
    let url = get_url_of_diff_package(today);
    let output_path = String::from(format!("{}/diff-cell-export.csv", *OUTPUT_FOLDER));
    info!("Start to load the last diff data set.");

    load_url(url, output_path.clone()).await?;
    info!("Load the last diff raw data set.");
    load_data(output_path)?;
    info!("Upload the data set to the database.");

    set_last_update(LastUpdatesType::Diff, today.naive_utc())?;
    info!("Successfully update the diff data set.");

    Ok(())
}

pub async fn update_local_database() -> Promise<()> {
    let last_update = Utc.from_utc_datetime(&get_last_update().unwrap());
    let now = chrono::offset::Utc::now();

    match get_update_type(DateTime::from(last_update), now) {
        None => Ok(()),
        Some(LastUpdatesType::Full) => load_last_full().await,
        Some(LastUpdatesType::Diff) => load_last_diff().await,
    }
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

    info!("Load data from: {:?}", full_path);
    let res = diesel::sql_query(format!("
    LOAD DATA INFILE {:?}
    REPLACE INTO TABLE cells
    FIELDS TERMINATED BY ','
    LINES TERMINATED BY '\n'
    IGNORE 1 LINES
    (radio, mcc, net, area, cell, @unit, lon, lat, cell_range, samples, changeable, @created, @updated, @average_signal)
    SET
    unit = NULLIF(@unit, '-1'),
    average_signal = NULLIF(@average_signal, ''),
    created = FROM_UNIXTIME(@created),
    updated = FROM_UNIXTIME(@updated);", full_path)).execute(connection);

    match res {
        Ok(writes) => info!("Success: {:?} writes.", writes),
        Err(e) => return Err(Error::new(std::io::ErrorKind::Other, e.to_string())),
    }
    Ok(())
}

// create output folder if not exists
pub async fn init() -> Promise<()> {
    let path = std::path::Path::new(&*OUTPUT_FOLDER);
    if !path.exists() {
        tokio::fs::create_dir_all(path).await?;
    }
    Ok(())
}

pub async fn update_loop(halt: &Arc<Mutex<bool>>) -> Promise<()> {
    info!("Init update loop.");
    init().await?;

    let mut count = 0;
    loop {
        if *halt.lock().await {
            break;
        }

        if (count % 600) == 0 {
            debug!("Check for updates!");
            update_local_database().await?;
            count = 0;
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        count += 1;
    }

    Ok(())
}
