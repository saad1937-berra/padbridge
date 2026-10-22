#!/usr/bin/env python3
"""Fail-closed static check for the non-public NSIS diagnostic preview.

Does not create installers, run PowerShell, trust certificates or open devices.
It confirms the diagnostic flavor is isolated, has no driver sidecars and
cannot be confused with a public/XInput-enabled release. The built installer
must additionally be scanned for driver package files in Windows CI.
"""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


class PreviewError(ValueError):
    pass


def require(test: bool, message: str) -> None:
    if not test:
        raise PreviewError(message)


def verify(base: object, preview: object, policy: object, backend: str) -> None:
    require(isinstance(base, dict) and isinstance(preview, dict)
            and isinstance(policy, dict), "Expected JSON objects")
    require(base.get("productName") == "PadBridge", "Unexpected base product")
    require(preview.get("productName") == "PadBridge Diagnostic Preview",
            "Diagnostic package must have a distinct product name")
    require(preview.get("identifier") == "app.padbridge.diagnosticpreview"
            and preview["identifier"] != base.get("identifier"),
            "Diagnostic application identity must be distinct")
    require(set(preview) == {"$schema", "productName", "identifier", "app"},
            "Preview cannot override bundle, security or build steps")
    require(set(preview["app"]) == {"windows"}, "Preview cannot override app capabilities")
    windows = preview["app"]["windows"]
    require(isinstance(windows, list) and len(windows) == 1,
            "Exactly one diagnostic window is allowed")
    require(windows[0].get("label") == "main"
            and "Diagnostic Preview" in windows[0].get("title", "")
            and "sans Xbox/XInput" in windows[0]["title"],
            "Preview window must clearly identify disabled Xbox emulation")

    base_bundle = base.get("bundle", {})
    require(base_bundle.get("active") is True and base_bundle.get("targets") == ["nsis"],
            "Installer target must remain NSIS only")
    require(base_bundle.get("windows", {}).get("nsis", {}).get("installMode") == "currentUser",
            "This installer must never require per-machine deployment")
    require("resources" not in base_bundle and "externalBin" not in base_bundle,
            "Cannot silently bundle drivers, helper services or sidecars")
    require("installerHooks" not in base_bundle.get("windows", {}).get("nsis", {}),
            "Custom installer hooks require separate security review")
    require(policy.get("distribution") == "free_public"
            and policy.get("price_usd") == 0
            and policy.get("public_release_allowed") is False
            and policy.get("verified_xinput_backend") is False
            and policy.get("backend_driver_bundled") is False
            and policy.get("automatic_driver_install") is False
            and policy.get("bundled_third_party_driver_packages") == [],
            "Diagnostic build must not claim public release or embed virtual drivers")
    require("Box::new(NullBackend)" in backend, "Native diagnostic requires NullBackend")
    require("Box::new(XboxAdapter" not in backend,
            "Unverified Xbox virtual adapter was selected")


def self_test(base: dict, preview: dict, policy: dict, backend: str) -> None:
    verify(base, preview, policy, backend)
    cases = [
        ("production identity reused",
         lambda b, p, y: p.update(identifier=b["identifier"])),
        ("misleading title",
         lambda b, p, y: p["app"]["windows"][0].update(title="PadBridge — Xbox Ready")),
        ("package a hidden driver",
         lambda b, p, y: b["bundle"].update(resources=["drivers/xinput.sys"])),
        ("add custom hooks",
         lambda b, p, y: b["bundle"]["windows"]["nsis"].update(installerHooks="setup.nsh")),
        ("privileged installation",
         lambda b, p, y: b["bundle"]["windows"]["nsis"].update(installMode="perMachine")),
        ("unreviewed bundle override",
         lambda b, p, y: p.update(bundle={"externalBin": ["privileged.exe"]})),
        ("premature release",
         lambda b, p, y: y.update(public_release_allowed=True)),
        ("driver auto-install",
         lambda b, p, y: y.update(automatic_driver_install=True)),
        ("XInput without verification",
         lambda b, p, y: y.update(verified_xinput_backend=True)),
    ]
    for name, mutate in cases:
        b, p, y = copy.deepcopy(base), copy.deepcopy(preview), copy.deepcopy(policy)
        mutate(b, p, y)
        try:
            verify(b, p, y, backend)
        except PreviewError:
            continue
        raise AssertionError(f"Unsafe preview policy accepted: {name}")
    try:
        verify(base, preview, policy, "Box::new(XboxAdapter::new(driver))")
    except PreviewError:
        pass
    else:
        raise AssertionError("Unsafe Xbox backend selected")
    print("Diagnostic NSIS policy: PASS (10 regressions rejected)")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    try:
        base = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
        preview = json.loads((ROOT / "src-tauri/tauri.diagnostic.conf.json").read_text(encoding="utf-8"))
        policy = json.loads((ROOT / "policy/public-release.json").read_text(encoding="utf-8"))
        backend = (ROOT / "src-tauri/src/virtual_pad/selected_backend.rs").read_text(encoding="utf-8")
        verify(base, preview, policy, backend)
        if args.self_test:
            self_test(base, preview, policy, backend)
        print("PadBridge diagnostic installer gate: PASS (no public release authorized)")
        return 0
    except (OSError, UnicodeError, ValueError, AssertionError) as error:
        print(f"PadBridge diagnostic installer gate: FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
