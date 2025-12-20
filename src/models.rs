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
#[sql_type = "CellsRadioEnum"]
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
#[sql_type = "LastUpdatesUpdateTypeEnum"]
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
