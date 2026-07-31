#!/usr/bin/env python3
"""Coste de decodificar, sin presentar.

Tres preguntas distintas por candidato y fixture:

1. **Arranque:** cuánto tarda en salir el primer frame decodificado, menos el
   suelo de arrancar el propio binario.
2. **Rendimiento:** cuántas veces el tiempo real decodifica el clip entero sin
   pacing — el margen que queda para hacer otras cosas.
3. **Coste sostenido:** CPU y PSS mientras reproduce a 1x, que es lo que el
   usuario paga de verdad.

Sobre el modo de hardware hay una trampa que este harness evita nombrando los
modos por separado: no es lo mismo decodificar en la GPU y **copiar** los frames
de vuelta a RAM (`hw-copy`) que dejarlos vivir en la GPU (`hw-gpu`), que es lo
que hace un reproductor real. Comparar el `hw-gpu` de uno contra el `hw-copy` de
otro produciría una diferencia de memoria y CPU que no existe.

La presentación (frame pacing, tearing, integración con Qt Quick) **no** se mide
aquí: sin superficie real no hay dato honesto. Eso es `measure_presentation.py`,
que abre una ventana y por eso es opt-in.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import spikelib as spike

REALTIME_SECONDS = 10
SW_RANKS = "vah264dec:NONE,vah265dec:NONE,vavp9dec:NONE,vaav1dec:NONE"
HW_RANKS = "vah264dec:MAX,vah265dec:MAX,vavp9dec:MAX,vaav1dec:MAX"

MODE_LABELS = {
    "sw": "software",
    "hw-copy": "VA-API, frames copiados a RAM",
    "hw-gpu": "VA-API, frames residentes en GPU",
}


def mpv_argv(fixture: Path, mode: str, phase: str, kind: str) -> list[str]:
    # `--hwdec=vaapi` sin una vo con contexto GPU no puede crear el dispositivo
    # y mpv cae a software en silencio; headless sólo `vaapi-copy` es real.
    hwdec = "vaapi-copy" if mode == "hw-copy" else "no"
    argv = [
        "mpv",
        "--no-config",
        "--msg-level=all=no",
        "--vo=null",
        "--ao=null",
        f"--hwdec={hwdec}",
    ]
    argv += ["--no-audio"] if kind == "video" else ["--no-video"]
    if phase == "first_frame":
        argv += ["--untimed", "--frames=1"]
    elif phase == "throughput":
        argv += ["--untimed"]
    else:
        argv += [f"--length={REALTIME_SECONDS}"]
    return argv + [str(fixture)]


def ffmpeg_argv(fixture: Path, mode: str, phase: str, kind: str) -> list[str]:
    argv = ["ffmpeg", "-hide_banner", "-v", "error", "-nostdin"]
    if mode.startswith("hw"):
        argv += ["-hwaccel", "vaapi", "-hwaccel_device", "/dev/dri/renderD128"]
    if mode == "hw-gpu":
        argv += ["-hwaccel_output_format", "vaapi"]
    if phase == "realtime":
        argv += ["-re"]
    argv += ["-i", str(fixture), "-an" if kind == "video" else "-vn"]
    if phase == "first_frame":
        argv += ["-frames:v", "1"]
    elif phase == "realtime":
        argv += ["-t", str(REALTIME_SECONDS)]
    return argv + ["-f", "null", "-"]


def gst_argv(fixture: Path, _mode: str, phase: str, _kind: str) -> list[str]:
    # gst-launch reparsea sus argumentos como descripción de pipeline, así que
    # las comillas de un sink con propiedades son parte del texto.
    sync = "true" if phase == "realtime" else "false"
    limit = " num-buffers=1" if phase == "first_frame" else ""
    return [
        "gst-launch-1.0",
        "-q",
        "playbin3",
        f"uri=file://{fixture}",
        f'video-sink="fakesink sync={sync}{limit}"',
        f'audio-sink="fakesink sync={sync}"',
    ]


def qt_argv(fixture: Path, _mode: str, _phase: str, _kind: str) -> list[str]:
    probe = Path(__file__).resolve().parent / "qt_probe.qml"
    return ["qml6", str(probe), "--", str(fixture)]


BACKENDS: dict[str, dict] = {
    "libmpv": {
        "argv": mpv_argv,
        "floor": ["mpv", "--no-config", "--version"],
        # hw-gpu queda fuera headless por límite de mpv, no por falta de soporte.
        "modes": ("sw", "hw-copy"),
        "phases": ("first_frame", "throughput", "realtime"),
    },
    "ffmpeg": {
        "argv": ffmpeg_argv,
        "floor": ["ffmpeg", "-hide_banner", "-version"],
        "modes": ("sw", "hw-copy", "hw-gpu"),
        "phases": ("first_frame", "throughput", "realtime"),
    },
    "gstreamer": {
        "argv": gst_argv,
        "floor": ["gst-launch-1.0", "--version"],
        # `fakesink` acepta VAMemory: el modo de hardware ya es GPU-residente.
        "modes": ("sw", "hw-gpu"),
        "phases": ("first_frame", "throughput", "realtime"),
    },
    "qt-multimedia": {
        "argv": qt_argv,
        "floor": ["qml6", "--version"],
        # El backend FFmpeg de Qt elige solo; la sonda no expone el conmutador.
        "modes": ("auto",),
        "phases": ("realtime",),
    },
}


def backend_env(name: str, mode: str) -> dict[str, str]:
    env: dict[str, str] = {}
    if name == "gstreamer":
        env["GST_PLUGIN_FEATURE_RANK"] = HW_RANKS if mode.startswith("hw") else SW_RANKS
    if name == "qt-multimedia":
        env["QT_QPA_PLATFORM"] = "offscreen"
        env["QT_ASSUME_STDERR_HAS_CONSOLE"] = "1"
    return env


def measure(fixtures: list[dict], out: Path, only: list[str] | None) -> dict:
    logs = out / "logs"
    results: list[dict] = []

    floors: dict[str, float] = {}
    for name, backend in BACKENDS.items():
        if only and name not in only:
            continue
        floor = spike.run_measured(
            f"floor:{name}", backend["floor"], logs / f"floor-{name}.log", timeout_s=30
        )
        floors[name] = floor.wall_s if floor.ok else 0.0

    for fixture in fixtures:
        path = Path(fixture["path"])
        kind = fixture.get("kind", "video")
        duration = float(fixture.get("probe", {}).get("duration", 0) or 0)
        if not path.exists() or kind == "image":
            continue

        for name, backend in BACKENDS.items():
            if only and name not in only:
                continue
            for mode in backend["modes"]:
                if mode.startswith("hw") and kind == "audio":
                    continue  # no hay ruta VA para audio
                for phase in backend["phases"]:
                    if phase == "first_frame" and kind == "audio":
                        continue  # un archivo de audio no tiene primer frame
                    argv = backend["argv"](path, mode, phase, kind)
                    label = f"{name}/{mode}/{phase}/{path.stem}"
                    result = spike.run_measured(
                        label,
                        argv,
                        logs / f"{name}-{mode}-{phase}-{path.stem}.log",
                        timeout_s=180,
                        env=backend_env(name, mode),
                    )
                    entry = result.to_dict()
                    entry.update(
                        {
                            "backend": name,
                            "mode": mode,
                            "phase": phase,
                            "fixture": path.name,
                            "fixture_kind": kind,
                            "fixture_duration_s": duration,
                            "floor_wall_s": round(floors.get(name, 0.0), 4),
                        }
                    )
                    if phase == "throughput" and result.ok and result.wall_s > 0 and duration:
                        entry["speed_factor"] = round(duration / result.wall_s, 2)
                        # La cifra comparable entre backends: núcleo-segundos de
                        # CPU por segundo de contenido decodificado. No depende
                        # de que el backend respete el pacing.
                        entry["cpu_per_content_s"] = round(result.cpu_s / duration, 4)
                        if name == "libmpv" and kind == "audio":
                            entry["notes"].append(
                                "mpv pacea por reloj de audio incluso con --untimed:"
                                " su velocidad aquí no es rendimiento"
                            )
                            entry.pop("speed_factor", None)
                            entry.pop("cpu_per_content_s", None)
                    if phase == "realtime" and result.ok and result.wall_s > 0:
                        entry["cores_used"] = round(result.cpu_s / result.wall_s, 3)
                        # Un pase va a 1x si duró la ventana pedida o si llegó
                        # al final del clip a ritmo real: gst-launch no tiene
                        # equivalente de `--length` y reproduce entero.
                        entry["paced"] = (
                            abs(result.wall_s - REALTIME_SECONDS) < 1.5
                            or (duration > 0 and abs(result.wall_s - duration) < 1.5)
                        )
                    if phase == "first_frame" and result.ok:
                        entry["first_frame_s"] = round(
                            max(result.wall_s - floors.get(name, 0.0), 0.0), 4
                        )
                    results.append(entry)
                    state = (
                        "ok"
                        if result.ok
                        else ("timeout" if result.timed_out else f"exit {result.exit_code}")
                    )
                    print(
                        f"  {label:<54} {result.wall_s:>7.2f}s cpu {result.cpu_s:>6.2f}s  {state}"
                    )
    return {"realtime_seconds": REALTIME_SECONDS, "runs": results}


def render(report: dict) -> str:
    runs = report["runs"]
    blocks = [
        "## Decodificación (sin presentar)",
        "",
        "`hw-copy` devuelve los frames a RAM; `hw-gpu` los deja en la GPU, que es",
        "la ruta de un reproductor real. mpv sólo alcanza `hw-copy` sin superficie.",
        "",
        "«CPU/s de vídeo» son núcleo-segundos por segundo de contenido decodificado,",
        "medidos en el pase sin pacing: es la única cifra comparable, porque no todos",
        "los candidatos respetan 1x desde CLI. La columna «pacing» dice si el pase a",
        "1x realmente duró lo pedido; donde no, su PSS sigue valiendo y su ritmo no.",
    ]
    for fixture in sorted({run["fixture"] for run in runs}):
        rows = []
        for backend in ("libmpv", "ffmpeg", "gstreamer", "qt-multimedia"):
            for mode in ("sw", "hw-copy", "hw-gpu", "auto"):
                subset = {
                    run["phase"]: run
                    for run in runs
                    if run["fixture"] == fixture
                    and run["backend"] == backend
                    and run["mode"] == mode
                }
                if not subset:
                    continue
                first = subset.get("first_frame")
                through = subset.get("throughput")
                real = subset.get("realtime")
                pacing = "—"
                if real and real.get("ok"):
                    pacing = "1x" if real.get("paced") else f"{real['wall_s']:.1f}s"
                rows.append(
                    [
                        f"{backend} ({mode})",
                        f"{first['first_frame_s'] * 1000:.0f} ms"
                        if first and first.get("first_frame_s") is not None
                        else "—",
                        f"{through['speed_factor']}×"
                        if through and through.get("speed_factor")
                        else ("falló" if through and not through.get("ok") else "—"),
                        f"{through['cpu_per_content_s']}"
                        if through and through.get("cpu_per_content_s") is not None
                        else "—",
                        spike.mib(real["peak_pss_kib"]) if real and real.get("ok") else "—",
                        pacing,
                    ]
                )
        blocks += [
            "",
            f"### {fixture}",
            "",
            spike.markdown_table(
                [
                    "Candidato",
                    "Primer frame",
                    "Velocidad",
                    "CPU/s de vídeo",
                    "PSS pico MiB",
                    "Pacing",
                ],
                rows,
            ),
        ]
    return "\n".join(blocks)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=spike.default_out_dir())
    parser.add_argument("--only", nargs="*", help="limita a estos backends")
    parser.add_argument(
        "--report-only",
        action="store_true",
        help="re-imprime el informe desde playback.json sin volver a medir",
    )
    args = parser.parse_args()

    if args.report_only:
        report = spike.read_json(args.out / "playback.json")
    else:
        inventory = spike.read_json(args.out / "fixtures.json")
        report = measure(inventory["fixtures"], args.out, args.only)
        spike.write_json(args.out / "playback.json", report)
    print()
    print(render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
