#!/usr/bin/env python3
"""Compare three READ-ONLY XInput snapshots gathered inside a disposable VM.

Requires outputs from padbridge-xinput-probe --snapshot. A slot appearing and
responding is only a *candidate*: XInput provides no trustworthy PadBridge
device identity and these snapshots cannot prove provenance.
This script never queries devices, starts services or installs drivers.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

FIELDS = {
    "packetNumber": (0, 0xFFFFFFFF),
    "buttons": (0, 0xFFFF),
    "leftTrigger": (0, 255),
    "rightTrigger": (0, 255),
    "thumbLx": (-32768, 32767),
    "thumbLy": (-32768, 32767),
    "thumbRx": (-32768, 32767),
    "thumbRy": (-32768, 32767),
}
CONTROL_FIELDS = set(FIELDS) - {"packetNumber"}


class EvidenceError(ValueError):
    pass


def validate(value: object) -> dict[int, dict]:
    if not isinstance(value, list) or len(value) != 4:
        raise EvidenceError("XInput snapshot must contain exactly four slots")
    result: dict[int, dict] = {}
    for item in value:
        if not isinstance(item, dict) or type(item.get("slot")) is not int:
            raise EvidenceError("Invalid or missing XInput slot index")
        slot = item["slot"]
        if slot not in range(4) or slot in result:
            raise EvidenceError("Slot must be unique and within 0..3")
        status = item.get("status")
        if status == "connected":
            if set(item) != {"slot", "status", "gamepad"}:
                raise EvidenceError("Unexpected connected slot fields")
            pad = item["gamepad"]
            if not isinstance(pad, dict) or set(pad) != set(FIELDS):
                raise EvidenceError("Connected slot must contain only the known gamepad fields")
            for field, (minimum, maximum) in FIELDS.items():
                number = pad[field]
                if type(number) is not int or not minimum <= number <= maximum:
                    raise EvidenceError("Invalid XInput field: " + field)
        elif status == "disconnected":
            if set(item) != {"slot", "status"}:
                raise EvidenceError("Disconnected slot must not contain gamepad data")
        elif status == "error":
            if (set(item) != {"slot", "status", "win32_code"}
                    or type(item["win32_code"]) is not int
                    or not 0 <= item["win32_code"] <= 0xFFFFFFFF):
                raise EvidenceError("Error slot must include a valid win32_code")
        else:
            raise EvidenceError("Unknown slot status")
        result[slot] = item
    return result


def compare(before: object, after_idle: object, after_activity: object) -> dict:
    b = validate(before)
    a = validate(after_idle)
    t = validate(after_activity)
    if any(item["status"] == "error" for group in (b, a, t) for item in group.values()):
        return {
            "result": "inconclusive_xinput_error",
            "candidateSlots": [],
            "controlChanges": [],
            "attribution": "unverified",
        }

    candidates = [
        slot for slot in range(4)
        if b[slot]["status"] == "disconnected"
        and a[slot]["status"] == "connected"
    ]
    changed = []
    for slot in candidates:
        if t[slot]["status"] != "connected":
            continue
        baseline_controls = a[slot]["gamepad"]
        active_controls = t[slot]["gamepad"]
        fields = sorted(field for field in CONTROL_FIELDS
                        if baseline_controls[field] != active_controls[field])
        if fields:
            changed.append({"slot": slot, "fields": fields})

    if not candidates:
        verdict = "no_new_xinput_slot"
    elif changed:
        verdict = "candidate_input_change_unattributed"
    else:
        verdict = "candidate_without_input_change"

    return {
        "result": verdict,
        "candidateSlots": candidates,
        "controlChanges": changed,
        "attribution": "unverified",
    }


def sample(connected: tuple[int, ...] = (), buttons: int = 0) -> list[dict]:
    """Small synthetic fixture only for self-tests."""
    pad = {name: 0 for name in FIELDS}
    pad["buttons"] = buttons
    return [
        {"slot": i, "status": "connected", "gamepad": dict(pad)}
        if i in connected else {"slot": i, "status": "disconnected"}
        for i in range(4)
    ]


def self_test() -> None:
    baseline = sample()
    connected_idle = sample((0,))
    moved = sample((0,), buttons=1)
    found = compare(baseline, connected_idle, moved)
    assert found["result"] == "candidate_input_change_unattributed"
    assert found["controlChanges"] == [{"slot": 0, "fields": ["buttons"]}]
    assert found["attribution"] == "unverified"
    assert compare(baseline, connected_idle, connected_idle)["result"] == "candidate_without_input_change"
    assert compare(baseline, baseline, baseline)["result"] == "no_new_xinput_slot"
    assert compare(sample((0,)), connected_idle, moved)["result"] == "no_new_xinput_slot"
    err = sample()
    err[0] = {"slot": 0, "status": "error", "win32_code": 5}
    assert compare(err, connected_idle, moved)["result"] == "inconclusive_xinput_error"
    for corrupted in (
        baseline[:3],
        baseline + [baseline[0]],
        [dict(item, extra="id") if item["slot"] == 0 else item for item in baseline],
        [{"slot": 0, "status": "connected", "gamepad": {"buttons": 1}}] + baseline[1:],
    ):
        try:
            validate(corrupted)
        except EvidenceError:
            pass
        else:
            raise AssertionError("Malformed or extra data accepted")
    print("XInput three-snapshot comparison self-tests: PASS")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--before", type=Path)
    parser.add_argument("--after-idle", type=Path)
    parser.add_argument("--after-activity", type=Path)
    args = parser.parse_args()

    if args.self_test:
        self_test()
        if all(p is None for p in (args.before, args.after_idle, args.after_activity)):
            return 0

    if any(p is None for p in (args.before, args.after_idle, args.after_activity)):
        parser.error("supply --before, --after-idle and --after-activity together")
    try:
        sources = [json.loads(path.read_text(encoding="utf-8-sig"))
                   for path in (args.before, args.after_idle, args.after_activity)]
        result = compare(*sources)
    except (ValueError, OSError, UnicodeError) as exc:
        parser.exit(2, "XInput evidence rejected: " + str(exc) + "\n")
    print(json.dumps(result, indent=2, sort_keys=True))
    # A candidate is not provenance proof. All valid comparisons exit 0,
    # never implying an installed PadBridge XInput device.
    return 0


if __name__ == "__main__":
    sys.exit(main())
