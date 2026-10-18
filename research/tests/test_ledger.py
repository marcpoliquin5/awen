"""Research ledger: schema, empty index, agent-sourced reference list and the import script."""

import importlib.util
import json
from pathlib import Path

import pytest

jsonschema = pytest.importorskip("jsonschema")

LEDGER_DIR = Path(__file__).resolve().parents[1] / "ledger"
spec = importlib.util.spec_from_file_location("import_ledger", LEDGER_DIR / "import_ledger.py")
imp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(imp)


REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "agent-sourced.json"


def load():
    return json.loads(REFERENCE.read_text())


def test_ledger_index_is_empty_until_marc_exports_it():
    index = json.loads((LEDGER_DIR / "ledger.json").read_text())
    imp.validate(index)
    assert index["entries"] == []


def test_agent_sourced_reference_is_valid_labeled_and_has_fifteen_verified_directions():
    ledger = load()
    assert ledger["source_note"].startswith("AGENT-SOURCED")
    imp.validate(ledger)
    assert len(ledger["entries"]) == 15
    for entry in ledger["entries"]:
        if entry["status"] == "verified":
            c = entry["citation"]
            assert c["url"] == "https://doi.org/" + c["doi"]
            assert entry["verification"]["date"] == "2026-10-18"


def test_schema_rejects_verified_without_citation():
    bad = {"version": "awen.research-ledger.v1", "entries": [{
        "id": "x", "direction": "d", "summary": "s", "relevance": "r",
        "awen_mapping": {"state": "not-started"}, "maturity": "theory", "status": "verified"}]}
    with pytest.raises(jsonschema.ValidationError):
        imp.validate(bad)


CSV = """id,direction,title,authors,year,venue,doi,summary,status,verification_method,verification_date
good,Direction A,Paper A,A. One; B. Two,2020,Journal,10.1000/abc,Sum,verified,checked against Crossref,2026-10-18
claimed,Direction B,Paper B,C. Three,2021,,10.1000/def,Sum,verified,,
nocite,Direction C,,,,,,Sum,,,
"""

MD = """| id | direction | title | authors | year | url | status |
|---|---|---|---|---|---|---|
| md-one | Direction M | Paper M | D. Four | 2019 | https://example.org/m | citation pending |
"""


def test_csv_import_downgrades_unverified_claims(tmp_path):
    src = tmp_path / "rows.csv"
    src.write_text(CSV)
    rows = imp.read_rows(src)
    entries = [imp.row_to_entry(r)[0] for r in rows]
    by_id = {e["id"]: e for e in entries}
    assert by_id["good"]["status"] == "verified"
    assert by_id["good"]["citation"]["authors"] == ["A. One", "B. Two"]
    assert by_id["claimed"]["status"] == "citation pending"
    assert "citation" not in by_id["nocite"] and by_id["nocite"]["status"] == "citation pending"


def test_markdown_import_and_merge_cli(tmp_path):
    ledger_path = tmp_path / "ledger.json"
    ledger_path.write_text(json.dumps(load()))
    src = tmp_path / "rows.md"
    src.write_text(MD)
    assert imp.main([str(src), "--ledger", str(ledger_path)]) == 0
    merged = json.loads(ledger_path.read_text())
    assert len(merged["entries"]) == 16
    assert merged["entries"][-1]["citation"]["url"] == "https://example.org/m"
    # Re-import does not duplicate or overwrite.
    assert imp.main([str(src), "--ledger", str(ledger_path)]) == 0
    assert len(json.loads(ledger_path.read_text())["entries"]) == 16


def test_dry_run_does_not_write(tmp_path):
    ledger_path = tmp_path / "ledger.json"
    ledger_path.write_text(json.dumps(load()))
    before = ledger_path.read_text()
    src = tmp_path / "rows.csv"
    src.write_text(CSV)
    imp.main([str(src), "--ledger", str(ledger_path), "--dry-run"])
    assert ledger_path.read_text() == before
