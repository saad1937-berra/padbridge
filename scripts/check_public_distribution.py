#!/usr/bin/env python3
"""Verify PadBridge's public-free release policy without publishing anything.

Audit mode validates the declared policy and blocks obvious regression.
--release mode is deliberately stricter: it FAILS until release blockers
are independently verified. This is NOT a software license audit or signer.
"""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_POLICY = ROOT / "policy" / "public-release.json"

ALLOWED_KEYS = {
    "schema_version", "product", "distribution", "price_usd",
    "user_account_required", "license_key_required", "optional_donation_allowed",
    "app_source_license", "release_status", "public_release_allowed",
    "verified_xinput_backend", "xinput_backend_selected",
    "backend_driver_bundled", "automatic_driver_install",
    "bundled_third_party_driver_packages", "driver_redistribution_rights_verified",
    "windows_driver_installation_verified", "release_blockers",
}
REQUIRED_FALSE = (
    "user_account_required", "license_key_required",
    "backend_driver_bundled", "automatic_driver_install",
)


class PolicyError(ValueError):
    pass


def expect(test: bool, reason: str) -> None:
    if not test:
        raise PolicyError(reason)


def validate(policy: object, *, for_release: bool = False) -> None:
    expect(isinstance(policy, dict), "Policy must be a JSON object")
    expect(set(policy) == ALLOWED_KEYS, "Unexpected, missing or changed policy fields")
    expect(type(policy["schema_version"]) is int and policy["schema_version"] == 1,
           "Unsupported release policy schema")
    expect(policy["product"] == "PadBridge", "Wrong product")
    expect(policy["distribution"] == "free_public", "PadBridge must remain publicly free")
    expect(type(policy["price_usd"]) is int and policy["price_usd"] == 0,
           "Paid releases are prohibited by product decision")
    for key in REQUIRED_FALSE:
        expect(policy[key] is False, key + " cannot become enabled")
    expect(type(policy["optional_donation_allowed"]) is bool,
           "Donations flag must be a Boolean")
    expect(type(policy["verified_xinput_backend"]) is bool, "Backend validation flag invalid")
    for key in ("public_release_allowed", "driver_redistribution_rights_verified",
                "windows_driver_installation_verified"):
        expect(type(policy[key]) is bool, key + " must be a Boolean")
    expect(isinstance(policy["app_source_license"], str)
           and policy["app_source_license"].strip(),
           "Application source-license decision must be explicit")
    expect(isinstance(policy["release_status"], str) and policy["release_status"] in
           ("blocked_pending_verification", "release_reviewed"),
           "Unknown release-status value")
    expect(isinstance(policy["xinput_backend_selected"], str)
           and policy["xinput_backend_selected"].strip(),
           "XInput backend selection must be explicit")
    packages = policy["bundled_third_party_driver_packages"]
    expect(isinstance(packages, list)
           and all(isinstance(x, str) and x.strip() for x in packages),
           "Bundled packages must be a list of distinct nonempty names")
    expect(not packages, "Bundling third-party driver packages requires a separate policy review")
    blockers = policy["release_blockers"]
    expect(isinstance(blockers, list)
           and all(isinstance(x, str) and x.strip() for x in blockers),
           "Release blockers must be documented")
    expect(len(blockers) == len(set(blockers)), "Duplicate release blockers")

    if not policy["verified_xinput_backend"]:
        expect(policy["xinput_backend_selected"] == "none",
               "Unverified XInput backend must not be selected")
    if (policy["release_status"] != "release_reviewed"
            or policy["app_source_license"] == "undecided"
            or not policy["verified_xinput_backend"]
            or not policy["driver_redistribution_rights_verified"]
            or not policy["windows_driver_installation_verified"]
            or blockers):
        expect(policy["public_release_allowed"] is False,
               "Cannot approve public release with unresolved verification")

    if for_release:
        expect(policy["release_status"] == "release_reviewed",
               "Public release is still blocked pending review")
        expect(policy["app_source_license"] != "undecided",
               "Choose and verify application source/distribution license")
        expect(policy["verified_xinput_backend"],
               "Xbox/XInput backend not verified")
        expect(policy["driver_redistribution_rights_verified"],
               "Driver redistribution rights have not been verified")
        expect(policy["windows_driver_installation_verified"],
               "Driver installation and removal not validated")
        expect(not blockers, "Outstanding release blockers remain")
        expect(policy["public_release_allowed"], "Release permission is not approved")


def self_test(original: dict) -> None:
    validate(original)
    for key, altered in [
        ("price_usd", 9),
        ("price_usd", True),
        ("user_account_required", True),
        ("license_key_required", True),
        ("backend_driver_bundled", True),
        ("automatic_driver_install", True),
        ("bundled_third_party_driver_packages", ["external-driver.msi"]),
        ("public_release_allowed", True),
        ("xinput_backend_selected", "unverified_proprietary_driver"),
    ]:
        changed = copy.deepcopy(original)
        changed[key] = altered
        try:
            validate(changed)
        except PolicyError:
            pass
        else:
            raise AssertionError("Unsafe policy mutation accepted: " + key)
    try:
        validate(original, for_release=True)
    except PolicyError:
        pass
    else:
        raise AssertionError("Incomplete XInput product was incorrectly releasable")
    print("PadBridge free-public policy: PASS (nine unsafe mutations rejected)")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--release", action="store_true",
                        help="fail until release rights and technical acceptance are proven")
    args = parser.parse_args()
    try:
        p = json.loads(args.policy.read_text(encoding="utf-8"))
        validate(p, for_release=args.release)
        if args.self_test:
            self_test(p)
        print("PadBridge public-free policy audit: PASS"
              + (" (public release reviewed)" if args.release else " (no publication authorized)"))
        return 0
    except (OSError, UnicodeError, ValueError, AssertionError) as exc:
        print("PadBridge public-free policy audit: FAIL: " + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
