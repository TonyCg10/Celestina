#!/usr/bin/env python3
"""Qué hay realmente instalado y qué puede integrarse con Qt Quick.

Un candidato no compite sólo por CPU: si su ruta de render hacia Qt Quick no
existe en la máquina, eso es un coste de integración, no un detalle. Este probe
lo registra antes que ningún número de decodificación.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

import spikelib as spike

# El sink Qt6 de GStreamer no está en el paquete base: si falta, integrar
# GStreamer con Qt Quick exige instalarlo (decisión del autor, no del agente).
GST_QT6_ELEMENTS = ("qml6glsink", "qml6glsrc")


def pkg_version(name: str) -> str:
    code, out = spike.capture(["pkg-config", "--modversion", name])
    return out.strip() if code == 0 else "ausente"


def first_line(argv: list[str]) -> str:
    code, out = spike.capture(argv)
    if code != 0:
        return "ausente"
    return out.strip().splitlines()[0] if out.strip() else "sin salida"


def gpu_model() -> str:
    code, out = spike.capture(["lspci"])
    if code != 0:
        return "desconocida"
    for line in out.splitlines():
        if re.search(r"VGA|3D controller|Display controller", line):
            return line.split(":", 2)[-1].strip()
    return "desconocida"


def va_decoders() -> list[str]:
    code, out = spike.capture(["gst-inspect-1.0", "va"])
    if code != 0:
        return []
    return sorted(
        {
            match.group(1)
            for match in re.finditer(r"^\s+(va\w*dec):", out, flags=re.MULTILINE)
        }
    )


def ffmpeg_hwaccels() -> list[str]:
    code, out = spike.capture(["ffmpeg", "-hide_banner", "-hwaccels"])
    if code != 0:
        return []
    lines = [line.strip() for line in out.splitlines()[1:] if line.strip()]
    return lines


def mpv_hwdecs() -> list[str]:
    code, out = spike.capture(["mpv", "--no-config", "--hwdec=help"])
    if code != 0:
        return []
    # Formato: `  <método> (<codec>-<método>)`; interesa el método, no cada codec.
    methods = {
        match.group(1)
        for match in re.finditer(r"^\s{2,}(\S+)\s*(?:\(|$)", out, flags=re.MULTILINE)
    }
    return sorted(method for method in methods if not method.startswith("("))


def gst_element_present(element: str) -> bool:
    code, _ = spike.capture(["gst-inspect-1.0", element])
    return code == 0


def probe() -> dict:
    headers = {
        "mpv/client.h": Path("/usr/include/mpv/client.h").exists(),
        "mpv/render_gl.h": Path("/usr/include/mpv/render_gl.h").exists(),
    }
    return {
        "gpu": gpu_model(),
        "session": {
            "type": spike.capture(["sh", "-c", "echo $XDG_SESSION_TYPE"])[1].strip(),
            "wayland_display": spike.capture(["sh", "-c", "echo $WAYLAND_DISPLAY"])[1].strip(),
        },
        "versions": {
            "mpv": first_line(["mpv", "--version"]),
            "ffmpeg": first_line(["ffmpeg", "-hide_banner", "-version"]),
            "gstreamer": first_line(["gst-launch-1.0", "--version"]),
            "qt_qml": first_line(["qml6", "--version"]),
        },
        "pkg_config": {
            name: pkg_version(name)
            for name in (
                "mpv",
                "libavcodec",
                "libavformat",
                "libavutil",
                "gstreamer-1.0",
                "gstreamer-video-1.0",
                "Qt6Multimedia",
                "Qt6Quick",
                "libva",
            )
        },
        "headers": headers,
        "hardware_decode": {
            "gstreamer_va": va_decoders(),
            "ffmpeg_hwaccels": ffmpeg_hwaccels(),
            "mpv_hwdec": mpv_hwdecs(),
            "va_driver": sorted(
                path.name for path in Path("/usr/lib/dri").glob("*_drv_video.so")
            ),
        },
        "qt_quick_integration": {
            "libmpv_render_api": headers["mpv/render_gl.h"],
            "gstreamer_qt6_sink": {
                element: gst_element_present(element) for element in GST_QT6_ELEMENTS
            },
            "qt_multimedia": pkg_version("Qt6Multimedia") != "ausente",
        },
        "binaries": {
            name: spike.have(name)
            for name in ("mpv", "ffmpeg", "ffprobe", "gst-launch-1.0", "gst-discoverer-1.0", "qml6")
        },
    }


def render(report: dict) -> str:
    integration = report["qt_quick_integration"]
    gst_sinks = integration["gstreamer_qt6_sink"]
    rows = [
        [
            "libmpv",
            report["pkg_config"]["mpv"],
            "sí (render API en cabeceras)" if integration["libmpv_render_api"] else "no",
            ", ".join(report["hardware_decode"]["mpv_hwdec"]) or "—",
        ],
        [
            "GStreamer",
            report["pkg_config"]["gstreamer-1.0"],
            "no: falta " + ", ".join(name for name, ok in gst_sinks.items() if not ok)
            if not all(gst_sinks.values())
            else "sí (qml6glsink)",
            ", ".join(report["hardware_decode"]["gstreamer_va"]) or "—",
        ],
        [
            "FFmpeg",
            report["pkg_config"]["libavcodec"],
            "no directa: hay que escribir el sink",
            ", ".join(report["hardware_decode"]["ffmpeg_hwaccels"]) or "—",
        ],
        [
            "Qt Multimedia",
            report["pkg_config"]["Qt6Multimedia"],
            "sí (VideoOutput nativo)",
            "vía backend FFmpeg",
        ],
    ]
    return "\n".join(
        [
            "## Entorno",
            "",
            f"- GPU: {report['gpu']}",
            f"- Sesión: {report['session']['type']} ({report['session']['wayland_display'] or 'sin Wayland'})",
            f"- Driver VA: {', '.join(report['hardware_decode']['va_driver']) or 'ninguno'}",
            "",
            spike.markdown_table(
                ["Candidato", "Versión", "Integración Qt Quick", "Decode por hardware"], rows
            ),
        ]
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=spike.default_out_dir())
    args = parser.parse_args()

    report = probe()
    spike.write_json(args.out / "environment.json", report)
    print(render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
