use chrono::{DateTime, Datelike, Timelike, Utc};
use tracing::{debug, info};

use crate::models::LastUpdatesType;

/// The hour (UTC) after which OpenCellID diff files are available.
/// OpenCellID uploads new packages at 3am UTC, so we wait until 4am to be safe.
const UPDATE_AVAILABLE_HOUR_UTC: u32 = 4;

/// Determines the type of update needed based on the last update timestamp.
/// Returns `None` if no update is needed (already updated today or before 4am UTC).
pub fn get_update_type(last_update: DateTime<Utc>, now: DateTime<Utc>) -> Option<LastUpdatesType> {
    debug!("Last update was: {}", last_update);

    // Wait until 4am UTC when OpenCellID diff files are available
    // (OpenCellID uploads new packages at 3am UTC)
    if now.hour() < UPDATE_AVAILABLE_HOUR_UTC {
        debug!(
            "Before {}:00 UTC. Waiting for new data packages to be available.",
            UPDATE_AVAILABLE_HOUR_UTC
        );
        return None;
    }

    if last_update.timestamp() == 0 {
        info!("No last update found. Make a full update.");
        return Some(LastUpdatesType::Full);
    };

    if last_update.year() != now.year() {
        info!("Last update was last year. Make a full update.");
        return Some(LastUpdatesType::Full);
    };
    if last_update.month() != now.month() {
        info!("Last update was last month. Make a full update.");
        return Some(LastUpdatesType::Full);
    };
    if last_update.day() == now.day() {
        info!("Last update was today. Skip update.");
        return None;
    };

    let diff = now - last_update;
    debug!("Last update was {} hours ago.", diff.num_hours());
    debug!("Last update was {} days ago.", diff.num_days());

    if (diff.num_days() <= 1) && (diff.num_hours() < 24) {
        info!("Last update was yesterday. Make a diff update.");
        return Some(LastUpdatesType::Diff);
    };

    info!("Last update was more than one day ago. Make a full update.");
    Some(LastUpdatesType::Full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
            .unwrap()
    }

    #[test]
    fn test_no_previous_update_returns_full() {
        let last_update = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let now = utc(2025, 12, 20, 10, 0, 0);

        assert_eq!(
            get_update_type(last_update, now),
            Some(LastUpdatesType::Full)
        );
    }

    #[test]
    fn test_same_day_returns_none() {
        let last_update = utc(2025, 12, 20, 8, 0, 0);
        let now = utc(2025, 12, 20, 10, 0, 0);

        assert_eq!(get_update_type(last_update, now), None);
    }

    #[test]
    fn test_yesterday_within_24h_returns_diff() {
        let last_update = utc(2025, 12, 19, 20, 0, 0);
        let now = utc(2025, 12, 20, 10, 0, 0); // 14 hours later

        assert_eq!(
            get_update_type(last_update, now),
            Some(LastUpdatesType::Diff)
        );
    }

    #[test]
    fn test_yesterday_over_24h_returns_full() {
        let last_update = utc(2025, 12, 19, 8, 0, 0);
        let now = utc(2025, 12, 20, 10, 0, 0); // 26 hours later

        assert_eq!(
            get_update_type(last_update, now),
            Some(LastUpdatesType::Full)
        );
    }

    #[test]
    fn test_different_month_returns_full() {
        let last_update = utc(2025, 11, 30, 10, 0, 0);
        let now = utc(2025, 12, 1, 10, 0, 0);

        assert_eq!(
            get_update_type(last_update, now),
            Some(LastUpdatesType::Full)
        );
    }

    #[test]
    fn test_different_year_returns_full() {
        let last_update = utc(2024, 12, 31, 23, 0, 0);
        let now = utc(2025, 1, 1, 10, 0, 0); // After 4am UTC

        assert_eq!(
            get_update_type(last_update, now),
            Some(LastUpdatesType::Full)
        );
    }

    #[test]
    fn test_two_days_ago_returns_full() {
        let last_update = utc(2025, 12, 18, 10, 0, 0);
        let now = utc(2025, 12, 20, 10, 0, 0);

        assert_eq!(
            get_update_type(last_update, now),
            Some(LastUpdatesType::Full)
        );
    }

    #[test]
    fn test_before_4am_utc_returns_none() {
        let last_update = utc(2025, 12, 19, 10, 0, 0);
        let now = utc(2025, 12, 20, 3, 30, 0); // 3:30am UTC, before 4am

        assert_eq!(get_update_type(last_update, now), None);
    }

    #[test]
    fn test_after_4am_utc_allows_update() {
        let last_update = utc(2025, 12, 19, 10, 0, 0);
        let now = utc(2025, 12, 20, 4, 0, 0); // Exactly 4am UTC

        assert_eq!(
            get_update_type(last_update, now),
            Some(LastUpdatesType::Diff)
        );
    }
}
