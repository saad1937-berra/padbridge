#!/usr/bin/env python3
"""Audit candidate Xbox providers without downloading, executing or installing.

This verifier intentionally recognizes ONLY the three still-unapproved
candidates reviewed on 2026-10-22. Adding a vendor or approving one requires
a deliberate code-review update of this gate, technical evidence and legal
review. Passing this script does NOT authorize Windows driver installation.
"""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "policy" / "xinput-provider-review.json"
POLICY = ROOT / "policy" / "public-release.json"
BACKEND = ROOT / "src-tauri" / "src" / "virtual_pad" / "selected_backend.rs"

EXPECTED_CANDIDATES = {"hidmaestro", "libvirtualhid", "padbridge-vhf"}
EXPECTED_FIELDS = {
    "id", "name", "repository", "upstream_commit", "source_license",
    "xinput_claim", "installation", "trust_root_mutation", "requires_paid_license",
    "redistribution_review_complete", "trusted_production_publisher_signature",
    "independent_xinput_test", "safe_uninstall_and_coexistence_test",
    "release_approved", "links",
}
MUST_REMAIN_TRUE = {
    "price_zero", "signup_not_required", "no_local_trust_root_installation",
    "no_user_paid_provider_license", "externally_verified_xinput",
    "documented_redistribution_rights", "signed_production_driver",
    "safe_uninstall_and_coexistence", "isolated_windows_test",
}


class Rejected(ValueError):
    pass


def require(ok: bool, message: str) -> None:
    if not ok:
        raise Rejected(message)


def check(review: object, release: object, selected_backend: str) -> None:
    require(type(review) is dict and type(release) is dict, "Both inputs must be JSON objects")
    require(type(review.get("schema_version")) is int
            and review["schema_version"] == 1, "Unknown provider review schema")
    require(set(review) == {"schema_version", "reviewed_on", "selected_provider",
                             "public_app_requires", "candidates"}, "Review has unexpected keys")
    require(review["reviewed_on"] == "2026-10-22", "Review is pinned; re-audit before updating")
    require(review["selected_provider"] == "none",
            "Cannot select any unverified Xbox provider")
    require(release.get("distribution") == "free_public"
            and type(release.get("price_usd")) is int
            and release["price_usd"] == 0, "PadBridge must remain free for everyone")
    require(release.get("public_release_allowed") is False,
            "Provider review does not authorize a public release")
    require(release.get("verified_xinput_backend") is False
            and release.get("xinput_backend_selected") == "none",
            "Project policy and audited provider selection disagree")
    require(release.get("backend_driver_bundled") is False
            and release.get("automatic_driver_install") is False,
            "Driver must not be bundled or installed automatically")
    require(release.get("license_key_required") is False
            and release.get("user_account_required") is False,
            "Paid license / account cannot become a mandatory condition")

    public = review["public_app_requires"]
    require(type(public) is dict and set(public) == MUST_REMAIN_TRUE,
            "Public acceptance criteria must remain complete")
    require(all(public[key] is True for key in MUST_REMAIN_TRUE),
            "Do not weaken public driver acceptance criteria")

    candidates = review["candidates"]
    require(type(candidates) is list and len(candidates) == len(EXPECTED_CANDIDATES),
            "Adding or removing a provider requires updating the reviewed gate")
    seen = set()
    for item in candidates:
        require(type(item) is dict and set(item) == EXPECTED_FIELDS,
                "Provider candidate fields changed")
        name = item["id"]
        require(type(name) is str and name in EXPECTED_CANDIDATES and name not in seen,
                "Unknown/duplicate provider; manual review required")
        seen.add(name)
        for field in ("name", "repository", "source_license", "xinput_claim", "installation"):
            require(type(item[field]) is str and len(item[field].strip()) > 3,
                    f"{name}: missing {field}")
        require(item["repository"].startswith("https://github.com/"),
                f"{name}: repository must be public GitHub URL")
        if item["upstream_commit"] is not None:
            sha = item["upstream_commit"]
            require(type(sha) is str and len(sha) == 40
                    and all(c in "0123456789abcdef" for c in sha),
                    f"{name}: upstream SHA invalid")
        require(type(item["links"]) is list and item["links"]
                and all(type(url) is str and url.startswith("https://") for url in item["links"]),
                f"{name}: missing source evidence links")
        for field in ("trust_root_mutation", "requires_paid_license",
                      "redistribution_review_complete", "trusted_production_publisher_signature",
                      "independent_xinput_test", "safe_uninstall_and_coexistence_test",
                      "release_approved"):
            require(type(item[field]) is bool, f"{name}: {field} not boolean")
        require(item["release_approved"] is False,
                f"{name}: cannot approve without a new design and evidence review")
        require(not (item["redistribution_review_complete"]
                     and item["trusted_production_publisher_signature"]
                     and item["independent_xinput_test"]
                     and item["safe_uninstall_and_coexistence_test"]),
                f"{name}: claimed fully validated despite frozen unapproved manifest")

    require(seen == EXPECTED_CANDIDATES, "One or more candidates missing")
    mapping = {c["id"]: c for c in candidates}
    require(mapping["hidmaestro"]["trust_root_mutation"] is True,
            "HIDMaestro local machine root certificate risk cannot be hidden")
    require(mapping["hidmaestro"]["trusted_production_publisher_signature"] is False,
            "Self-signed installation is not verified production publisher signing")
    require(mapping["libvirtualhid"]["requires_paid_license"] is True,
            "LizardByte per-machine entitlement cannot be omitted")
    require(mapping["padbridge-vhf"]["independent_xinput_test"] is False,
            "Generic HID cannot be declared real XInput")

    # This deliberately conservative guard detects a wiring regression in
    # the single selector. It is NOT a Rust parser or security proof.
    require("Box::new(NullBackend)" in selected_backend,
            "Provider selector no longer explicitly uses NullBackend")
    require("Box::new(XboxAdapter" not in selected_backend,
            "Provider selector must not enable the dormant adapter")


def self_test(review: dict, release: dict, selector: str) -> None:
    check(review, release, selector)
    unsafe_changes = [
        ("approve provider", lambda r, p: r["candidates"][0].update(release_approved=True)),
        ("hide root trust mutation",
         lambda r, p: r["candidates"][0].update(trust_root_mutation=False)),
        ("mark local signing production",
         lambda r, p: r["candidates"][0].update(trusted_production_publisher_signature=True)),
        ("remove paid license restriction",
         lambda r, p: r["candidates"][1].update(requires_paid_license=False)),
        ("claim VHF XInput verified",
         lambda r, p: r["candidates"][2].update(independent_xinput_test=True)),
        ("select unapproved provider",
         lambda r, p: r.update(selected_provider="hidmaestro")),
        ("enable automatic install",
         lambda r, p: p.update(automatic_driver_install=True)),
        ("enable public release",
         lambda r, p: p.update(public_release_allowed=True)),
        ("charge for PadBridge",
         lambda r, p: p.update(price_usd=5)),
        ("weaken signature requirement",
         lambda r, p: r["public_app_requires"].update(signed_production_driver=False)),
    ]
    for label, change in unsafe_changes:
        fake_review, fake_policy = copy.deepcopy(review), copy.deepcopy(release)
        change(fake_review, fake_policy)
        try:
            check(fake_review, fake_policy, selector)
        except Rejected:
            continue
        raise AssertionError("Unsafe policy accepted: " + label)
    try:
        check(review, release, "Box::new(XboxAdapter::new(unverified))")
    except Rejected:
        pass
    else:
        raise AssertionError("Unverified backend selector was accepted")
    print("XInput provider admission self-test PASS (11 unsafe changes rejected)")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--review", type=Path, default=MANIFEST)
    ap.add_argument("--policy", type=Path, default=POLICY)
    ap.add_argument("--selected-backend", type=Path, default=BACKEND)
    args = ap.parse_args()
    try:
        review = json.loads(args.review.read_text(encoding="utf-8"))
        release = json.loads(args.policy.read_text(encoding="utf-8"))
        selector = args.selected_backend.read_text(encoding="utf-8")
        check(review, release, selector)
        if args.self_test:
            self_test(review, release, selector)
    except (OSError, UnicodeError, ValueError, AssertionError) as exc:
        print("XInput provider admission: FAIL: " + str(exc), file=sys.stderr)
        return 1
    print("XInput provider admission: PASS (no external provider approved or installed)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
