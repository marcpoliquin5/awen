#!/usr/bin/env python3
"""Import research-ledger rows from CSV or a Markdown table into ledger.json.

    python research/ledger/import_ledger.py papers.csv --ledger research/ledger/ledger.json
    python research/ledger/import_ledger.py papers.md  --ledger research/ledger/ledger.json --dry-run

Columns (header names, case-insensitive; extra columns are ignored):
  id, direction, title, authors (separated by ';'), year, venue, doi, url,
  summary, relevance, awen_state (implemented|partial|not-started), modules (';'),
  next_step, maturity (theory|simulation|lab-demonstration|product),
  hardware_needed, open_questions (';'), tags (';'), status,
  verification_method, verification_date (YYYY-MM-DD)

Honesty rules enforced here:
* A row is imported as ``verified`` only if it has a DOI or https URL, title,
  authors, year, *and* a verification method and date. Otherwise it is
  imported as ``citation pending``, whatever its status column says.
* Existing entries are never overwritten unless ``--replace`` is given.
* Every resulting ledger is validated against ledger.schema.json.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import re
import sys
from pathlib import Path
from typing import Any, Iterable, Optional

HERE = Path(__file__).resolve().parent
SCHEMA = HERE / "ledger.schema.json"
LIST_FIELDS = {"authors", "modules", "open_questions", "tags"}


def parse_markdown_table(text: str) -> list[dict[str, str]]:
    rows = [line.strip() for line in text.splitlines() if line.strip().startswith("|")]
    if len(rows) < 2:
        return []
    def cells(line: str) -> list[str]:
        return [c.strip() for c in line.strip().strip("|").split("|")]
    header = [h.lower() for h in cells(rows[0])]
    body = [r for r in rows[1:] if not re.fullmatch(r"\|?[\s:\-|]+\|?", r)]
    return [dict(zip(header, cells(r))) for r in body]


def parse_csv(text: str) -> list[dict[str, str]]:
    reader = csv.DictReader(io.StringIO(text))
    return [{(k or "").strip().lower(): (v or "").strip() for k, v in row.items()} for row in reader]


def read_rows(path: Path) -> list[dict[str, str]]:
    text = path.read_text(encoding="utf-8")
    if path.suffix.lower() in {".md", ".markdown"}:
        return parse_markdown_table(text)
    return parse_csv(text)


def _split(value: str) -> list[str]:
    return [v.strip() for v in value.split(";") if v.strip()]


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")[:64] or "entry"


def row_to_entry(row: dict[str, str]) -> tuple[dict[str, Any], list[str]]:
    """Convert one row; returns (entry, warnings)."""
    warnings: list[str] = []
    get = lambda k: (row.get(k) or "").strip()  # noqa: E731
    direction = get("direction") or get("title")
    if not direction:
        raise ValueError("row needs a direction or a title")
    entry: dict[str, Any] = {
        "id": slug(get("id") or direction),
        "direction": direction,
        "summary": get("summary") or "summary not yet written",
        "relevance": get("relevance") or "relevance not yet assessed",
        "awen_mapping": {"state": get("awen_state") or "not-started"},
        "maturity": get("maturity") or "theory",
    }
    if get("modules"):
        entry["awen_mapping"]["modules"] = _split(get("modules"))
    if get("next_step"):
        entry["awen_mapping"]["next_step"] = get("next_step")
    for key in ("hardware_needed",):
        if get(key):
            entry[key] = get(key)
    for key in ("open_questions", "tags"):
        if get(key):
            entry[key] = _split(get(key))
    citation: dict[str, Any] = {}
    if get("title"):
        citation["title"] = get("title")
    if get("authors"):
        citation["authors"] = _split(get("authors"))
    if get("year"):
        try:
            citation["year"] = int(get("year"))
        except ValueError:
            warnings.append(f"{entry['id']}: year {get('year')!r} is not an integer; dropped")
    if get("venue"):
        citation["venue"] = get("venue")
    doi = re.sub(r"^https?://(dx\.)?doi\.org/", "", get("doi"))
    if doi:
        citation["doi"] = doi
    if get("url"):
        if get("url").startswith("https://"):
            citation["url"] = get("url")
        else:
            warnings.append(f"{entry['id']}: non-https URL dropped")
    complete = all(k in citation for k in ("title", "authors", "year")) and ("doi" in citation or "url" in citation)
    if complete:
        entry["citation"] = citation
    elif citation:
        warnings.append(f"{entry['id']}: incomplete citation kept out of the entry (needs title, authors, year, doi or url)")
    verified = (get("status").lower() == "verified" and complete
                and get("verification_method") and re.fullmatch(r"\d{4}-\d{2}-\d{2}", get("verification_date")))
    if get("status").lower() == "verified" and not verified:
        warnings.append(f"{entry['id']}: marked verified without a complete, checked citation; imported as 'citation pending'")
    entry["status"] = "verified" if verified else "citation pending"
    if verified:
        entry["verification"] = {"method": get("verification_method"), "date": get("verification_date")}
    return entry, warnings


def validate(ledger: dict[str, Any]) -> None:
    from jsonschema import Draft202012Validator

    Draft202012Validator(json.loads(SCHEMA.read_text())).validate(ledger)
    ids = [e["id"] for e in ledger["entries"]]
    dupes = sorted({i for i in ids if ids.count(i) > 1})
    if dupes:
        raise ValueError(f"duplicate ledger ids: {', '.join(dupes)}")


def merge(ledger: dict[str, Any], entries: Iterable[dict[str, Any]], replace: bool = False) -> tuple[int, list[str]]:
    by_id = {e["id"]: i for i, e in enumerate(ledger["entries"])}
    added, notes = 0, []
    for entry in entries:
        if entry["id"] in by_id:
            if replace:
                ledger["entries"][by_id[entry["id"]]] = entry
                notes.append(f"{entry['id']}: replaced")
            else:
                notes.append(f"{entry['id']}: exists; skipped (use --replace)")
            continue
        by_id[entry["id"]] = len(ledger["entries"])
        ledger["entries"].append(entry)
        added += 1
    return added, notes


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path)
    parser.add_argument("--ledger", type=Path, default=HERE / "ledger.json")
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    ledger = json.loads(args.ledger.read_text()) if args.ledger.exists() else {
        "version": "awen.research-ledger.v1", "entries": []}
    converted, warnings = [], []
    for row in read_rows(args.source):
        entry, w = row_to_entry(row)
        converted.append(entry)
        warnings.extend(w)
    added, notes = merge(ledger, converted, args.replace)
    validate(ledger)
    for line in warnings + notes:
        print("note:", line, file=sys.stderr)
    pending = sum(e["status"] == "citation pending" for e in converted)
    print(f"rows {len(converted)}, added {added}, citation pending {pending}")
    if not args.dry_run:
        args.ledger.write_text(json.dumps(ledger, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
