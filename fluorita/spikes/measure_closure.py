#!/usr/bin/env python3
"""Cuánto pesa realmente cada candidato sobre lo que Fluorita ya necesita.

El número honesto no es «cuánto ocupa el paquete» sino el cierre recursivo que
añade *por encima* de la línea base que la app tiene igualmente (Qt Quick, Mesa,
glibc). Se reportan dos cifras, porque cuentan cosas distintas:

- **Delta sobre la base:** lo que instalaría una máquina limpia.
- **Delta exclusivo:** lo que nadie más en este sistema exige ya; el resto
  estaría en el disco aunque Fluorita no existiera.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

import spikelib as spike

# Lo que la app arrastra sí o sí por ser Qt Quick sobre esta máquina.
BASELINE = ["qt6-base", "qt6-declarative", "mesa"]

CANDIDATES: dict[str, list[str]] = {
    "libmpv": ["mpv"],
    "gstreamer": [
        "gstreamer",
        "gst-plugins-base",
        "gst-plugins-good",
        "gst-libav",
        "gst-plugin-va",
    ],
    "ffmpeg": ["ffmpeg"],
    "qt-multimedia": ["qt6-multimedia", "qt6-multimedia-ffmpeg"],
}

# Paquetes que un candidato necesitaría para llegar a Qt Quick y que no están
# instalados: no se pueden medir sin instalar, y eso lo decide el autor.
INTEGRATION_EXTRAS: dict[str, list[str]] = {
    "gstreamer": ["gst-plugin-qt6"],
}


def closure(packages: list[str]) -> set[str]:
    found: set[str] = set()
    for package in packages:
        code, out = spike.capture(["pactree", "-lu", package])
        if code != 0:
            continue
        found.update(normalize(line) for line in out.splitlines() if line.strip())
    return found - {""}


def normalize(entry: str) -> str:
    """`pactree` imprime la restricción de versión (`glibc>=2.27`).

    Sin quitarla, el mismo paquete cuenta como dos nombres distintos y el delta
    de un candidato se infla con dependencias que la base ya traía.
    """
    return re.split(r"[<>=]", entry.strip(), maxsplit=1)[0].strip()


def package_facts(packages: set[str]) -> dict[str, dict]:
    if not packages:
        return {}
    code, out = spike.capture(["pacman", "-Qi", *sorted(packages)], timeout_s=120)
    if code != 0:
        return {}
    facts: dict[str, dict] = {}
    current: dict[str, str] = {}
    for line in out.splitlines():
        if not line.strip():
            if current.get("Name"):
                facts[current["Name"]] = {
                    "size_kib": parse_size(current.get("Installed Size", "0")),
                    "required_by": parse_list(current.get("Required By", "None")),
                }
            current = {}
            continue
        match = re.match(r"^(\S[^:]*?)\s*:\s*(.*)$", line)
        if match:
            current[match.group(1).strip()] = match.group(2).strip()
        elif current:
            last = list(current)[-1]
            current[last] += " " + line.strip()
    if current.get("Name"):
        facts[current["Name"]] = {
            "size_kib": parse_size(current.get("Installed Size", "0")),
            "required_by": parse_list(current.get("Required By", "None")),
        }
    return facts


def parse_size(value: str) -> int:
    match = re.match(r"([\d.,]+)\s*(\w+)", value)
    if not match:
        return 0
    amount = float(match.group(1).replace(",", "."))
    unit = match.group(2).upper()
    factor = {"B": 1 / 1024, "KIB": 1, "MIB": 1024, "GIB": 1024 * 1024}.get(unit, 1)
    return int(amount * factor)


def parse_list(value: str) -> list[str]:
    if value in ("None", ""):
        return []
    return [item for item in value.split() if item]


def measure() -> dict:
    base = closure(BASELINE)
    # Los cuatro candidatos acaban arrastrando FFmpeg (mpv enlaza libav*,
    # gst-libav y qt6-multimedia-ffmpeg lo usan de backend), así que el número
    # que de verdad separa a unos de otros es lo que añaden *sobre* ese núcleo.
    base_with_ffmpeg = base | closure(["ffmpeg"])
    results: dict[str, dict] = {}

    for name, packages in CANDIDATES.items():
        installed = [pkg for pkg in packages if spike.capture(["pacman", "-Qq", pkg])[0] == 0]
        full = closure(packages)
        delta = full - base
        facts = package_facts(delta)
        delta_kib = sum(entry["size_kib"] for entry in facts.values())

        # Exclusivo: nada fuera del propio cierre del candidato lo exige ya.
        exclusive = {
            package: entry
            for package, entry in facts.items()
            if not set(entry["required_by"]) - full
        }
        over_ffmpeg = full - base_with_ffmpeg
        over_ffmpeg_facts = package_facts(over_ffmpeg)
        results[name] = {
            "packages": packages,
            "all_installed": len(installed) == len(packages),
            "missing_packages": [pkg for pkg in packages if pkg not in installed],
            "closure_size": len(full),
            "delta_packages": len(delta),
            "delta_kib": delta_kib,
            "exclusive_packages": len(exclusive),
            "exclusive_kib": sum(entry["size_kib"] for entry in exclusive.values()),
            "over_ffmpeg_packages": len(over_ffmpeg),
            "over_ffmpeg_kib": sum(entry["size_kib"] for entry in over_ffmpeg_facts.values()),
            "over_ffmpeg_largest": sorted(
                ((entry["size_kib"], package) for package, entry in over_ffmpeg_facts.items()),
                reverse=True,
            )[:3],
            "largest": sorted(
                ((entry["size_kib"], package) for package, entry in facts.items()), reverse=True
            )[:5],
            "integration_extras": {
                extra: spike.capture(["pacman", "-Qq", extra])[0] == 0
                for extra in INTEGRATION_EXTRAS.get(name, [])
            },
        }
    return {"baseline": sorted(base), "baseline_packages": len(base), "candidates": results}


def render(report: dict) -> str:
    rows = []
    for name, data in report["candidates"].items():
        extras = data["integration_extras"]
        missing_extras = [extra for extra, present in extras.items() if not present]
        rows.append(
            [
                name,
                str(data["delta_packages"]),
                spike.mib(data["delta_kib"]),
                f"{data['over_ffmpeg_packages']} / {spike.mib(data['over_ffmpeg_kib'])}",
                ", ".join(
                    f"{package} {spike.mib(size)}"
                    for size, package in data["over_ffmpeg_largest"][:2]
                )
                or "—",
                "falta " + ", ".join(missing_extras) if missing_extras else "—",
            ]
        )
    return "\n".join(
        [
            "## Cierre instalado",
            "",
            f"Línea base (Qt Quick + Mesa): {report['baseline_packages']} paquetes.",
            "FFmpeg entra en el cierre de los cuatro candidatos, así que la columna",
            "decisiva es la que mide *sobre* base + FFmpeg.",
            "",
            spike.markdown_table(
                [
                    "Candidato",
                    "Paquetes sobre base",
                    "MiB sobre base",
                    "Paquetes / MiB sobre base+FFmpeg",
                    "Mayores sobre base+FFmpeg (MiB)",
                    "Extra de integración",
                ],
                rows,
            ),
        ]
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=spike.default_out_dir())
    args = parser.parse_args()

    if not spike.have("pactree") or not spike.have("pacman"):
        print("este medidor asume pacman/pactree (Arch); sáltalo en otra distro")
        return 2

    report = measure()
    spike.write_json(args.out / "closure.json", report)
    print(render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
