use std::env;
use std::io::Error;
use std::sync::Arc;

use crate::models::LastUpdatesType;
use crate::utils::config::Config;
use async_compression::tokio::bufread::GzipDecoder;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;

use super::update_type::get_update_type;
use super::url_builder::{get_url_of_diff_package, get_url_of_full_package};
use diesel::RunQueryDsl;
use futures::stream::TryStreamExt;
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

pub async fn load_last_full(config: Config) -> Promise<()> {
    let url = get_url_of_full_package(config.clone());
    let output_folder = config.output_folder.clone();
    let output_path = String::from(format!("{}/full-cell-export.csv", output_folder));
    info!("Start to load the last full data set.");

    match load_url(url, output_path.clone()).await {
        Ok(_) => {}
        Err(e) => {
            info!("Load Data Error: {}", e);
            return Ok(());
        }
    }
    info!("Load the full raw data set.");
    load_data(output_path, config.clone())?;
    info!("Upload the data set to the database.");

    let today = chrono::offset::Utc::now();
    set_last_update(LastUpdatesType::Full, today.naive_utc(), config.clone())?;
    info!("Successfully update the full data set.");
    Ok(())
}

pub async fn load_last_diff(config: Config) -> Promise<()> {
    let today = chrono::offset::Utc::now();
    let url = get_url_of_diff_package(today, config.clone());
    let output_folder = config.output_folder.clone();
    let output_path = String::from(format!("{}/diff-cell-export.csv", output_folder));
    info!("Start to load the last diff data set.");

    load_url(url, output_path.clone()).await?;
    info!("Load the last diff raw data set.");
    load_data(output_path, config.clone())?;
    info!("Upload the data set to the database.");

    set_last_update(LastUpdatesType::Diff, today.naive_utc(), config.clone())?;
    info!("Successfully update the diff data set.");

    Ok(())
}

pub async fn update_local_database(config: Config) -> Promise<()> {
    let last_update = Utc.from_utc_datetime(&get_last_update(config.clone()).unwrap());
    let now = chrono::offset::Utc::now();

    match get_update_type(DateTime::from(last_update), now) {
        None => Ok(()),
        Some(LastUpdatesType::Full) => load_last_full(config.clone()).await,
        Some(LastUpdatesType::Diff) => load_last_diff(config.clone()).await,
    }
}

pub fn load_data(input_path: String, config: Config) -> Result<(), Error> {
    let connection = &mut establish_connection(config.clone());
    load_data_with_connection(input_path, connection)
}

/// Load CSV data into the database using the provided connection.
/// This is the testable version that accepts a connection parameter.
pub fn load_data_with_connection(
    input_path: String,
    connection: &mut diesel::MysqlConnection,
) -> Result<(), Error> {
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
pub async fn init(config: Config) -> Promise<()> {
    let path = std::path::Path::new(&*config.output_folder);
    if !path.exists() {
        tokio::fs::create_dir_all(path).await?;
    }
    Ok(())
}

pub async fn update_loop(halt: &Arc<Mutex<bool>>, config: Config) -> Promise<()> {
    info!("Init update loop.");
    init(config.clone()).await?;

    let mut count = 0;
    loop {
        if *halt.lock().await {
            break;
        }

        if (count % 600) == 0 {
            debug!("Check for updates!");
            update_local_database(config.clone()).await?;
            count = 0;
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        count += 1;
    }

    Ok(())
}

/// Integration tests for load_data using testcontainers.
/// Tests loading CSV data into the database.
///
/// Run with: cargo test --features integration_tests load_data
#[cfg(all(test, feature = "integration_tests"))]
mod tests {
    use super::*;
    use crate::models::{Cell, Radio};
    use crate::schema::cells::dsl::*;
    use diesel::Connection;
    use diesel::ExpressionMethods;
    use diesel::MysqlConnection;
    use diesel::QueryDsl;
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    use std::sync::OnceLock;
    use testcontainers::core::ImageExt;
    use testcontainers::runners::SyncRunner;
    use testcontainers::Container;
    use testcontainers_modules::mariadb::Mariadb;

    const MARIADB_VERSION: &str = "11.4";
    const CONTAINER_CSV_PATH: &str = "/var/lib/mysql-files/test-export.csv";

    pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

    // Shared container across all tests - initialized once
    static TEST_DB: OnceLock<(Container<Mariadb>, String)> = OnceLock::new();

    /// Initialize the shared test database container once.
    /// Returns the database URL for creating connections.
    fn init_test_db() -> &'static str {
        let (_, url) = TEST_DB.get_or_init(|| {
            // Get the path to the test CSV file from the tests/fixtures directory
            // This path is committed to git and available in CI
            let test_csv_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/test-cells.csv");

            let container = Mariadb::default()
                .with_tag(MARIADB_VERSION)
                .with_copy_to(CONTAINER_CSV_PATH, test_csv_path)
                .start()
                .expect("Failed to start MariaDB container. Is Docker running?");

            let host_port = container
                .get_host_port_ipv4(3306)
                .expect("Failed to get MySQL port");

            let database_url = format!("mysql://root@127.0.0.1:{}/test", host_port);

            // Run migrations once
            let mut conn = MysqlConnection::establish(&database_url)
                .expect("Failed to connect to test database");
            conn.run_pending_migrations(MIGRATIONS)
                .expect("Failed to run migrations");

            (container, database_url)
        });
        url
    }

    /// Get a fresh connection with a clean database state.
    /// Uses TRUNCATE to clear tables since LOAD DATA INFILE doesn't work in transactions.
    fn get_test_connection() -> MysqlConnection {
        let url = init_test_db();
        let mut conn = MysqlConnection::establish(url).expect("Failed to connect to test database");

        // Clean state for each test
        diesel::sql_query("TRUNCATE TABLE cells")
            .execute(&mut conn)
            .expect("Failed to truncate cells table");

        conn
    }

    #[test]
    fn test_load_data_imports_csv_file() {
        let mut conn = get_test_connection();

        // Use the actual load_data_with_connection function
        let result = load_data_with_connection(CONTAINER_CSV_PATH.to_string(), &mut conn);
        assert!(result.is_ok(), "Failed to load CSV: {:?}", result.err());

        // Verify data was loaded - test CSV has 99 data rows (100 lines - 1 header)
        let count: i64 = cells
            .count()
            .get_result(&mut conn)
            .expect("Failed to count cells");

        assert!(count > 0, "No cells were loaded from CSV");
        assert_eq!(count, 99, "Expected 99 cells from test CSV");
    }

    #[test]
    fn test_load_data_can_query_specific_cell() {
        let mut conn = get_test_connection();

        // Load using the actual function
        load_data_with_connection(CONTAINER_CSV_PATH.to_string(), &mut conn)
            .expect("Failed to load CSV data");

        // Query for the first cell from the test CSV:
        // GSM,262,2,317,11911,0,13.4524,52.5075,1454,122,1,1288894949,1724275323,0
        let found_cell: Cell = cells
            .filter(mcc.eq(262_u16))
            .filter(net.eq(2_u16))
            .filter(area.eq(317_u32))
            .filter(cell.eq(11911_u64))
            .first(&mut conn)
            .expect("Failed to find cell");

        assert_eq!(found_cell.mcc, 262);
        assert_eq!(found_cell.net, 2);
        assert_eq!(found_cell.area, 317);
        assert_eq!(found_cell.cell, 11911);
        assert!(matches!(found_cell.radio, Radio::Gsm));
        // Check coordinates (approximately)
        assert!((found_cell.lon - 13.4524).abs() < 0.001);
        assert!((found_cell.lat - 52.5075).abs() < 0.001);
    }

    #[test]
    fn test_load_data_query_three_random_cells() {
        let mut conn = get_test_connection();

        // Load using the actual function
        load_data_with_connection(CONTAINER_CSV_PATH.to_string(), &mut conn)
            .expect("Failed to load CSV data");

        // Query 3 random cells from the database
        let random_cells: Vec<Cell> = diesel::sql_query(
            "SELECT radio, mcc, net, area, cell, unit, lon, lat, cell_range, samples, changeable, created, updated, average_signal 
             FROM cells ORDER BY RAND() LIMIT 3",
        )
        .load(&mut conn)
        .expect("Failed to query random cells");

        assert_eq!(random_cells.len(), 3, "Expected 3 random cells");

        // Verify each cell can be queried back via Diesel
        for random_cell in &random_cells {
            let found: Cell = cells
                .filter(mcc.eq(random_cell.mcc))
                .filter(net.eq(random_cell.net))
                .filter(area.eq(random_cell.area))
                .filter(cell.eq(random_cell.cell))
                .first(&mut conn)
                .expect("Failed to find random cell");

            assert_eq!(found.mcc, random_cell.mcc);
            assert_eq!(found.net, random_cell.net);
        }

        // Print the random cells for visibility
        println!("Successfully queried 3 random cells:");
        for (i, c) in random_cells.iter().enumerate() {
            println!(
                "  {}: {:?} mcc={} net={} area={} cell={} @ ({}, {})",
                i + 1,
                c.radio,
                c.mcc,
                c.net,
                c.area,
                c.cell,
                c.lat,
                c.lon
            );
        }
    }
}
