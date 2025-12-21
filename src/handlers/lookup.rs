use serde::{Deserialize, Serialize};

use crate::{models::*, utils::db::establish_connection};
use diesel::prelude::*;
use diesel::MysqlConnection;

pub const LOOKUP_MAX_KEYS: usize = 50;

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellLookupKey {
    pub mcc: u16,
    pub mnc: u16,
    pub lac: u32,
    pub cid: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LookupCellsRequest {
    pub cells: Vec<CellLookupKey>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupCellsResponse {
    pub cells: Vec<Option<Cell>>,
}

fn is_better_lookup_candidate(candidate: &Cell, current: &Cell) -> bool {
    // If multiple radios exist for the same (mcc,mnc,lac,cid), we pick a single
    // "best" row deterministically.
    //
    // Preference:
    // 1) Higher samples (more evidence)
    // 2) Newer updated timestamp
    // 3) Higher radio generation (NR > LTE > UMTS > GSM > CDMA)
    if candidate.samples != current.samples {
        return candidate.samples > current.samples;
    }
    if candidate.updated != current.updated {
        return candidate.updated > current.updated;
    }

    fn radio_rank(r: &Radio) -> u8 {
        match r {
            Radio::Nr => 5,
            Radio::Lte => 4,
            Radio::Umts => 3,
            Radio::Gsm => 2,
            Radio::Cdma => 1,
        }
    }

    radio_rank(&candidate.radio) > radio_rank(&current.radio)
}

/// Batch-lookup cells by (mcc,mnc,lac,cid), returning one best match per key.
pub fn query_cells_lookup(
    keys: &[CellLookupKey],
    connection: &mut MysqlConnection,
) -> Result<Vec<Option<Cell>>, diesel::result::Error> {
    use crate::schema::cells::dsl::*;
    use std::collections::{HashMap, HashSet};

    if keys.is_empty() {
        return Ok(vec![]);
    }

    let unique_keys: Vec<CellLookupKey> = {
        let mut seen = HashSet::with_capacity(keys.len());
        let mut out = Vec::with_capacity(keys.len());
        for &k in keys {
            if seen.insert(k) {
                out.push(k);
            }
        }
        out
    };

    let mut db_query = cells.into_boxed();

    // Build OR conditions for up to LOOKUP_MAX_KEYS composite IDs.
    // With the stated p50 (~50) this is acceptable.
    let mut first = true;
    for k in unique_keys.iter().take(LOOKUP_MAX_KEYS) {
        let cond = mcc
            .eq(k.mcc)
            .and(net.eq(k.mnc))
            .and(area.eq(k.lac))
            .and(cell.eq(k.cid));
        if first {
            db_query = db_query.filter(cond);
            first = false;
        } else {
            db_query = db_query.or_filter(cond);
        }
    }

    // If the request had more than the max, we only attempt lookups for the first N.
    // The handler will pad the remainder with nulls.
    let matched_rows: Vec<Cell> = db_query.load(connection)?;

    let mut best_by_key: HashMap<(u16, u16, u32, u64), Cell> = HashMap::new();
    for row in matched_rows {
        let key_tuple = (row.mcc, row.net, row.area, row.cell);
        match best_by_key.get(&key_tuple) {
            None => {
                best_by_key.insert(key_tuple, row);
            }
            Some(current) => {
                if is_better_lookup_candidate(&row, current) {
                    best_by_key.insert(key_tuple, row);
                }
            }
        }
    }

    let mut out = Vec::with_capacity(keys.len());
    for (i, k) in keys.iter().enumerate() {
        if i >= LOOKUP_MAX_KEYS {
            out.push(None);
            continue;
        }
        out.push(best_by_key.get(&(k.mcc, k.mnc, k.lac, k.cid)).cloned());
    }

    Ok(out)
}

pub async fn handle_lookup_cells(
    req: LookupCellsRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let connection = &mut establish_connection();

    let mut results = match query_cells_lookup(&req.cells, connection) {
        Ok(r) => r,
        Err(_) => vec![None; req.cells.len().min(LOOKUP_MAX_KEYS)],
    };

    // If the caller sent more than LOOKUP_MAX_KEYS keys, pad with nulls so the
    // response still aligns 1:1 with the request.
    if req.cells.len() > results.len() {
        results.resize(req.cells.len(), None);
    }

    Ok(warp::reply::json(&LookupCellsResponse { cells: results }))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod lookup_cells_request {
        use super::*;

        #[test]
        fn test_deserialize_request() {
            let json = r#"{
                "cells": [
                    {"mcc": 262, "mnc": 1, "lac": 123, "cid": 456},
                    {"mcc": 262, "mnc": 1, "lac": 124, "cid": 457}
                ]
            }"#;

            let req: LookupCellsRequest = serde_json::from_str(json).unwrap();
            assert_eq!(req.cells.len(), 2);
            assert_eq!(req.cells[0].mcc, 262);
            assert_eq!(req.cells[0].mnc, 1);
            assert_eq!(req.cells[0].lac, 123);
            assert_eq!(req.cells[0].cid, 456);
        }

        #[test]
        fn test_key_roundtrip_serialize() {
            let key = CellLookupKey {
                mcc: 310,
                mnc: 410,
                lac: 5000,
                cid: 6000,
            };

            let json = serde_json::to_string(&key).unwrap();
            let de: CellLookupKey = serde_json::from_str(&json).unwrap();
            assert_eq!(de, key);
        }
    }
}
