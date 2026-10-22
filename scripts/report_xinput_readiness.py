#!/usr/bin/env python3
"""Read-only XInput readiness report. No driver, registry or network access."""

import argparse
import json
import re
import sys
from pathlib import Path


def analyze(public, provider, backend_source):
    """Return an evidence report. A passing report is NOT release approval."""
    violations = []
    blockers = []

    if public.get("public_release_allowed") is not False:
        violations.append("Public release must stay explicitly disabled")
    if public.get("verified_xinput_backend") is not False:
        violations.append("Public policy must not claim a verified backend")
    if public.get("backend_driver_bundled") is not False:
        violations.append("Driver bundling must remain disabled")
    if public.get("automatic_driver_install") is not False:
        violations.append("Automatic driver install must remain disabled")
    if public.get("xinput_backend_selected") != "none":
        violations.append("Public policy selected an unapproved backend")
    if public.get("release_status") != "blocked_pending_verification":
        violations.append("Public release status must remain blocked")
    if provider.get("selected_provider") != "none":
        violations.append("Provider review selected a backend before validation")
    if not re.search(r"Box\s*::\s*new\s*\(\s*NullBackend\s*\)", backend_source):
        violations.append("selected_backend.rs is no longer using NullBackend")

    candidates = provider.get("candidates")
    if not isinstance(candidates, list) or not candidates:
        violations.append("Missing candidate review records")
        candidates = []

    reviewed = []
    for i, candidate in enumerate(candidates):
        if not isinstance(candidate, dict):
            violations.append(f"Candidate {i} is not an object")
            continue
        name = candidate.get("id", f"candidate-{i}")
        if candidate.get("release_approved") is not False:
            violations.append(f"Candidate {name} has invalid release_approved flag")
        evidence = {
            "id": name,
            "release_approved": candidate.get("release_approved") is True,
            "redistribution_review_complete": candidate.get("redistribution_review_complete") is True,
            "trusted_production_publisher_signature": candidate.get("trusted_production_publisher_signature") is True,
            "independent_xinput_test": candidate.get("independent_xinput_test") is True,
            "safe_uninstall_and_coexistence_test": candidate.get("safe_uninstall_and_coexistence_test") is True,
        }
        reviewed.append(evidence)

    blockers.extend(public.get("release_blockers", []) if isinstance(public.get("release_blockers"), list) else [])
    blockers.append("No production XUSB/XInput provider selected and independently validated")
    blockers.append("Real Windows XInput/PnP identity and safe driver lifecycle not demonstrated")

    return {
        "schema_version": 1,
        "static_guardrails_pass": not violations,
        "ready_for_public_xinput": False,
        "backend": "NullBackend" if not any("NullBackend" in v for v in violations) else "unknown",
        "candidate_evidence": reviewed,
        "release_blockers": list(dict.fromkeys(str(b) for b in blockers)),
        "violations": violations,
        "note": "Static policy consistency only; not a driver, XInput or game compatibility test.",
    }


def load_report(root):
    policy_dir = root / "policy"
    public = json.loads((policy_dir / "public-release.json").read_text(encoding="utf-8"))
    provider = json.loads((policy_dir / "xinput-provider-review.json").read_text(encoding="utf-8"))
    selected = (root / "src-tauri/src/virtual_pad/selected_backend.rs").read_text(encoding="utf-8")
    return analyze(public, provider, selected)


def self_test():
    public = {
        "public_release_allowed": False,
        "verified_xinput_backend": False,
        "backend_driver_bundled": False,
        "automatic_driver_install": False,
        "xinput_backend_selected": "none",
        "release_status": "blocked_pending_verification",
        "release_blockers": ["No approved provider"],
    }
    provider = {"selected_provider": "none", "candidates": [{"id": "sample", "release_approved": False}]}
    selected = "pub fn select() { Box::new(NullBackend) }"
    baseline = analyze(public, provider, selected)
    assert baseline["static_guardrails_pass"] and not baseline["ready_for_public_xinput"]
    for field, value in (
        ("public_release_allowed", True),
        ("verified_xinput_backend", True),
        ("backend_driver_bundled", True),
        ("automatic_driver_install", True),
        ("xinput_backend_selected", "example"),
        ("release_status", "released"),
    ):
        changed = dict(public, **{field: value})
        assert not analyze(changed, provider, selected)["static_guardrails_pass"], field
    assert not analyze(public, dict(provider, selected_provider="sample"), selected)["static_guardrails_pass"]
    assert not analyze(public, provider, "Box::new(OtherBackend)")["static_guardrails_pass"]
    assert not analyze(public, {"selected_provider": "none", "candidates": [{"id": "x", "release_approved": True}]}, selected)["static_guardrails_pass"]
    print("XInput readiness self-test PASS (9 unsafe mutations rejected)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="Machine-readable report")
    parser.add_argument("--self-test", action="store_true", help="Run pure-data safety checks")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    try:
        report = load_report(Path(__file__).resolve().parents[1])
    except (OSError, ValueError, TypeError) as exc:
        print(f"XInput readiness report ERROR: {exc}", file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print("XInput static guardrails: " + ("PASS" if report["static_guardrails_pass"] else "FAIL"))
        print("Public XInput release: BLOCKED (no approved provider)")
        print(f"Candidates reviewed: {len(report['candidate_evidence'])}")
        for violation in report["violations"]:
            print(f"VIOLATION: {violation}")
    return 0 if report["static_guardrails_pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
