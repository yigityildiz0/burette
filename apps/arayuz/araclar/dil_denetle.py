#!/usr/bin/env python3
"""Cetvel UI locale validator and stub generator.

Validates that all 11 locale files under apps/arayuz/diller/ exist, parse cleanly
as UTF-8 JSON, share the exact key schema and {placeholder} tokens of tr.json,
and adhere to translation marker rules (never __CEVIRI__ on tr.json / en.json;
meta.tamMi matches whether any __CEVIRI__ keys remain on the other 9 locales).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

DILLER_DIR = Path(__file__).resolve().parent.parent / "diller"

STUB_LOCALES: list[tuple[str, str, str]] = [
    ("de", "German", "Deutsch"),
    ("fr", "French", "Français"),
    ("es", "Spanish", "Español"),
    ("pt-BR", "Portuguese (Brazil)", "Português (Brasil)"),
    ("ru", "Russian", "Русский"),
    ("ja", "Japanese", "日本語"),
    ("ko", "Korean", "한국어"),
    ("zh-Hans", "Chinese (Simplified)", "简体中文"),
    ("zh-Hant", "Chinese (Traditional)", "繁體中文"),
]

REQUIRED_LOCALES: list[str] = ["tr", "en", *[kod for kod, _, _ in STUB_LOCALES]]
PLACEHOLDER_RE = re.compile(r"\{([a-zA-Z0-9_]+)\}")
CEVIRI_PREFIX = "__CEVIRI__"
# Rust bağlayıcılarının `hata.tur` olarak döndürdüğü sabit sluglar:
# `SayimHatasi::Okunamadi("slug")` ve `HataBilgisi { tur: "slug".to_string(), .. }`.
HATA_SLUG_RE = re.compile(r'Okunamadi\("([a-z0-9_]+)"\)|HataBilgisi\s*\{\s*tur:\s*"([a-z0-9_]+)"')
CRATES_DIR = DILLER_DIR.parents[2] / "crates"


def prefix_values(node: Any, kod: str, ad: str, yerli_ad: str, path: str = "") -> Any:
    if isinstance(node, dict):
        out = {}
        for k, v in node.items():
            sub_path = f"{path}.{k}" if path else k
            if sub_path == "meta.kod":
                out[k] = kod
            elif sub_path == "meta.ad":
                out[k] = ad
            elif sub_path == "meta.yerliAd":
                out[k] = yerli_ad
            elif sub_path == "meta.tamMi":
                out[k] = False
            else:
                out[k] = prefix_values(v, kod, ad, yerli_ad, sub_path)
        return out
    if isinstance(node, list):
        return [prefix_values(item, kod, ad, yerli_ad, f"{path}[]") for item in node]
    if isinstance(node, str):
        clean = node.removeprefix("__CEVIRI__").lstrip()
        return f"{CEVIRI_PREFIX}{clean}"
    return node


def generate_stubs() -> None:
    en_path = DILLER_DIR / "en.json"
    en_data = json.loads(en_path.read_text(encoding="utf-8"))
    for kod, ad, yerli_ad in STUB_LOCALES:
        stub = prefix_values(en_data, kod, ad, yerli_ad)
        out_path = DILLER_DIR / f"{kod}.json"
        out_path.write_text(json.dumps(stub, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def flatten_keys(node: Any, prefix: str = "") -> dict[str, Any]:
    items: dict[str, Any] = {}
    if isinstance(node, dict):
        for k, v in node.items():
            full = f"{prefix}.{k}" if prefix else k
            items.update(flatten_keys(v, full))
    else:
        items[prefix] = node
    return items


def check_all() -> int:
    src_path = DILLER_DIR / "tr.json"
    if not src_path.is_file():
        print(f"ERROR: Missing source file {src_path}", file=sys.stderr)
        return 1

    src_data = json.loads(src_path.read_text(encoding="utf-8"))
    src_flat = flatten_keys(src_data)
    src_keys = set(src_flat.keys())
    errors: list[str] = []

    for kod in REQUIRED_LOCALES:
        fpath = DILLER_DIR / f"{kod}.json"
        if not fpath.is_file():
            errors.append(f"[{kod}] Missing locale file: {fpath.name}")
            continue
        try:
            data = json.loads(fpath.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            errors.append(f"[{kod}] Invalid JSON: {exc}")
            continue

        flat = flatten_keys(data)
        keys = set(flat.keys())
        missing = sorted(src_keys - keys)
        extra = sorted(keys - src_keys)
        if missing:
            errors.append(f"[{kod}] Missing {len(missing)} keys: {', '.join(missing[:10])}")
        if extra:
            errors.append(f"[{kod}] Extra {len(extra)} keys: {', '.join(extra[:10])}")

        is_base = kod in ("tr", "en")
        pending_count = 0
        translated_count = 0
        for key in sorted(src_keys & keys):
            val = flat[key]
            src_val = src_flat[key]
            if type(val) is not type(src_val):
                errors.append(f"[{kod}] Type mismatch at '{key}': {type(val).__name__} != {type(src_val).__name__}")
                continue
            if isinstance(val, str):
                if key.startswith("meta."):
                    continue
                if not val.strip():
                    errors.append(f"[{kod}] Empty string at '{key}'")
                    continue
                if is_base and "__CEVIRI__" in val:
                    errors.append(f"[{kod}] Forbidden __CEVIRI__ marker at '{key}'")
                elif val.startswith("__CEVIRI__"):
                    if not val[len("__CEVIRI__"):].strip():
                        errors.append(f"[{kod}] Empty text after __CEVIRI__ at '{key}'")
                    pending_count += 1
                else:
                    if "__CEVIRI__" in val:
                        errors.append(f"[{kod}] Misplaced __CEVIRI__ marker at '{key}'")
                    translated_count += 1
                src_ph = sorted(PLACEHOLDER_RE.findall(src_val))
                val_ph = sorted(PLACEHOLDER_RE.findall(val))
                if src_ph != val_ph:
                    errors.append(f"[{kod}] Placeholder mismatch at '{key}': {val_ph} != {src_ph}")

        tam_mi = data.get("meta", {}).get("tamMi")
        expected_tam = pending_count == 0
        if tam_mi is not expected_tam:
            errors.append(f"[{kod}] meta.tamMi is {tam_mi!r}, expected {expected_tam!r} (pending={pending_count})")

        status_label = "complete" if pending_count == 0 else f"{translated_count} translated, {pending_count} __CEVIRI__"
        print(f"OK [{kod:>7}] {len(keys)} keys checked ({status_label})")

    # Sözlükte olmayan hata slug'ı arayüzde aksansız ham metin olarak görünür ("Veritabani acilamadi").
    hata_turleri = src_data.get("hataTurleri", {})
    if CRATES_DIR.is_dir():
        for rs in sorted(CRATES_DIR.rglob("*.rs")):
            if "target" in rs.parts:
                continue
            metin = rs.read_text(encoding="utf-8", errors="replace")
            sluglar = {a or b for a, b in HATA_SLUG_RE.findall(metin)}
            for slug in sorted(sluglar - set(hata_turleri)):
                errors.append(f"[tr] hataTurleri.{slug} missing (used in {rs.name})")

    if errors:
        print("\nLocale verification FAILED:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1

    print(f"\nAll {len(REQUIRED_LOCALES)} locale files verified successfully ({len(src_keys)} keys each).")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Cetvel locale JSON files.")
    parser.add_argument(
        "--generate-stubs",
        action="store_true",
        help="Generate the 9 __CEVIRI__ stub locale files from en.json before validating.",
    )
    args = parser.parse_args()
    if args.generate_stubs:
        generate_stubs()
    return check_all()


if __name__ == "__main__":
    raise SystemExit(main())
