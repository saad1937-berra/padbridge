#!/usr/bin/env python3
"""Static creator-credit regression check (no app launch, network or release)."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def check(root: Path = ROOT) -> None:
    import json
    import tomllib

    branding = (root / "src/lib/branding.ts").read_text(encoding="utf-8")
    app = (root / "src/App.tsx").read_text(encoding="utf-8")
    settings = (root / "src/screens/Settings.tsx").read_text(encoding="utf-8")
    readme = (root / "README.md").read_text(encoding="utf-8")
    policy = json.loads((root / "policy/public-release.json").read_text(encoding="utf-8"))
    npm = json.loads((root / "package.json").read_text(encoding="utf-8"))
    cargo = tomllib.loads((root / "src-tauri/Cargo.toml").read_text(encoding="utf-8"))

    assert '"PadBridge by Saad Berra"' in branding, "Missing public byline"
    assert '"Saad Berra"' in branding, "Missing creator name"
    assert '"saad1937-berra"' in branding, "Missing creator GitHub"
    assert "PADBRIDGE_BRAND.creator" in app, "Missing visible sidebar attribution"
    assert "PADBRIDGE_BRAND.byline" in settings, "Missing About-page attribution"
    assert "PADBRIDGE_BRAND.projectStatus" in settings, "Missing honest status"
    assert "PadBridge by Saad Berra" in readme, "README lacks author credit"
    assert npm["author"] == "Saad Berra", "npm author metadata differs"
    assert cargo["package"]["authors"] == ["Saad Berra"], "Cargo author differs"
    assert policy["distribution"] == "free_public" and policy["price_usd"] == 0, \
        "Branding must not change public-free pricing"
    assert policy["public_release_allowed"] is False, "Premature public release"
    assert not policy["verified_xinput_backend"], "Do not claim XInput verification"
    assert not policy["license_key_required"], "Branding must not add paid activation"


if __name__ == "__main__":
    check()
    print("PadBridge creator credits and free-public status: PASS")
