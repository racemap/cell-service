#!/usr/bin/env python3
"""Regenerate src/utils/mcc-mnc.csv from the upstream MCC/MNC list.

Run after an upstream refresh, then review the committed diff:

    python3 scripts/update-mcc-mnc.py && git diff src/utils/mcc-mnc.csv
"""

import argparse
import collections
import csv
import json
import re
import urllib.request

SOURCE = (
    "https://raw.githubusercontent.com/cavoq/mcc-mnc-list/master/mcc-mnc-list.json"
)

# Better-attested rows win a duplicate (mcc, mnc) key; everything else ranks equal.
STATUS_RANK = {"operational": 0, "temporary operational": 1}


def clean_country(name):
    """"Guam (United States of America)" -> "Guam"."""
    return re.sub(r"\s*\([^)]*\)\s*$", "", name).strip() if name else ""


def normalize(record):
    try:
        mnc = int(record["mnc"])
    except (TypeError, ValueError):
        return None  # a few MNC values are ranges or notes, not codes
    code = record.get("countryCode") or ""
    return {
        "mcc": int(record["mcc"]),
        "mnc": mnc,
        "operator": record.get("brand") or record.get("operator") or "",
        "country": clean_country(record.get("countryName")),
        # Multi-territory codes ("BQ/CW/SX") and subdivisions ("GE-AB") are not alpha-2.
        "country_code": code if len(code) == 2 else "",
        "rank": STATUS_RANK.get((record.get("status") or "").strip().lower(), 9),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", default=SOURCE)
    parser.add_argument("--out", default="src/utils/mcc-mnc.csv")
    args = parser.parse_args()

    with urllib.request.urlopen(args.source) as response:
        records = json.load(response)

    rows = [r for r in (normalize(r) for r in records) if r]

    # An MCC can span several countries (310 = US/Guam/USVI/...), so the fallback for an
    # unknown MNC is the country most of that MCC's best-attested rows agree on.
    votes = collections.defaultdict(collections.Counter)
    for row in rows:
        if row["rank"] == 0 and row["country"]:
            votes[row["mcc"]][(row["country"], row["country_code"])] += 1
    for row in rows:
        if not votes[row["mcc"]] and row["country"]:
            votes[row["mcc"]][(row["country"], row["country_code"])] += 1
    main_country = {mcc: c.most_common(1)[0][0] for mcc, c in votes.items() if c}

    best = {}
    for index, row in enumerate(rows):
        key = (row["mcc"], row["mnc"])
        order = (
            row["rank"],
            0 if (row["country"], row["country_code"]) == main_country.get(row["mcc"]) else 1,
            index,
        )
        if key not in best or order < best[key][0]:
            best[key] = (order, row)

    out = [row for _, row in best.values()]
    # An empty mnc marks the MCC-level fallback used when the pair is unknown.
    out += [
        {"mcc": mcc, "mnc": "", "operator": "", "country": country, "country_code": code}
        for mcc, (country, code) in main_country.items()
    ]
    out.sort(key=lambda r: (r["mcc"], r["mnc"] if r["mnc"] != "" else -1))

    fields = ["mcc", "mnc", "operator", "country", "country_code"]
    with open(args.out, "w", newline="") as handle:
        writer = csv.DictWriter(handle, fields, extrasaction="ignore", lineterminator="\n")
        writer.writeheader()
        writer.writerows(out)

    print(f"{args.out}: {len(out)} rows ({len(main_country)} MCC fallbacks)")


if __name__ == "__main__":
    main()
