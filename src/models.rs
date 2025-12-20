use crate::schema::sql_types::{CellsRadioEnum, LastUpdatesUpdateTypeEnum};
use chrono::NaiveDateTime;
use diesel::deserialize::FromSql;
use diesel::mysql::{Mysql, MysqlValue};
use diesel::prelude::*;
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::*;

use serde_with::BoolFromInt;
use std::io::Write;

#[derive(Debug, serde::Deserialize, serde::Serialize, FromSqlRow, AsExpression)]
#[diesel(sql_type = CellsRadioEnum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Radio {
    Gsm,
    Umts,
    Cdma,
    Lte,
    Nr,
}

impl ToSql<CellsRadioEnum, Mysql> for Radio {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Mysql>) -> serialize::Result {
        match *self {
            Radio::Umts => out.write_all(b"umts")?,
            Radio::Gsm => out.write_all(b"gsm")?,
            Radio::Lte => out.write_all(b"lte")?,
            Radio::Nr => out.write_all(b"nr")?,
            Radio::Cdma => out.write_all(b"cdma")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<CellsRadioEnum, Mysql> for Radio {
    fn from_sql(bytes: MysqlValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"umts" => Ok(Radio::Umts),
            b"gsm" => Ok(Radio::Gsm),
            b"lte" => Ok(Radio::Lte),
            b"nr" => Ok(Radio::Nr),
            b"cdma" => Ok(Radio::Cdma),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[serde_with::serde_as]
#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::cells)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    radio: Radio,
    mcc: u16,
    net: u16,
    area: u32,
    cell: u64,
    unit: Option<u16>,
    lon: f32,
    lat: f32,
    #[serde(alias = "range")]
    cell_range: u32,
    samples: u32,
    #[serde_as(as = "BoolFromInt")]
    changeable: bool,
    #[serde_as(as = "chrono::DateTime<chrono::Utc>")]
    created: NaiveDateTime,
    #[serde_as(as = "chrono::DateTime<chrono::Utc>")]
    updated: NaiveDateTime,
    average_signal: Option<i16>,
}

#[derive(Debug, FromSqlRow, AsExpression, PartialEq, Eq)]
#[diesel(sql_type = LastUpdatesUpdateTypeEnum)]
pub enum LastUpdatesType {
    Full,
    Diff,
}

impl ToSql<LastUpdatesUpdateTypeEnum, Mysql> for LastUpdatesType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Mysql>) -> serialize::Result {
        match *self {
            LastUpdatesType::Full => out.write_all(b"full")?,
            LastUpdatesType::Diff => out.write_all(b"diff")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<LastUpdatesUpdateTypeEnum, Mysql> for LastUpdatesType {
    fn from_sql(bytes: MysqlValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"full" => Ok(LastUpdatesType::Full),
            b"diff" => Ok(LastUpdatesType::Diff),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::last_updates)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct LastUpdates {
    pub value: NaiveDateTime,
    pub update_type: LastUpdatesType,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod radio_serialization {
        use super::*;

        #[test]
        fn test_serialize_gsm() {
            let json = serde_json::to_string(&Radio::Gsm).unwrap();
            assert_eq!(json, "\"GSM\"");
        }

        #[test]
        fn test_serialize_umts() {
            let json = serde_json::to_string(&Radio::Umts).unwrap();
            assert_eq!(json, "\"UMTS\"");
        }

        #[test]
        fn test_serialize_cdma() {
            let json = serde_json::to_string(&Radio::Cdma).unwrap();
            assert_eq!(json, "\"CDMA\"");
        }

        #[test]
        fn test_serialize_lte() {
            let json = serde_json::to_string(&Radio::Lte).unwrap();
            assert_eq!(json, "\"LTE\"");
        }

        #[test]
        fn test_serialize_nr() {
            let json = serde_json::to_string(&Radio::Nr).unwrap();
            assert_eq!(json, "\"NR\"");
        }

        #[test]
        fn test_deserialize_gsm() {
            let radio: Radio = serde_json::from_str("\"GSM\"").unwrap();
            assert!(matches!(radio, Radio::Gsm));
        }

        #[test]
        fn test_deserialize_umts() {
            let radio: Radio = serde_json::from_str("\"UMTS\"").unwrap();
            assert!(matches!(radio, Radio::Umts));
        }

        #[test]
        fn test_deserialize_cdma() {
            let radio: Radio = serde_json::from_str("\"CDMA\"").unwrap();
            assert!(matches!(radio, Radio::Cdma));
        }

        #[test]
        fn test_deserialize_lte() {
            let radio: Radio = serde_json::from_str("\"LTE\"").unwrap();
            assert!(matches!(radio, Radio::Lte));
        }

        #[test]
        fn test_deserialize_nr() {
            let radio: Radio = serde_json::from_str("\"NR\"").unwrap();
            assert!(matches!(radio, Radio::Nr));
        }

        #[test]
        fn test_deserialize_invalid_returns_error() {
            let result: Result<Radio, _> = serde_json::from_str("\"INVALID\"");
            assert!(result.is_err());
        }
    }

    mod cell_serialization {
        use super::*;
        use chrono::TimeZone;

        fn sample_cell() -> Cell {
            Cell {
                radio: Radio::Lte,
                mcc: 262,
                net: 1,
                area: 12345,
                cell: 67890123,
                unit: Some(42),
                lon: 13.405,
                lat: 52.52,
                cell_range: 1000,
                samples: 50,
                changeable: true,
                created: chrono::Utc
                    .with_ymd_and_hms(2024, 1, 15, 10, 30, 0)
                    .unwrap()
                    .naive_utc(),
                updated: chrono::Utc
                    .with_ymd_and_hms(2025, 12, 20, 14, 0, 0)
                    .unwrap()
                    .naive_utc(),
                average_signal: Some(-85),
            }
        }

        #[test]
        fn test_serialize_uses_camel_case_field_names() {
            let cell = sample_cell();
            let json = serde_json::to_string(&cell).unwrap();

            assert!(json.contains("\"cellRange\""));
            assert!(json.contains("\"averageSignal\""));
            assert!(!json.contains("\"cell_range\""));
            assert!(!json.contains("\"average_signal\""));
        }

        #[test]
        fn test_serialize_radio_as_screaming_snake_case() {
            let cell = sample_cell();
            let json = serde_json::to_string(&cell).unwrap();

            assert!(json.contains("\"radio\":\"LTE\""));
        }

        #[test]
        fn test_serialize_optional_fields() {
            let mut cell = sample_cell();
            cell.unit = None;
            cell.average_signal = None;

            let json = serde_json::to_string(&cell).unwrap();

            assert!(json.contains("\"unit\":null"));
            assert!(json.contains("\"averageSignal\":null"));
        }

        #[test]
        fn test_deserialize_with_range_alias() {
            let json = r#"{
                "radio": "GSM",
                "mcc": 262,
                "net": 1,
                "area": 100,
                "cell": 200,
                "unit": null,
                "lon": 10.0,
                "lat": 50.0,
                "range": 500,
                "samples": 10,
                "changeable": 1,
                "created": "2024-01-01T00:00:00Z",
                "updated": "2024-01-01T00:00:00Z",
                "averageSignal": null
            }"#;

            let cell: Cell = serde_json::from_str(json).unwrap();

            assert_eq!(cell.cell_range, 500);
        }

        #[test]
        fn test_deserialize_changeable_from_int() {
            let json = r#"{
                "radio": "GSM",
                "mcc": 262,
                "net": 1,
                "area": 100,
                "cell": 200,
                "unit": null,
                "lon": 10.0,
                "lat": 50.0,
                "cellRange": 500,
                "samples": 10,
                "changeable": 0,
                "created": "2024-01-01T00:00:00Z",
                "updated": "2024-01-01T00:00:00Z",
                "averageSignal": null
            }"#;

            let cell: Cell = serde_json::from_str(json).unwrap();

            assert!(!cell.changeable);
        }

        #[test]
        fn test_roundtrip_serialization() {
            let original = sample_cell();
            let json = serde_json::to_string(&original).unwrap();
            let deserialized: Cell = serde_json::from_str(&json).unwrap();

            assert!(matches!(deserialized.radio, Radio::Lte));
            assert_eq!(deserialized.mcc, original.mcc);
            assert_eq!(deserialized.cell, original.cell);
            assert_eq!(deserialized.cell_range, original.cell_range);
        }
    }

    mod last_updates_type {
        use super::*;

        #[test]
        fn test_full_equals_full() {
            assert_eq!(LastUpdatesType::Full, LastUpdatesType::Full);
        }

        #[test]
        fn test_diff_equals_diff() {
            assert_eq!(LastUpdatesType::Diff, LastUpdatesType::Diff);
        }

        #[test]
        fn test_full_not_equals_diff() {
            assert_ne!(LastUpdatesType::Full, LastUpdatesType::Diff);
        }

        #[test]
        fn test_debug_format() {
            assert_eq!(format!("{:?}", LastUpdatesType::Full), "Full");
            assert_eq!(format!("{:?}", LastUpdatesType::Diff), "Diff");
        }
    }
}
