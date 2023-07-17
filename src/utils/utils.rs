use std::{env, error::Error, ffi::OsString};

use tokio::task::JoinHandle;

pub type FutureError = Box<dyn std::error::Error + Send + Sync>;
pub type Promise<T> = std::result::Result<T, FutureError>;

pub fn get_first_arg() -> Result<OsString, Box<dyn Error>> {
    match env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}

pub async fn flatten<T>(handle: JoinHandle<Result<T, FutureError>>) -> Result<T, String> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err(String::from("handling failed")),
    }
}
