use std::{env, error::Error, ffi::OsString};

pub type FutureError = Box<dyn std::error::Error + Send + Sync>;
pub type Promise<T> = std::result::Result<T, FutureError>;

pub fn get_first_arg() -> Result<OsString, Box<dyn Error>> {
    match env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}
