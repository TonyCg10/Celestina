#!/usr/bin/env python3
"""Presentación real: frame pacing sobre la sesión Wayland del autor.

Este medidor **abre ventanas** en la sesión viva, así que no corre por defecto y
no lo lanza `run_all.py`: interrumpir el escritorio del autor sin pedirlo es
exactamente lo que el contrato de la suite prohíbe. Hay que pasar
`--abrir-ventanas` a conciencia.

Es la mitad que ningún pase headless puede sustituir: sin superficie no hay
frames presentados, no hay VSync y `libmpv` ni siquiera puede crear su
dispositivo VA-API, con lo que se le mide en su peor configuración.
"""

from __future__ import annotations

import argparse
import os
import re
from pathlib import Path

import spikelib as spike

WINDOW_SECONDS = 15


def mpv_argv(fixture: Path, seconds: int) -> list[str]:
    return [
        "mpv",
        "--no-config",
        "--vo=gpu-next",
        "--gpu-api=vulkan",
        "--hwdec=vaapi",
        "--ao=null",
        f"--length={seconds}",
        # `statusline=status` es lo que destapa la línea de estado: sin ella
        # `--term-status-msg` no imprime y el pase mide sin poder leerse.
        "--msg-level=all=no,cplayer=info,vd=v,vo=v,statusline=status",
        # Los nombres son `frame-drop-count` y `decoder-frame-drop-count`: una
        # propiedad inexistente no falla, simplemente no imprime, y entonces un
        # «0 frames perdidos» significaría «no me enteré».
        "--term-status-msg=pacing drop=${frame-drop-count}"
        " dec-drop=${decoder-frame-drop-count}"
        " fps=${estimated-vf-fps} hwdec=${hwdec-current}",
        str(fixture),
    ]


def gst_argv(fixture: Path, seconds: int) -> list[str]:
    # `fpsdisplaysink` imprime medias y descartes; `glimagesink` presenta en
    # Wayland con los frames ya en la GPU.
    del seconds
    # `-m` imprime los mensajes del bus, que es como `fpsdisplaysink` publica
    # sus medias y descartes sin que nadie conecte una señal.
    # `-v` imprime los cambios de propiedad, y `fpsdisplaysink` publica sus
    # medias y descartes en `last-message`: es la única vía sin conectar señales.
    return [
        "gst-launch-1.0",
        "-v",
        "playbin3",
        f"uri=file://{fixture}",
        'video-sink="fpsdisplaysink video-sink=glimagesink text-overlay=false sync=true"',
        "audio-sink=fakesink",
    ]


BACKENDS = {
    "libmpv": mpv_argv,
    "gstreamer": gst_argv,
}


ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]")


def parse_mpv(log: str) -> dict:
    # El pty trae secuencias de color que se pegarían al valor leído.
    log = ANSI.sub("", log)
    drops = [int(value) for value in re.findall(r"drop=(\d+)", log)]
    decoder_drops = [int(value) for value in re.findall(r"dec-drop=(\d+)", log)]
    fps = [float(value) for value in re.findall(r"fps=([\d.]+)", log)]
    status_hwdec = re.findall(r"hwdec=(\S+)", log)
    # La línea de estado puede faltar; el log verboso siempre dice qué eligió.
    reported = re.findall(r"Using hardware decoding \(([^)]+)\)", log)
    output = re.findall(r"VO: \[([^\]]+)\]", log)
    return {
        "rendered_frames": None,
        "dropped_frames": max(drops, default=0),
        "decoder_dropped_frames": max(decoder_drops, default=0),
        # La última muestra es el régimen; promediar mete dentro la estimación
        # inestable de los primeros segundos y sube el número sin motivo.
        "average_fps": round(fps[-1], 2) if fps else None,
        "hwdec_used": (status_hwdec[-1] if status_hwdec else None)
        or (reported[-1] if reported else "software"),
        "video_output": output[-1] if output else "desconocido",
        "status_line_seen": bool(drops),
    }


def parse_gst(log: str) -> dict:
    log = ANSI.sub("", log)
    measurements = re.findall(
        r"last-message = rendered:\s*(\d+), dropped:\s*(\d+), current:\s*([\d.]+),"
        r"\s*average:\s*([\d.]+)",
        log,
    )
    decoder = re.search(r"(vah26[45]dec|vavp9dec|vaav1dec|avdec_\w+)", log)
    return {
        "rendered_frames": int(measurements[-1][0]) if measurements else None,
        "dropped_frames": max((int(item[1]) for item in measurements), default=0),
        "average_fps": float(measurements[-1][3]) if measurements else None,
        "hwdec_used": decoder.group(1) if decoder else "desconocido",
        "status_line_seen": bool(measurements),
    }


def measure(fixtures: list[dict], out: Path, seconds: int) -> dict:
    logs = out / "logs"
    runs = []
    for fixture in fixtures:
        if fixture.get("kind") != "video":
            continue
        path = Path(fixture["path"])
        for name, builder in BACKENDS.items():
            env = (
                {"GST_PLUGIN_FEATURE_RANK": "vah264dec:MAX,vah265dec:MAX"}
                if name == "gstreamer"
                else {}
            )
            env["LC_ALL"] = "C"  # los mensajes traducidos no son parseables
            result = spike.run_measured(
                f"{name}/presentación/{path.stem}",
                builder(path, seconds),
                logs / f"presentation-{name}-{path.stem}.log",
                timeout_s=seconds + 20,
                env=env,
                use_pty=(name == "libmpv"),
            )
            log_text = Path(result.log_path).read_text(encoding="utf-8", errors="replace")
            entry = result.to_dict()
            entry.update(
                {
                    "backend": name,
                    "fixture": path.name,
                    **(parse_mpv(log_text) if name == "libmpv" else parse_gst(log_text)),
                }
            )
            runs.append(entry)
            print(
                f"  {name}/{path.stem:<24} fps={entry.get('average_fps')} "
                f"drops={entry.get('dropped_frames')} hwdec={entry.get('hwdec_used')}"
            )
    return {"window_seconds": seconds, "runs": runs}


def render(report: dict) -> str:
    rows = [
        [
            f"{run['backend']} · {run['fixture']}",
            str(run.get("average_fps") or "—"),
            str(run.get("dropped_frames", "—")),
            str(run.get("hwdec_used", "—"))
            + (f" · {run['video_output']}" if run.get("video_output") else ""),
            f"{run['cpu_s'] / max(run['wall_s'], 0.001):.3f}",
            spike.mib(run["peak_pss_kib"]),
            "sí" if run.get("status_line_seen") else "NO LEÍDA",
        ]
        for run in report["runs"]
    ]
    return "\n".join(
        [
            "## Presentación en sesión real",
            "",
            "Los dos FPS no son la misma medida: el de GStreamer es la tasa de",
            "render que cuenta `fpsdisplaysink`, y el de mpv es su",
            "`estimated-vf-fps`, una estimación que en un clip de 30 fps llegó a",
            "decir 59. El veredicto de pacing se apoya en los frames perdidos,",
            "que mpv sí cuenta con exactitud, y en la columna de telemetría: un",
            "«0» sin telemetría leída significaría «no me enteré», no «perfecto».",
            "",
            spike.markdown_table(
                [
                    "Candidato · fixture",
                    "FPS medio",
                    "Frames perdidos",
                    "Decodificador",
                    "Núcleos",
                    "PSS MiB",
                    "Telemetría",
                ],
                rows,
            ),
        ]
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=spike.default_out_dir())
    parser.add_argument("--seconds", type=int, default=WINDOW_SECONDS)
    parser.add_argument(
        "--abrir-ventanas",
        action="store_true",
        help="confirma que puede abrir ventanas en la sesión viva del autor",
    )
    args = parser.parse_args()

    if not args.abrir_ventanas:
        print(
            "Este pase abre ventanas reales sobre la sesión del autor.\n"
            "Relanza con --abrir-ventanas cuando sea buen momento."
        )
        return 2
    if not os.environ.get("WAYLAND_DISPLAY"):
        print("no hay WAYLAND_DISPLAY: este pase necesita la sesión real")
        return 2

    inventory = spike.read_json(args.out / "fixtures.json")
    report = measure(inventory["fixtures"], args.out, args.seconds)
    spike.write_json(args.out / "presentation.json", report)
    print()
    print(render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
