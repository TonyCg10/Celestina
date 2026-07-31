#!/usr/bin/env python3
"""Deriva fixtures acotados a partir de media real del autor.

Medir sobre un archivo sintético de barras de color mentiría sobre el coste de
decodificar lo que hay en el disco, y medir sobre una película de 90 minutos
haría el spike inmanejable. Cada fixture es un recorte corto de una fuente real,
escrito sólo en el directorio de salida; ningún archivo del usuario se toca.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import spikelib as spike

# Las fuentes reales del autor son bucles cortos de fondo de pantalla, así que
# los fixtures se construyen repitiendo la fuente hasta la duración pedida: un
# clip más corto que la ventana de medición mediría sobre todo el arranque.
CLIP_SECONDS = 20


def ffprobe_stream(path: Path) -> dict[str, str]:
    code, out = spike.capture(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,r_frame_rate",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1",
            str(path),
        ]
    )
    if code != 0:
        return {}
    fields: dict[str, str] = {}
    for line in out.splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            fields[key] = value
    return fields


def has_audio(path: Path) -> bool:
    code, out = spike.capture_stdout(
        ["ffprobe", "-v", "error", "-select_streams", "a", "-show_entries",
         "stream=codec_name", "-of", "csv=p=0", str(path)]
    )
    return code == 0 and bool(out.strip())


def build(source: Path, audio_source: Path, cover: Path, out_dir: Path, logs: Path) -> list[dict]:
    """Produce los fixtures y devuelve su inventario."""
    out_dir.mkdir(parents=True, exist_ok=True)
    jobs: list[tuple[str, str, Path, list[str]]] = []

    uhd = out_dir / "video-2160p-h264.mp4"
    jobs.append(
        (
            "2160p H.264 (copia directa de la fuente)",
            "video",
            uhd,
            ["ffmpeg", "-y", "-v", "error", "-stream_loop", "-1", "-i", str(source),
             "-t", str(CLIP_SECONDS), "-c", "copy", str(uhd)],
        )
    )

    fhd = out_dir / "video-1080p-h264.mp4"
    jobs.append(
        (
            "1080p H.264 (el caso del presupuesto)",
            "video",
            fhd,
            ["ffmpeg", "-y", "-v", "error", "-stream_loop", "-1", "-i", str(source),
             "-t", str(CLIP_SECONDS), "-vf", "scale=1920:1080", "-c:v", "libx264",
             "-preset", "veryfast", "-crf", "20", "-an", str(fhd)],
        )
    )

    hevc = out_dir / "video-1080p-hevc.mp4"
    jobs.append(
        (
            "1080p HEVC (segundo códec con ruta VA)",
            "video",
            hevc,
            ["ffmpeg", "-y", "-v", "error", "-stream_loop", "-1", "-i", str(source),
             "-t", str(CLIP_SECONDS), "-vf", "scale=1920:1080", "-c:v", "libx265",
             "-preset", "veryfast", "-crf", "24", "-an", str(hevc)],
        )
    )

    audio = out_dir / "audio-con-caratula.mp3"
    jobs.append(
        (
            "MP3 con carátula embebida y etiquetas",
            "audio",
            audio,
            ["ffmpeg", "-y", "-v", "error", "-t", "60", "-i", str(audio_source), "-i", str(cover),
             "-map", "0:a:0", "-map", "1:v:0", "-c:a", "libmp3lame", "-b:a", "192k",
             "-c:v", "mjpeg", "-vf", "scale=600:-1", "-id3v2_version", "3",
             "-metadata:s:v", "title=Album cover", "-metadata:s:v", "comment=Cover (front)",
             "-metadata", "title=Pista de prueba", "-metadata", "artist=Fluorita Spike",
             "-metadata", "album=Medición F2", "-metadata", "track=1", str(audio)],
        )
    )

    inventory: list[dict] = []
    for label, kind, target, argv in jobs:
        result = spike.run_measured(
            f"fixture:{target.name}", argv, logs / f"fixture-{target.name}.log", timeout_s=300
        )
        entry = {
            "label": label,
            "kind": kind,
            "path": str(target),
            "built": result.ok and target.exists(),
            "bytes": target.stat().st_size if target.exists() else 0,
            "build_wall_s": result.wall_s,
            # La cadencia se mide, no se supone: un nombre no puede prometer
            # 60 fps si la fuente del autor va a 30.
            "probe": ffprobe_stream(target) if target.exists() else {},
        }
        inventory.append(entry)
        state = "ok" if entry["built"] else f"FALLÓ (log: {result.log_path})"
        print(f"  {target.name:<28} {state}")

    still = out_dir / "image-large.jpg"
    if not still.exists():
        still.write_bytes(cover.read_bytes())
    inventory.append(
        {
            "label": "Imagen grande real (ruta sin backend AV)",
            "kind": "image",
            "path": str(still),
            "built": still.exists(),
            "bytes": still.stat().st_size,
            "build_wall_s": 0.0,
            "probe": {},
        }
    )
    return inventory


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="clip de vídeo real de origen")
    parser.add_argument(
        "--audio-source",
        type=Path,
        help="origen del fixture de audio; por defecto --source, que debe tener pista de audio",
    )
    parser.add_argument("--cover", type=Path, required=True, help="imagen real para carátula")
    parser.add_argument("--out", type=Path, default=spike.default_out_dir())
    args = parser.parse_args()

    audio_source = args.audio_source or args.source
    for path in (args.source, audio_source, args.cover):
        if not path.is_file():
            print(f"fuente inexistente: {path}")
            return 2
    if not has_audio(audio_source):
        print(f"la fuente de audio no tiene pista de audio: {audio_source}")
        return 2

    fixtures = args.out / "fixtures"
    print(f"Fixtures en {fixtures}")
    inventory = build(args.source, audio_source, args.cover, fixtures, args.out / "logs")
    spike.write_json(
        args.out / "fixtures.json",
        {
            "source": str(args.source),
            "audio_source": str(audio_source),
            "cover": str(args.cover),
            "fixtures": inventory,
        },
    )
    return 0 if all(entry["built"] for entry in inventory) else 1


if __name__ == "__main__":
    raise SystemExit(main())
