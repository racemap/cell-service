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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_flatten_returns_ok_on_success() {
        let handle = tokio::spawn(async { Ok::<_, FutureError>(42) });

        let result = flatten(handle).await;

        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_flatten_returns_err_string_on_inner_error() {
        let handle = tokio::spawn(async { Err::<i32, FutureError>("something went wrong".into()) });

        let result = flatten(handle).await;

        assert_eq!(result, Err(String::from("something went wrong")));
    }

    #[tokio::test]
    async fn test_flatten_returns_handling_failed_on_join_error() {
        let handle: JoinHandle<Result<i32, FutureError>> = tokio::spawn(async {
            panic!("task panicked");
        });

        let result = flatten(handle).await;

        assert_eq!(result, Err(String::from("handling failed")));
    }

    #[tokio::test]
    async fn test_flatten_works_with_string_result() {
        let handle = tokio::spawn(async { Ok::<_, FutureError>(String::from("hello")) });

        let result = flatten(handle).await;

        assert_eq!(result, Ok(String::from("hello")));
    }
}
