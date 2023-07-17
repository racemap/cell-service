use std::{env, error::Error, io::Cursor};

use async_compression::tokio::bufread::GzipDecoder;
use chrono::Datelike;
use chrono::Timelike;
use chrono::Utc;
use diesel::RunQueryDsl;
use futures::stream::TryStreamExt;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tokio_util::io::StreamReader;

use super::db::establish_connection;
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

fn convert_error(err: reqwest::Error) -> std::io::Error {
    todo!()
}

pub async fn load_last_full() -> Promise<()> {
    let today = chrono::offset::Utc::now();
    let url = get_url_of_full_package(today);

    println!("Downloading from: {}", url);
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

    println!("Saving to: data/MLS-full-cell-export.csv");
    let mut file_2 = tokio::fs::File::create("data/MLS-full-cell-export.csv").await?;
    tokio::io::copy(&mut buf_reader, &mut file_2).await?;

    Ok(())
}

pub fn load_data(input_path: String) -> Result<(), Box<dyn Error>> {
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
        Err(e) => println!("Error: {:?}", e),
    }
    Ok(())
}
