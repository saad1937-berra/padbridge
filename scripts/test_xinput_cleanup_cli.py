#!/usr/bin/env python3
"""End-to-end CLI tests for offline XInput cleanup evidence; no drivers or devices."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "check_xinput_cleanup.py"


def snapshot(connected=(), buttons=0):
    pad = {
        "packetNumber": 0, "buttons": buttons, "leftTrigger": 0,
        "rightTrigger": 0, "thumbLx": 0, "thumbLy": 0,
        "thumbRx": 0, "thumbRy": 0,
    }
    return [
        {"slot": index, "status": "connected", "gamepad": dict(pad)}
        if index in connected else {"slot": index, "status": "disconnected"}
        for index in range(4)
    ]


class CleanupCliTests(unittest.TestCase):
    def run_cli(self, snapshots):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            flags = []
            for label, value in zip(
                ("before", "after-idle", "after-activity", "after-close"), snapshots
            ):
                file = root / (label + ".json")
                file.write_text(json.dumps(value), encoding="utf-8")
                flags.extend(("--" + label, str(file)))
            return subprocess.run(
                [sys.executable, str(SCRIPT), *flags],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )

    def test_disconnected_candidate_remains_unattributed(self):
        result = self.run_cli((
            snapshot(), snapshot((0,)), snapshot((0,), 1), snapshot()
        ))
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["result"], "candidate_disappeared_unattributed")
        self.assertEqual(report["candidateSlots"], [0])
        self.assertEqual(report["stillConnected"], [])
        self.assertEqual(report["attribution"], "unverified")
        self.assertIs(report["cleanup_proven"], False)

    def test_stuck_slot_does_not_claim_cleanup(self):
        result = self.run_cli((
            snapshot(), snapshot((0,)), snapshot((0,), 1), snapshot((0,))
        ))
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["result"], "candidate_still_connected")
        self.assertEqual(report["stillConnected"], [0])
        self.assertIs(report["cleanup_proven"], False)

    def test_malformed_evidence_is_rejected(self):
        malformed = snapshot()
        malformed[0]["serialNumber"] = "should-not-be-collected"
        result = self.run_cli((
            malformed, snapshot((0,)), snapshot((0,), 1), snapshot()
        ))
        self.assertEqual(result.returncode, 2)
        self.assertIn("evidence rejected", result.stderr.lower())

    def test_error_slot_stays_inconclusive(self):
        last = snapshot()
        last[0] = {"slot": 0, "status": "error", "win32_code": 5}
        result = self.run_cli((
            snapshot(), snapshot((0,)), snapshot((0,), 1), last
        ))
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["result"], "inconclusive_xinput_error")
        self.assertIs(report["cleanup_proven"], False)

    def test_missing_file_fails(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT),
             "--before", "nonexistent.json",
             "--after-idle", "nonexistent.json",
             "--after-activity", "nonexistent.json",
             "--after-close", "nonexistent.json"],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("evidence rejected", result.stderr.lower())


if __name__ == "__main__":
    unittest.main()
