#!/usr/bin/env python3
"""Read-only four-snapshot XInput release check for an isolated Windows VM.

No Windows APIs or drivers are touched. Only compares previously saved JSON.
Even a disappearing slot does NOT establish device provenance or safe uninstall.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from compare_xinput_snapshots import EvidenceError, compare, sample, validate


def assess(before: object, idle: object, active: object, closed: object) -> dict:
    baseline = compare(before, idle, active)
    end = validate(closed)
    if baseline["result"] == "inconclusive_xinput_error" or any(
        item["status"] == "error" for item in end.values()
    ):
        return {
            "result": "inconclusive_xinput_error",
            "candidateSlots": [],
            "stillConnected": [],
            "attribution": "unverified",
            "cleanup_proven": False,
        }

    candidates = baseline["candidateSlots"]
    still = [slot for slot in candidates if end[slot]["status"] != "disconnected"]

    if not candidates:
        verdict = "no_new_xinput_slot"
    elif still:
        verdict = "candidate_still_connected"
    elif baseline["result"] == "candidate_input_change_unattributed":
        verdict = "candidate_disappeared_unattributed"
    else:
        verdict = "candidate_disappeared_without_input_evidence"

    return {
        "result": verdict,
        "candidateSlots": candidates,
        "stillConnected": still,
        "attribution": "unverified",
        "cleanup_proven": False,
        "note": "No PnP provenance or driver removal can be established from XInput snapshots alone.",
    }


def self_test() -> None:
    before = sample()
    idle = sample((0,))
    active = sample((0,), buttons=1)
    good = assess(before, idle, active, before)
    assert good["result"] == "candidate_disappeared_unattributed"
    assert good["attribution"] == "unverified" and good["cleanup_proven"] is False
    assert assess(before, idle, active, active)["result"] == "candidate_still_connected"
    assert assess(before, idle, idle, before)["result"] == "candidate_disappeared_without_input_evidence"
    assert assess(before, before, before, before)["result"] == "no_new_xinput_slot"
    err = sample()
    err[0] = {"slot": 0, "status": "error", "win32_code": 5}
    assert assess(before, idle, active, err)["result"] == "inconclusive_xinput_error"
    try:
        assess(before, idle, active, [{"slot": 0, "status": "disconnected"}])
    except EvidenceError:
        pass
    else:
        raise AssertionError("Incomplete after-close evidence was accepted")
    print("XInput cleanup snapshot self-test PASS (6 scenarios)")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    for arg in ("before", "after-idle", "after-activity", "after-close"):
        parser.add_argument("--" + arg, type=Path)
    args = parser.parse_args()
    if args.self_test:
        self_test()
        if not any(getattr(args, key.replace("-", "_")) for key in (
            "before", "after-idle", "after-activity", "after-close"
        )):
            return 0
    paths = [args.before, args.after_idle, args.after_activity, args.after_close]
    if any(path is None for path in paths):
        parser.error("supply all four snapshot paths together")
    try:
        sources = [json.loads(path.read_text(encoding="utf-8-sig")) for path in paths]
        result = assess(*sources)
    except (OSError, ValueError, UnicodeError) as exc:
        parser.exit(2, f"XInput cleanup evidence rejected: {exc}\n")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
