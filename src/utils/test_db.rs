use diesel::Connection;
use diesel::MysqlConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use testcontainers::core::ImageExt;
use testcontainers::runners::SyncRunner;
use testcontainers::Container;
use testcontainers_modules::mariadb::Mariadb;

const MARIADB_VERSION: &str = "11.4";
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a random database name like `test-<random>`.
///
/// This is used to isolate integration tests from each other to avoid lock
/// contention and deadlocks when tests run in parallel.
pub fn random_db_name() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock before UNIX_EPOCH")
        .as_nanos();
    let counter = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("test-{:x}-{:x}", now, counter)
}

/// Options for creating a test database connection.
#[derive(Default)]
pub struct TestConnectionOptions {
    /// File to copy into the container (source path, destination path in container).
    pub copy_file: Option<(PathBuf, &'static str)>,
    /// Whether to wrap the connection in a test transaction.
    /// Set to false for operations like `LOAD DATA INFILE` that don't work in transactions.
    pub use_test_transaction: bool,
}

/// Start a MariaDB testcontainer with a fresh, unique database and return a
/// Diesel connection.
///
/// The container is returned so the caller can keep it alive for the duration
/// of the test (bind it to a local like `_container`).
pub fn get_test_connection_with_options(
    options: TestConnectionOptions,
) -> (Container<Mariadb>, MysqlConnection) {
    let db_name = random_db_name();

    let mut container_config = Mariadb::default()
        .with_tag(MARIADB_VERSION)
        .with_env_var("MYSQL_DATABASE", db_name.clone())
        .with_env_var("MARIADB_DATABASE", db_name.clone());

    if let Some((source, dest)) = options.copy_file {
        container_config = container_config.with_copy_to(dest, source);
    }

    let container = container_config
        .start()
        .expect("Failed to start MariaDB container. Is Docker running?");

    let host_port = container
        .get_host_port_ipv4(3306)
        .expect("Failed to get MySQL port");

    let database_url = format!("mysql://root@127.0.0.1:{}/{}", host_port, db_name);
    let mut conn =
        MysqlConnection::establish(&database_url).expect("Failed to connect to test database");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    if options.use_test_transaction {
        conn.begin_test_transaction()
            .expect("Failed to begin test transaction");
    }

    (container, conn)
}

/// Start a MariaDB testcontainer with a fresh, unique database and return a
/// Diesel connection inside a test transaction.
///
/// The container is returned so the caller can keep it alive for the duration
/// of the test (bind it to a local like `_container`).
pub fn get_test_connection() -> (Container<Mariadb>, MysqlConnection) {
    get_test_connection_with_options(TestConnectionOptions {
        use_test_transaction: true,
        ..Default::default()
    })
}
