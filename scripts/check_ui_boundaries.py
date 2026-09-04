#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Quadrant contributors
# SPDX-License-Identifier: GPL-3.0-only

"""Report Quadrant UI dependency-boundary and frozen-API findings.

Stage 0 intentionally keeps this check non-blocking. It always exits successfully
after printing findings so the current migration debt is visible before files move.
Stage 9 may promote the same checks to enforcement after compatibility facades are
removed.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
UI_ROOT = REPO_ROOT / "ui"
CRATES_ROOT = REPO_ROOT / "crates"
BASELINE_PATH = Path(__file__).with_name("ui_api_baseline_v1.json")

IMPORT_RE = re.compile(
    r"(?:import|export)\s*\{.*?\}\s*from\s*\"([^\"]+)\"\s*;", re.DOTALL
)
EXPORT_RE = re.compile(
    r"^\s*export\s+(component|global|enum|struct)\s+([A-Za-z_][A-Za-z0-9_]*)"
    r"(?:\s+inherits\s+([^\s{]+))?",
    re.MULTILINE,
)
API_DECL_RE = re.compile(
    r"^(?:(?:in|out|in-out)\s+property\s+<[^>]+>\s+[A-Za-z_][A-Za-z0-9_]*"
    r"(?:\s*:[^;]+)?|callback\s+[A-Za-z_][A-Za-z0-9_]*(?:\([^;]*\))?"
    r"|public\s+function\s+[A-Za-z_][A-Za-z0-9_]*\([^)]*\)(?:\s*->\s*[^\s{]+)?)\s*;?$"
)


@dataclass(frozen=True)
class Finding:
    code: str
    location: str
    message: str


def relative(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def normalize_space(value: str) -> str:
    return " ".join(value.strip().split())


def slint_files(root: Path) -> list[Path]:
    if not root.exists():
        return []
    return sorted(root.rglob("*.slint"))


def import_targets(path: Path) -> list[tuple[str, Path | None]]:
    text = path.read_text(encoding="utf-8")
    targets: list[tuple[str, Path | None]] = []
    for source in IMPORT_RE.findall(text):
        if source == "std-widgets.slint":
            targets.append((source, None))
            continue
        targets.append((source, (path.parent / source).resolve()))
    return targets


def is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root.resolve())
    except ValueError:
        return False
    return True


def matching_brace(text: str, opening: int) -> int | None:
    depth = 0
    for index in range(opening, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
    return None


def top_level_api(body: str) -> list[str]:
    declarations: list[str] = []
    depth = 0
    for raw_line in body.splitlines():
        line = raw_line.strip()
        if depth == 0 and API_DECL_RE.match(normalize_space(line)):
            declarations.append(normalize_space(line))
        depth += raw_line.count("{") - raw_line.count("}")
    return declarations


def exported_definitions(path: Path) -> list[dict[str, object]]:
    text = path.read_text(encoding="utf-8")
    definitions: list[dict[str, object]] = []
    for match in EXPORT_RE.finditer(text):
        kind, name, inherits = match.groups()
        opening = text.find("{", match.end())
        if opening < 0:
            continue
        closing = matching_brace(text, opening)
        if closing is None:
            continue
        body = text[opening + 1 : closing]
        if kind == "enum":
            api = [normalize_space(item) for item in body.split(",") if item.strip()]
        else:
            api = top_level_api(body)
        definitions.append(
            {
                "kind": kind,
                "name": name,
                "inherits": inherits,
                "api": api,
                "file": relative(path),
            }
        )
    return definitions


def frozen_api_findings() -> list[Finding]:
    if not BASELINE_PATH.exists():
        return [Finding("API000", relative(BASELINE_PATH), "frozen API baseline is missing")]

    baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    expected = baseline["exports"]
    current_by_name: dict[str, list[dict[str, object]]] = {}
    for path in slint_files(UI_ROOT):
        for definition in exported_definitions(path):
            current_by_name.setdefault(str(definition["name"]), []).append(definition)

    findings: list[Finding] = []
    for name, expected_definition in expected.items():
        definitions = current_by_name.get(name, [])
        if not definitions:
            findings.append(Finding("API001", name, "frozen public export is missing"))
            continue
        if len(definitions) > 1:
            locations = ", ".join(str(item["file"]) for item in definitions)
            findings.append(
                Finding("API002", name, f"frozen public export has duplicate definitions: {locations}")
            )
            continue
        current = definitions[0]
        for key in ("kind", "inherits", "api"):
            if current[key] != expected_definition[key]:
                findings.append(
                    Finding(
                        "API003",
                        str(current["file"]),
                        f"{name} {key} differs from scripts/ui_api_baseline_v1.json",
                    )
                )
    return findings


def duplicate_export_findings() -> list[Finding]:
    locations: dict[str, list[str]] = {}
    for path in slint_files(UI_ROOT):
        for definition in exported_definitions(path):
            locations.setdefault(str(definition["name"]), []).append(str(definition["file"]))
    return [
        Finding("UI001", name, f"exported definition appears in: {', '.join(files)}")
        for name, files in sorted(locations.items())
        if len(files) > 1
    ]


def foundation_ownership_findings() -> list[Finding]:
    expected_locations = {
        "ThemeMode": "ui/kit/foundation/theme.slint",
        "Typography": "ui/kit/foundation/theme.slint",
        "Motion": "ui/kit/foundation/theme.slint",
        "Theme": "ui/kit/foundation/theme.slint",
        "Elevation": "ui/kit/foundation/theme.slint",
        "UiConstants": "ui/kit/foundation/constants.slint",
        "FluentIcons": "ui/kit/foundation/fluent_icons.slint",
        "Branding": "ui/kit/foundation/branding.slint",
    }
    definitions: dict[str, list[dict[str, object]]] = {}
    for path in slint_files(UI_ROOT):
        for definition in exported_definitions(path):
            definitions.setdefault(str(definition["name"]), []).append(definition)

    findings: list[Finding] = []
    for name, expected_file in expected_locations.items():
        actual = definitions.get(name, [])
        locations = [str(item["file"]) for item in actual]
        if locations != [expected_file]:
            findings.append(
                Finding(
                    "FND001",
                    name,
                    f"canonical Foundation definition must be only in {expected_file}; found {locations}",
                )
            )

    fluent = definitions.get("FluentIcons", [])
    if fluent and any("app_mark" in str(item) for item in fluent[0]["api"]):
        findings.append(
            Finding("FND002", str(fluent[0]["file"]), "FluentIcons must not expose Branding.app_mark")
        )

    icons_facade = UI_ROOT / "icons.slint"
    if icons_facade.exists():
        facade_definitions = {
            str(item["name"]): item for item in exported_definitions(icons_facade)
        }
        icons = facade_definitions.get("Icons")
        if icons is None:
            findings.append(
                Finding("FND003", relative(icons_facade), "compatibility Icons global is missing")
            )
        else:
            for declaration in icons["api"]:
                match = re.fullmatch(
                    r"out property <image> ([A-Za-z0-9_]+): (Branding|FluentIcons)\.([A-Za-z0-9_]+);",
                    str(declaration),
                )
                if match is None:
                    findings.append(
                        Finding("FND004", relative(icons_facade), f"invalid Icons proxy: {declaration}")
                    )
                    continue
                name, source, member = match.groups()
                expected_source = "Branding" if name == "app_mark" else "FluentIcons"
                if source != expected_source or member != name:
                    findings.append(
                        Finding(
                            "FND004",
                            relative(icons_facade),
                            f"Icons.{name} must proxy {expected_source}.{name}",
                        )
                    )
    return findings


def kit_import_findings() -> list[Finding]:
    kit_root = UI_ROOT / "kit"
    findings: list[Finding] = []
    for path in slint_files(kit_root):
        for source, target in import_targets(path):
            if target is not None and not is_within(target, kit_root):
                findings.append(
                    Finding("KIT001", relative(path), f'Kit import leaves ui/kit: "{source}"')
                )
    return findings


def gallery_import_findings() -> list[Finding]:
    gallery_root = UI_ROOT / "gallery"
    kit_public = (UI_ROOT / "kit" / "kit.slint").resolve()
    findings: list[Finding] = []
    for path in slint_files(gallery_root):
        for source, target in import_targets(path):
            if target is None or is_within(target, gallery_root) or target == kit_public:
                continue
            findings.append(
                Finding(
                    "GAL001",
                    relative(path),
                    f'Gallery import bypasses kit.slint or enters Product UI: "{source}"',
                )
            )

    legacy_gallery = UI_ROOT / "dev" / "design_gallery.slint"
    if legacy_gallery.exists():
        for source, target in import_targets(legacy_gallery):
            if (
                target is None
                or target == kit_public
                or is_within(target, legacy_gallery.parent)
                or is_within(target, gallery_root)
            ):
                continue
            findings.append(
                Finding(
                    "GAL000",
                    relative(legacy_gallery),
                    f'legacy Gallery still imports pre-Kit path "{source}"',
                )
            )
    return findings


def product_gallery_findings() -> list[Finding]:
    app_root = UI_ROOT / "app.slint"
    findings: list[Finding] = []
    if not app_root.exists():
        return findings
    for source, target in import_targets(app_root):
        if target is not None and (
            is_within(target, UI_ROOT / "gallery") or is_within(target, UI_ROOT / "dev")
        ):
            findings.append(
                Finding("PROD001", relative(app_root), f'Product root references Gallery: "{source}"')
            )
    return findings


def cargo_dependency_names(path: Path) -> set[str]:
    names: set[str] = set()
    section = ""
    dependency_section = re.compile(
        r"^(?:target\..+\.)?(dependencies|dev-dependencies|build-dependencies)$"
    )
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if line.startswith("[") and line.endswith("]"):
            section = line.strip("[]")
            continue
        if dependency_section.match(section):
            match = re.match(r'([A-Za-z0-9_-]+)\s*=\s*', line)
            if match:
                names.add(match.group(1))
    return names


def cargo_findings() -> list[Finding]:
    findings: list[Finding] = []
    gallery_manifest = CRATES_ROOT / "quadrant-ui-gallery" / "Cargo.toml"
    forbidden_gallery_dependencies = {
        "quadrant-domain",
        "quadrant-application",
        "quadrant-storage",
        "quadrant-platform",
        "quadrant-ui",
    }
    if gallery_manifest.exists():
        forbidden = sorted(cargo_dependency_names(gallery_manifest) & forbidden_gallery_dependencies)
        if forbidden:
            findings.append(
                Finding(
                    "CAR001",
                    relative(gallery_manifest),
                    f"Gallery crate depends on forbidden Quadrant crates: {', '.join(forbidden)}",
                )
            )

    for manifest in sorted(CRATES_ROOT.glob("*/Cargo.toml")):
        if manifest == gallery_manifest:
            continue
        if "quadrant-ui-gallery" in cargo_dependency_names(manifest):
            findings.append(
                Finding(
                    "CAR002",
                    relative(manifest),
                    "Product crate depends on quadrant-ui-gallery",
                )
            )
    return findings


def spdx_findings() -> list[Finding]:
    findings: list[Finding] = []
    for root in (UI_ROOT / "kit", UI_ROOT / "gallery"):
        for path in slint_files(root):
            header = "\n".join(path.read_text(encoding="utf-8").splitlines()[:12])
            if "SPDX-License-Identifier:" not in header:
                findings.append(
                    Finding("LIC001", relative(path), "Kit/Gallery Slint file has no SPDX header")
                )
    return findings


def main() -> int:
    checks = [
        ("Frozen public API", frozen_api_findings),
        ("Duplicate exports", duplicate_export_findings),
        ("Foundation ownership", foundation_ownership_findings),
        ("Kit imports", kit_import_findings),
        ("Gallery imports", gallery_import_findings),
        ("Product/Gallery separation", product_gallery_findings),
        ("Cargo dependencies", cargo_findings),
        ("SPDX headers", spdx_findings),
    ]
    all_findings: list[Finding] = []
    print("Quadrant UI boundary audit (report-only mode; introduced in Stage 0)")
    for title, check in checks:
        findings = check()
        all_findings.extend(findings)
        print(f"\n{title}: {'OK' if not findings else f'{len(findings)} finding(s)'}")
        for finding in findings:
            print(f"  [{finding.code}] {finding.location}: {finding.message}")

    print(
        f"\nREPORT ONLY: {len(all_findings)} finding(s); exit status remains 0 until Stage 9."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
