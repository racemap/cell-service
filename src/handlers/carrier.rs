use serde::{Deserialize, Serialize};
use tracing::instrument;
use warp::http::StatusCode;

use crate::utils::carrier::{lookup, Carrier};

#[derive(Deserialize, Serialize, Debug)]
pub struct GetCarrierQuery {
    pub mcc: u16,
    /// `/cell` names this `net`, `/cells` names it `mnc`. Accept both.
    #[serde(alias = "mnc")]
    pub net: u16,
}

/// Resolves the carrier for an MCC/MNC pair. No database access — the table is compiled in, so
/// this needs neither `Config` nor the `query_*` split the DB-backed handlers use.
#[instrument]
pub fn handle_get_carrier(query: GetCarrierQuery) -> impl warp::Reply {
    let carrier = lookup(query.mcc, query.net);

    // ponytail: an all-default result means neither the exact pair nor the MCC fallback row
    // matched. Only 6 of 238 MCCs lack a fallback row (1, 901, 902, 991, 995, 999 — test and
    // satellite ranges), and their rows carry no country either, so a 200 would be three nulls.
    // Pinned by `server::tests::carrier_endpoint::test_known_mcc_without_fallback_row_returns_not_found`.
    // Swap in an explicit MCC-membership check if those ranges ever need to 200 with nulls.
    if carrier == Carrier::default() {
        return warp::reply::with_status(
            warp::reply::json(&serde_json::Value::Null),
            StatusCode::NOT_FOUND,
        );
    }

    warp::reply::with_status(warp::reply::json(&carrier), StatusCode::OK)
}
