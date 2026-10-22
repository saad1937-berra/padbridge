#!/usr/bin/env python3
"""Check PadBridge's C KMDF HID descriptor against the Rust VHF report ABI.

Static, standard-library-only CI check: NEVER installs, loads or contacts a driver.
This is a generic HID gamepad contract, NOT an XInput/XUSB implementation.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class ContractError(ValueError):
    pass


def require(condition: bool, description: str) -> None:
    if not condition:
        raise ContractError(description)


def macro_number(header: str, name: str) -> int:
    found = re.search(r"(?m)^#define\s+" + re.escape(name) + r"\s+(0[xX][0-9a-fA-F]+|\d+)[uUlL]*\s*(?://.*)?$", header)
    require(found is not None, "Missing or nonnumeric kernel macro: " + name)
    return int(found.group(1), 0)


def rust_number(source: str, name: str) -> int:
    found = re.search(
        r"(?m)^\s*pub\s+const\s+" + re.escape(name) + r":\s*\w+\s*=\s*(0[xX][0-9a-fA-F_]+|\d[\d_]*)\s*;",
        source,
    )
    require(found is not None, "Missing or changed Rust constant: " + name)
    return int(found.group(1).replace("_", ""), 0)


def remove_comments(source: str) -> str:
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.S)
    return re.sub(r"//[^\n]*", "", source)


def extract_descriptor(source: str) -> bytes:
    match = re.search(r"\bstatic\s+UCHAR\s+PbReportDescriptor\s*\[\s*\]\s*=\s*\{(.*?)\}\s*;", source, re.S)
    require(match is not None, "C HID report descriptor not found")
    value = remove_comments(match.group(1))
    numbers = re.findall(r"0[xX][0-9a-fA-F]+", value)
    require(bool(numbers), "Empty HID report descriptor")
    require(re.fullmatch(r"[\s,]*", re.sub(r"0[xX][0-9a-fA-F]+", "", value)) is not None,
            "Descriptor includes unsupported tokens; inspect the validator")
    fields = [int(num, 16) for num in numbers]
    require(all(0 <= num <= 255 for num in fields), "Descriptor item exceeds a byte")
    return bytes(fields)


def signed(value: int, width: int) -> int:
    mask = 1 << (width * 8 - 1)
    return value - (1 << (width * 8)) if value & mask else value


def parse_hid_descriptor(data: bytes) -> tuple[int, list[dict]]:
    """Minimal HID short-item interpreter; fails on unsupported item formats.

    Input-bit offsets refer to the payload *after* the report-ID byte.
    """
    offset = 0
    depth = 0
    report_ids = set()
    root_gamepad = False
    bit_offset = 0
    global_state = {
        "page": 0,
        "size": None,
        "count": None,
        "minimum": None,
        "maximum": None,
    }
    local_state = {"usages": [], "usage_min": None, "usage_max": None}
    inputs = []
    while offset < len(data):
        prefix = data[offset]
        offset += 1
        require(prefix != 0xFE, "Long HID items are unsupported in this POC")
        width = (0, 1, 2, 4)[prefix & 0x03]
        require(offset + width <= len(data), "Truncated HID item")
        raw = int.from_bytes(data[offset : offset + width], "little")
        offset += width
        kind = (prefix >> 2) & 0x03
        tag = prefix >> 4
        if kind == 1:  # Global
            if tag == 0:
                global_state["page"] = raw
            elif tag == 1:
                global_state["minimum"] = signed(raw, width)
            elif tag == 2:
                lower = global_state["minimum"]
                global_state["maximum"] = signed(raw, width) if lower is not None and lower < 0 else raw
            elif tag == 7:
                global_state["size"] = raw
            elif tag == 8:
                report_ids.add(raw)
            elif tag == 9:
                global_state["count"] = raw
            elif tag in (10, 11):
                raise ContractError("Global PUSH/POP is not supported by this validator")
        elif kind == 2:  # Local
            if tag == 0:
                local_state["usages"].append(raw)
            elif tag == 1:
                local_state["usage_min"] = raw
            elif tag == 2:
                local_state["usage_max"] = raw
        elif kind == 0:  # Main
            if tag == 10:  # Collection
                if depth == 0:
                    root_gamepad = (
                        global_state["page"] == 1
                        and local_state["usages"] == [5]
                        and raw == 1
                    )
                depth += 1
            elif tag == 12:  # End Collection
                depth -= 1
                require(depth >= 0, "HID collection underflow")
            elif tag == 8:  # Input
                size = global_state["size"]
                count = global_state["count"]
                require(isinstance(size, int) and isinstance(count, int), "Missing input size/count")
                require(size > 0 and count > 0, "Zero-length input field")
                require(len(report_ids) == 1 and 1 in report_ids, "Expected report ID 1 only")
                inputs.append(
                    {
                        "size": size,
                        "count": count,
                        "flags": raw,
                        "page": global_state["page"],
                        "usages": tuple(local_state["usages"]),
                        "usage_min": local_state["usage_min"],
                        "usage_max": local_state["usage_max"],
                        "minimum": global_state["minimum"],
                        "maximum": global_state["maximum"],
                        "offset": bit_offset,
                    }
                )
                bit_offset += size * count
            else:
                raise ContractError("Unexpected main HID item " + str(tag))
            local_state = {"usages": [], "usage_min": None, "usage_max": None}
    require(depth == 0 and root_gamepad, "Not a single properly closed Game Pad collection")
    require(report_ids == {1}, "Unexpected HID report identifiers")
    require(bit_offset % 8 == 0, "Input report has partial trailing byte")
    return bit_offset, inputs


def validate_sources(kernel_c: str, kernel_h: str, rust: str) -> dict:
    descriptor = extract_descriptor(kernel_c)
    bits, fields = parse_hid_descriptor(descriptor)

    expected = [
        (1, 11, 0x02, 9, (), 1, 11, 0, 1, 0),
        (1, 5, 0x03, 9, (), None, None, 0, 1, 11),
        (4, 1, 0x42, 1, (0x39,), None, None, 0, 7, 16),
        (4, 1, 0x03, 1, (), None, None, 0, 7, 20),
        (8, 4, 0x02, 1, (0x30, 0x31, 0x33, 0x34), None, None, -127, 127, 24),
        (8, 2, 0x02, 1, (0x32, 0x35), None, None, 0, 255, 56),
    ]
    require(len(fields) == len(expected), "Unexpected HID Input field count")
    for index, (field, spec) in enumerate(zip(fields, expected)):
        actual = (
            field["size"], field["count"], field["flags"], field["page"],
            field["usages"], field["usage_min"], field["usage_max"],
            field["minimum"], field["maximum"], field["offset"],
        )
        require(actual == spec, "HID Input field " + str(index) + " differs: " + str(actual))

    report_size = 1 + bits // 8  # 1 byte report ID, 9 bytes input fields
    require(report_size == 10, "HID report descriptor must produce 10 bytes")
    require(rust_number(rust, "REPORT_ID") == macro_number(kernel_h, "PB_VHF_REPORT_ID") == 1,
            "C/Rust report ID mismatch")
    require(rust_number(rust, "REPORT_LEN") == report_size, "Rust/C report length mismatch")
    require(rust_number(rust, "HAT_NEUTRAL") == macro_number(kernel_h, "PB_VHF_HAT_NEUTRAL") == 8,
            "C/Rust neutral hat mismatch")
    require(macro_number(kernel_h, "PB_VHF_VALID_BUTTONS") == (1 << 11) - 1,
            "C valid-button bitmap is not 11 bits")

    for rust_name, c_name in [
        ("WATCHDOG_INTERVAL_MS", "PB_WATCHDOG_INTERVAL_MS"),
        ("WATCHDOG_TIMEOUT_MS", "PB_WATCHDOG_TIMEOUT_MS"),
    ]:
        require(rust_number(rust, rust_name) == macro_number(kernel_h, c_name),
                "Rust/C watchdog drift: " + rust_name)
    require(rust_number(rust, "WATCHDOG_INTERVAL_MS") == 200
            and rust_number(rust, "WATCHDOG_TIMEOUT_MS") == 800,
            "Watchdog policy changed; security review required")

    header = remove_comments(kernel_h)
    structure = re.search(
        r"typedef\s+struct\s+_PB_VHF_REPORT\s*\{(.*?)\}\s*PB_VHF_REPORT\b",
        header,
        flags=re.S,
    )
    require(structure is not None, "Kernel report structure not found")
    members = re.findall(r"\b(UCHAR|USHORT|CHAR)\s+(\w+)\s*;", structure.group(1))
    expected_members = [
        ("UCHAR", "ReportId"), ("USHORT", "Buttons"), ("UCHAR", "Hat"),
        ("CHAR", "Lx"), ("CHAR", "Ly"), ("CHAR", "Rx"), ("CHAR", "Ry"),
        ("UCHAR", "Lt"), ("UCHAR", "Rt"),
    ]
    require(members == expected_members, "Packed C report member order/types changed")
    require("pragma pack(push, 1)" in header and "pragma pack(pop)" in header,
            "Packed struct markers missing")
    require(re.search(r"C_ASSERT\s*\(\s*sizeof\s*\(\s*PB_VHF_REPORT\s*\)\s*==\s*10\s*\)", header)
            is not None, "Missing kernel 10-byte static assertion")

    c_ioctl = re.search(
        r"(?m)^#define\s+PB_VHF_IOCTL_SUBMIT\s+CTL_CODE\s*\(\s*FILE_DEVICE_UNKNOWN\s*,\s*0x800\s*,\s*METHOD_BUFFERED\s*,\s*FILE_WRITE_DATA\s*\)",
        kernel_h,
    )
    require(c_ioctl is not None, "Kernel IOCTL control code changed")
    ioctl_expected = (0x22 << 16) | (2 << 14) | (0x800 << 2)
    require(rust_number(rust, "IOCTL_SUBMIT") == ioctl_expected, "Rust/kernel IOCTL number mismatch")

    c_comment = re.search(r"Buttons;\s*//[^\n]*?:\s*([A-Za-z,]+)\s*\(bits", kernel_h)
    require(c_comment is not None, "No human-readable kernel bit order")
    bit_order = tuple(re.findall(r"XButton::(\w+)", re.search(
        r"const\s+BUTTON_ORDER\s*:\s*\[XButton;\s*11\]\s*=\s*\[(.*?)\];", rust, flags=re.S
    ).group(1)))
    require(tuple(c_comment.group(1).split(",")) == bit_order,
            "Rust/C button-bit order drift")

    return {
        "descriptor_bytes": len(descriptor),
        "report_bytes": report_size,
        "input_bits": bits,
        "input_fields": len(fields),
        "button_bits": 11,
        "hat_neutral": 8,
        "watchdog_ms": 800,
        "note": "Generic HID gamepad; no XInput emulation proof",
    }


def run_negative_tests(c_source: str, h_source: str, rust_source: str) -> None:
    def fails(c: str, h: str, r: str, label: str) -> None:
        try:
            validate_sources(c, h, r)
        except ContractError:
            return
        raise ContractError("Drift mutation was not detected: " + label)

    old = "0x95, 0x0B"
    require(c_source.count(old) == 1, "Mutation setup cannot find 11-button report field")
    fails(c_source.replace(old, "0x95, 0x0C", 1), h_source, rust_source, "button count")

    old = "#define PB_VHF_HAT_NEUTRAL 0x08"
    require(h_source.count(old) == 1, "Mutation setup cannot find hat macro")
    fails(c_source, h_source.replace(old, "#define PB_VHF_HAT_NEUTRAL 0x07", 1), rust_source,
          "neutral hat")

    old = "pub const REPORT_LEN: usize = 10;"
    require(rust_source.count(old) == 1, "Mutation setup cannot find report size")
    fails(c_source, h_source, rust_source.replace(old, "pub const REPORT_LEN: usize = 11;", 1),
          "report length")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true",
                        help="also ensure intentional kernel/Rust drift fails validation")
    args = parser.parse_args()
    source_c = (ROOT / "driver/poc-vhf/PadBridgeVhf.c").read_text(encoding="utf-8")
    source_h = (ROOT / "driver/poc-vhf/PadBridgeVhf.h").read_text(encoding="utf-8")
    source_rust = (ROOT / "src-tauri/src/virtual_pad/vhf_protocol.rs").read_text(encoding="utf-8")

    try:
        result = validate_sources(source_c, source_h, source_rust)
        if args.self_test:
            run_negative_tests(source_c, source_h, source_rust)
    except ContractError as error:
        parser.exit(1, "VHF ABI CONTRACT FAILURE: " + str(error) + "\n")
    print("VHF ABI contract: PASS " + str(result))
    if args.self_test:
        print("VHF contract drift detection: PASS (3 independent mutations rejected)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
