#!/usr/bin/env python3
"""Ejecuta el spike headless completo y escribe un informe reproducible.

Orden: entorno → cierre instalado → fixtures → decodificación → recursos
derivados. El pase de presentación queda fuera a propósito: abre ventanas en la
sesión del autor y se lanza a mano con `measure_presentation.py`.

Nada de esto decide nada por sí solo. El informe es la entrada de una decisión
del autor, que es quien aprueba qué dependencia pesada entra en
`fluorita-engine`.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

import measure_closure
import measure_derived
import measure_playback
import measure_presentation
import prepare_fixtures
import probe_environment
import spikelib as spike

HERE = Path(__file__).resolve().parent


def run_step(script: str, argv: list[str]) -> bool:
    print(f"\n=== {script} ===")
    completed = subprocess.run([sys.executable, str(HERE / script), *argv], check=False)
    return completed.returncode == 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=spike.default_out_dir())
    parser.add_argument("--source", type=Path, required=True, help="clip de vídeo real")
    parser.add_argument("--audio-source", type=Path, help="origen con pista de audio")
    parser.add_argument("--cover", type=Path, required=True, help="imagen real")
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    fixture_argv = ["--source", str(args.source), "--cover", str(args.cover), "--out", str(out)]
    if args.audio_source:
        fixture_argv += ["--audio-source", str(args.audio_source)]

    steps = [
        ("probe_environment.py", ["--out", str(out)]),
        ("measure_closure.py", ["--out", str(out)]),
        ("prepare_fixtures.py", fixture_argv),
        ("measure_playback.py", ["--out", str(out)]),
        ("measure_derived.py", ["--out", str(out)]),
    ]
    failures = [script for script, argv in steps if not run_step(script, argv)]

    report_path = out / "report.md"
    sections = [
        "# Spike F2 — backend de decodificación de Fluorita",
        "",
        "Generado por `fluorita/spikes/run_all.py`. Sólo mide; no decide.",
        "",
        probe_environment.render(spike.read_json(out / "environment.json")),
        "",
        measure_closure.render(spike.read_json(out / "closure.json")),
        "",
        measure_playback.render(spike.read_json(out / "playback.json")),
        "",
        measure_derived.render(spike.read_json(out / "derived.json")),
        "",
        # El pase con ventana no lo lanza este script, pero si ya se corrió a
        # mano su resultado pertenece al mismo informe.
        (
            measure_presentation.render(spike.read_json(out / "presentation.json"))
            if (out / "presentation.json").exists()
            else "## Presentación en sesión real\n\nNo ejecutada."
        ),
        "",
        "## Lo que este informe no prueba",
        "",
        "- Tearing, comportamiento en 60 Hz y estabilidad de pacing con contenido",
        "  más exigente: los fixtures del autor son de 30 fps.",
        "- Integración con Qt Quick: aquí sólo se comprueba que la API exista.",
        "- Estabilidad en horas de reproducción, seeks agresivos o archivos rotos.",
    ]
    report_path.write_text("\n".join(sections) + "\n", encoding="utf-8")
    print(f"\nInforme: {report_path}")

    if failures:
        print("pasos con error: " + ", ".join(failures))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
