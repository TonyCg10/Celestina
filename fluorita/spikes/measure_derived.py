#!/usr/bin/env python3
"""Recursos derivados: metadata, póster, carátula y tráiler acotado.

Esto es la mitad del contrato de F2 que no se ve al reproducir: catalogar una
biblioteca extrae metadata de miles de archivos y escribe un PNG por cada uno,
así que el coste por archivo pesa más que el pico de un reproductor.

Dos advertencias que el harness sí distingue:

- Que algo no salga con **una línea de CLI** no significa que la librería no
  pueda: `gst-launch` no expresa un seek+un frame sin escribir código, y eso se
  marca como «exige código», no como «no puede».
- El tráiler se valida contra el presupuesto que ya congeló `fluorita-core`
  (5 s, 1280×720, 24 MiB): un tráiler que se sale del presupuesto es un fallo
  del candidato, no una medición mejor.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import spikelib as spike

# Debe coincidir con `TrailerBudget::conservative()` en fluorita-core.
TRAILER_SECONDS = 5
TRAILER_WIDTH = 1280
TRAILER_MAX_BYTES = 24 * 1024 * 1024
POSTER_MAX_PIXELS = 256
# El punto de extracción se calcula sobre la duración real del fixture: un
# `-ss` fijo más allá del final produce un archivo vacío y, peor, ffmpeg sale 0.
SEEK_FRACTION = 0.25
# Un artefacto por debajo de esto no es un póster ni un tráiler, es un envase.
MIN_ARTIFACT_BYTES = 1024


def probe_duration(path: Path) -> float:
    code, out = spike.capture_stdout(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration",
         "-of", "default=noprint_wrappers=1:nokey=1", str(path)]
    )
    try:
        return float(out.strip()) if code == 0 else 0.0
    except ValueError:
        return 0.0


def metadata_jobs(video: Path, audio: Path) -> list[tuple[str, str, list[str] | None]]:
    return [
        (
            "ffmpeg",
            "metadata",
            ["ffprobe", "-v", "error", "-show_format", "-show_streams", "-of", "json", str(video)],
        ),
        ("gstreamer", "metadata", ["gst-discoverer-1.0", "-v", str(video)]),
        (
            "libmpv",
            "metadata",
            [
                "mpv",
                "--no-config",
                "--vo=null",
                "--ao=null",
                "--frames=0",
                "--msg-level=all=no,cplayer=info",
                "--term-playing-msg=duration=${=duration} size=${width}x${height}",
                str(video),
            ],
        ),
        (
            "ffmpeg",
            "metadata-audio",
            ["ffprobe", "-v", "error", "-show_format", "-show_streams", "-of", "json", str(audio)],
        ),
        ("gstreamer", "metadata-audio", ["gst-discoverer-1.0", "-v", str(audio)]),
    ]


def derived_jobs(
    video: Path, audio: Path, work: Path, seek: float
) -> list[tuple[str, str, list[str] | None]]:
    poster_png = work / "poster-ffmpeg.png"
    mpv_poster_dir = work / "poster-mpv"
    mpv_poster_dir.mkdir(parents=True, exist_ok=True)
    cover_jpg = work / "cover-ffmpeg.jpg"
    trailer_sw = work / "trailer-sw.mp4"
    trailer_hw = work / "trailer-hw.mp4"

    return [
        (
            "ffmpeg",
            "poster",
            [
                "ffmpeg", "-y", "-v", "error", "-nostdin", "-ss", f"{seek:.2f}",
                "-i", str(video), "-frames:v", "1",
                "-vf", f"scale={POSTER_MAX_PIXELS}:-1", str(poster_png),
            ],
        ),
        (
            "libmpv",
            "poster",
            [
                "mpv", "--no-config", "--msg-level=all=no", "--ao=null", "--no-audio",
                f"--start={seek:.2f}", "--frames=1", "--vo=image",
                "--vo-image-format=png", f"--vo-image-outdir={mpv_poster_dir}",
                f"--vf=scale={POSTER_MAX_PIXELS}:-2", str(video),
            ],
        ),
        # gst-launch no expresa «busca a T y saca un frame» sin código: el
        # elemento existe, la línea de comandos no.
        ("gstreamer", "poster", None),
        (
            "ffmpeg",
            "cover",
            [
                "ffmpeg", "-y", "-v", "error", "-nostdin", "-i", str(audio),
                "-an", "-map", "0:v:0", "-c", "copy", str(cover_jpg),
            ],
        ),
        ("libmpv", "cover", None),
        ("gstreamer", "cover", None),
        (
            "ffmpeg",
            "trailer-sw",
            [
                "ffmpeg", "-y", "-v", "error", "-nostdin", "-ss", f"{seek:.2f}",
                "-t", str(TRAILER_SECONDS), "-i", str(video),
                "-vf", f"scale={TRAILER_WIDTH}:-2", "-c:v", "libx264",
                "-preset", "veryfast", "-an", str(trailer_sw),
            ],
        ),
        (
            "ffmpeg",
            "trailer-hw",
            [
                "ffmpeg", "-y", "-v", "error", "-nostdin",
                "-vaapi_device", "/dev/dri/renderD128",
                "-ss", f"{seek:.2f}", "-t", str(TRAILER_SECONDS), "-i", str(video),
                "-vf", f"scale={TRAILER_WIDTH}:-2,format=nv12,hwupload",
                "-c:v", "h264_vaapi", "-an", str(trailer_hw),
            ],
        ),
    ]


def validate_trailer(path: Path) -> dict:
    """El tráiler debe caber en el presupuesto congelado por el core."""
    if not path.exists():
        return {"within_budget": False, "reason": "no se produjo"}
    size = path.stat().st_size
    code, out = spike.capture_stdout(
        [
            "ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
            "stream=width,height", "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1", str(path),
        ]
    )
    values = out.split()
    width = int(values[0]) if len(values) > 0 and values[0].isdigit() else 0
    height = int(values[1]) if len(values) > 1 and values[1].isdigit() else 0
    duration = float(values[2]) if len(values) > 2 else 0.0
    reasons = []
    if size > TRAILER_MAX_BYTES:
        reasons.append(f"{size} bytes > presupuesto")
    if width * height > TRAILER_WIDTH * 720:
        reasons.append(f"{width}x{height} > 1280x720")
    if duration > TRAILER_SECONDS + 0.5:
        reasons.append(f"{duration:.1f}s > {TRAILER_SECONDS}s")
    return {
        "within_budget": code == 0 and not reasons,
        "bytes": size,
        "width": width,
        "height": height,
        "duration_s": round(duration, 2),
        "reason": "; ".join(reasons) or "dentro del presupuesto",
    }


def measure_cancellation(video: Path, work: Path, logs: Path) -> dict:
    """Un tráiler cancelado debe morir rápido y no dejar basura publicable."""
    target = work / "trailer-cancelada.mp4"
    argv = [
        "ffmpeg", "-y", "-v", "error", "-nostdin", "-i", str(video),
        "-vf", f"scale={TRAILER_WIDTH}:-2", "-c:v", "libx264", "-preset", "veryslow",
        "-an", str(target),
    ]
    result = spike.run_measured(
        "cancelación", argv, logs / "trailer-cancelada.log", timeout_s=0.4
    )
    partial = target.exists()
    partial_bytes = target.stat().st_size if partial else 0
    if partial:
        target.unlink()
    return {
        "killed_after_s": 0.4,
        "died_in_s": round(result.wall_s, 3),
        "left_partial_output": partial,
        "partial_bytes": partial_bytes,
        "removable": not target.exists(),
    }


def artifact_of(backend: str, operation: str, argv: list[str]) -> Path | None:
    """Dónde debería haber quedado el resultado de este trabajo."""
    if backend == "libmpv" and operation == "poster":
        outdir = next(
            (Path(arg.split("=", 1)[1]) for arg in argv if arg.startswith("--vo-image-outdir=")),
            None,
        )
        if outdir is None:
            return None
        images = sorted(outdir.glob("*.png"))
        return images[-1] if images else outdir / "sin-imagen.png"
    return Path(argv[-1])


def measure(fixtures: list[dict], out: Path) -> dict:
    # El fixture se elige por forma, no por un nombre codificado: renombrar un
    # fixture no puede romper el medidor ni, peor, hacerle medir otro archivo.
    video = next(
        Path(f["path"])
        for f in fixtures
        if f["kind"] == "video" and f.get("probe", {}).get("width") == "1920"
    )
    audio = next(Path(f["path"]) for f in fixtures if f["kind"] == "audio")
    work = out / "derived"
    work.mkdir(parents=True, exist_ok=True)
    logs = out / "logs"

    runs: list[dict] = []
    seek = max(probe_duration(video) * SEEK_FRACTION, 0.0)
    jobs = metadata_jobs(video, audio) + derived_jobs(video, audio, work, seek)
    for backend, operation, argv in jobs:
        if argv is None:
            runs.append(
                {
                    "backend": backend,
                    "operation": operation,
                    "supported_via_cli": False,
                    "note": "la librería lo permite; la línea de comandos no lo expresa",
                }
            )
            continue
        result = spike.run_measured(
            f"{backend}/{operation}", argv, logs / f"derived-{backend}-{operation}.log", timeout_s=120
        )
        entry = result.to_dict()
        entry.update({"backend": backend, "operation": operation, "supported_via_cli": True})
        if operation in ("poster", "cover") or operation.startswith("trailer"):
            artifact = artifact_of(backend, operation, argv)
            produced = artifact is not None and artifact.exists()
            size = artifact.stat().st_size if produced else 0
            entry["artifact"] = str(artifact) if artifact else None
            entry["artifact_bytes"] = size
            if size < MIN_ARTIFACT_BYTES:
                # ffmpeg sale 0 tras no escribir nada; sin esto el spike
                # publicaría un «ok» por un archivo vacío.
                entry["ok"] = False
                entry["notes"].append("salió 0 pero no produjo un artefacto usable")
        if operation.startswith("trailer"):
            entry["budget"] = validate_trailer(Path(argv[-1]))
        runs.append(entry)
        state = "ok" if entry["ok"] else f"exit {result.exit_code}"
        print(f"  {backend}/{operation:<16} {result.wall_s:>6.2f}s cpu {result.cpu_s:>5.2f}s  {state}")

    cancellation = measure_cancellation(video, work, logs)
    print(f"  cancelación: murió en {cancellation['died_in_s']}s, parcial={cancellation['left_partial_output']}")
    return {"fixture_video": str(video), "fixture_audio": str(audio), "runs": runs, "cancellation": cancellation}


def render(report: dict) -> str:
    operations = ["metadata", "metadata-audio", "poster", "cover", "trailer-sw", "trailer-hw"]
    rows = []
    for backend in ("libmpv", "ffmpeg", "gstreamer"):
        cells = [backend]
        for operation in operations:
            match = next(
                (
                    run
                    for run in report["runs"]
                    if run["backend"] == backend and run["operation"] == operation
                ),
                None,
            )
            if match is None:
                cells.append("—")
            elif not match.get("supported_via_cli"):
                cells.append("exige código")
            elif match.get("ok"):
                cells.append(f"{match['wall_s'] * 1000:.0f} ms")
            else:
                cells.append("falló")
        rows.append(cells)

    cancellation = report["cancellation"]
    trailers = [
        run for run in report["runs"] if run.get("operation", "").startswith("trailer") and run.get("budget")
    ]
    lines = [
        "## Recursos derivados",
        "",
        spike.markdown_table(["Candidato", *operations], rows),
        "",
        "Tráiler contra el presupuesto de `fluorita-core` (5 s · 1280×720 · 24 MiB):",
        "",
    ]
    for run in trailers:
        budget = run["budget"]
        verdict = "dentro" if budget["within_budget"] else f"FUERA ({budget['reason']})"
        lines.append(
            f"- `{run['operation']}`: {budget['width']}×{budget['height']},"
            f" {budget['duration_s']} s, {budget['bytes'] / 1024:.0f} KiB → {verdict}"
        )
    lines += [
        "",
        f"Cancelación a los {cancellation['killed_after_s']} s: el proceso murió en"
        f" {cancellation['died_in_s']} s y dejó"
        + (
            f" {cancellation['partial_bytes']} bytes parciales, borrables por el host."
            if cancellation["left_partial_output"]
            else " ningún archivo parcial."
        ),
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=spike.default_out_dir())
    parser.add_argument("--report-only", action="store_true")
    args = parser.parse_args()

    if args.report_only:
        report = spike.read_json(args.out / "derived.json")
    else:
        inventory = spike.read_json(args.out / "fixtures.json")
        report = measure(inventory["fixtures"], args.out)
        spike.write_json(args.out / "derived.json", report)
    print()
    print(render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
