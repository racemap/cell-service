use chrono::{DateTime, Datelike, Utc};

use crate::utils::config::Config;

/// Builds the URL for the full cell tower data package.
pub fn get_url_of_full_package(config: Config) -> String {
    let basic_url = config.download_source_url;
    let token = config.download_source_token;
    build_full_package_url(&basic_url, &token)
}

/// Builds the URL for the diff cell tower data package for a given date.
pub fn get_url_of_diff_package(date: DateTime<Utc>, config: Config) -> String {
    let basic_url = config.download_source_url;
    let token = config.download_source_token;
    build_diff_package_url(&basic_url, &token, date)
}

/// Builds the full package URL from components (testable without env vars).
pub fn build_full_package_url(base_url: &str, token: &str) -> String {
    format!(
        "{}?token={}&type=full&file=cell_towers.csv.gz",
        base_url, token
    )
}

/// Builds the diff package URL from components (testable without env vars).
pub fn build_diff_package_url(base_url: &str, token: &str, date: DateTime<Utc>) -> String {
    let year = date.year();
    let month = date.month();
    let day = date.day();
    format!(
        "{}?token={}&type=diff&file=OCID-diff-cell-export-{:04}-{:02}-{:02}-T000000.csv.gz",
        base_url, token, year, month, day
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const TEST_BASE_URL: &str = "https://example.com/downloads";
    const TEST_TOKEN: &str = "test-token-123";

    #[test]
    fn test_build_full_package_url() {
        let url = build_full_package_url(TEST_BASE_URL, TEST_TOKEN);

        assert_eq!(
            url,
            "https://example.com/downloads?token=test-token-123&type=full&file=cell_towers.csv.gz"
        );
    }

    #[test]
    fn test_build_diff_package_url_formats_date_correctly() {
        let date = Utc.with_ymd_and_hms(2025, 12, 20, 10, 0, 0).unwrap();
        let url = build_diff_package_url(TEST_BASE_URL, TEST_TOKEN, date);

        assert_eq!(
            url,
            "https://example.com/downloads?token=test-token-123&type=diff&file=OCID-diff-cell-export-2025-12-20-T000000.csv.gz"
        );
    }

    #[test]
    fn test_build_diff_package_url_pads_single_digit_month() {
        let date = Utc.with_ymd_and_hms(2025, 3, 15, 0, 0, 0).unwrap();
        let url = build_diff_package_url(TEST_BASE_URL, TEST_TOKEN, date);

        assert!(url.contains("2025-03-15"), "Month should be zero-padded");
    }

    #[test]
    fn test_build_diff_package_url_pads_single_digit_day() {
        let date = Utc.with_ymd_and_hms(2025, 11, 5, 0, 0, 0).unwrap();
        let url = build_diff_package_url(TEST_BASE_URL, TEST_TOKEN, date);

        assert!(url.contains("2025-11-05"), "Day should be zero-padded");
    }

    #[test]
    fn test_build_diff_package_url_handles_new_year() {
        let date = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let url = build_diff_package_url(TEST_BASE_URL, TEST_TOKEN, date);

        assert!(url.contains("2026-01-01"));
    }

    #[test]
    fn test_build_diff_package_url_handles_leap_year() {
        let date = Utc.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap();
        let url = build_diff_package_url(TEST_BASE_URL, TEST_TOKEN, date);

        assert!(url.contains("2024-02-29"));
    }
}
