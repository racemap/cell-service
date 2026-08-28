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

# Upstream lists these as territory lists ("AU/CC/CX") but names the umbrella country, which has
# its own alpha-2. Keyed by name: upstream reorders territory lists more readily than it renames
# countries. One verified entry at a time — a general "first segment" rule would corrupt the
# groupings that have no alpha-2 of their own ("French Antilles" is not Saint Barthélemy).
UMBRELLA_CODES = {"Australia": "AU"}


def clean_country(name):
    """"Guam (United States of America)" -> "Guam"."""
    return re.sub(r"\s*\([^)]*\)\s*$", "", name).strip() if name else ""


def normalize(record):
    """One upstream record to one output row, or None if its MNC is not a code.

    Prefers the consumer-facing brand over the legal entity, and carries a sort rank so
    duplicate (mcc, mnc) keys can be collapsed later.
    """
    try:
        mnc = int(record["mnc"])
    except (TypeError, ValueError):
        return None  # a few MNC values are ranges or notes, not codes
    source_code = record.get("countryCode") or ""
    country = clean_country(record.get("countryName"))
    code = source_code
    if len(code) != 2:
        # "GE-AB" is ISO 3166-2; the sovereign parent precedes the dash. Multi-territory codes
        # ("BQ/CW/SX") resolve only where the umbrella country has a code of its own.
        resolved = code.split("-")[0] if "-" in code else UMBRELLA_CODES.get(country, "")
        code = resolved if len(resolved) == 2 else ""
    return {
        "mcc": int(record["mcc"]),
        "mnc": mnc,
        "operator": record.get("brand") or record.get("operator") or "",
        "country": country,
        "country_code": code,
        "rank": STATUS_RANK.get((record.get("status") or "").strip().lower(), 9),
        "source_code": source_code,  # reported at the end of a run; DictWriter ignores it
    }


def main():
    """Fetch the upstream list and write the lookup table Rust compiles in.

    Every ambiguity is resolved here rather than at runtime: duplicate keys collapse by
    status then by the MCC's dominant country, non-alpha-2 country codes resolve or drop, and
    each MCC gets an mnc-less fallback row. Output is sorted so the committed diff stays
    reviewable.
    """
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

    # Silent drops are how the missing Australian codes went unnoticed. Report, do not raise:
    # upstream legitimately carries groupings with no alpha-2, so a hard failure would be muted.
    unresolved = collections.Counter(
        row["source_code"] for row in rows if not row["country_code"] and row["source_code"]
    )
    for code, count in unresolved.most_common():
        print(f"  dropped non-alpha-2 country code {code!r}: {count} rows")


if __name__ == "__main__":
    main()
