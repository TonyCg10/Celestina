"""Primitivas de medición compartidas por el spike de backend de Fluorita.

Nada de esto es código de producto: vive fuera de `src/` y de los crates, no lo
importa ninguna app y no añade dependencias al workspace. Mide lo que ya está
instalado en la máquina del autor para que la decisión de F2 se tome con
números.

Reglas que sí importan:

- Cada proceso medido tiene timeout y se mata de forma determinista.
- La CPU sale de `getrusage(RUSAGE_CHILDREN)` en serie (un hijo cada vez), que
  es exacto; la memoria sale de muestrear `/proc/<pid>/smaps_rollup`, así que un
  proceso más corto que el intervalo queda marcado como no muestreado en vez de
  reportar un cero falso.
- Nada se escribe fuera del directorio de salida que recibe el harness.
"""

from __future__ import annotations

import json
import os
import pty
import resource
import shutil
import subprocess
import threading
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path

SAMPLE_INTERVAL_S = 0.02


@dataclass
class RunResult:
    """Una ejecución medida."""

    label: str
    argv: list[str]
    exit_code: int | None
    timed_out: bool
    wall_s: float
    cpu_s: float
    peak_pss_kib: int
    peak_rss_kib: int
    samples: int
    log_path: str
    notes: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return self.exit_code == 0 and not self.timed_out

    @property
    def sampled(self) -> bool:
        return self.samples > 0

    def to_dict(self) -> dict:
        data = asdict(self)
        data["ok"] = self.ok
        return data


def _drain_pty(master: int, log) -> None:
    """Vuelca el pty al log hasta que el hijo cierra su extremo."""
    try:
        while True:
            chunk = os.read(master, 4096)
            if not chunk:
                return
            log.write(chunk)
            log.flush()
    except OSError:
        return  # EIO: el esclavo se cerró, que es el final normal
    finally:
        try:
            os.close(master)
        except OSError:
            pass


def _sample_memory(pid: int, stop: threading.Event, out: dict) -> None:
    """Muestrea PSS/RSS del proceso hasta que termina o se pide parar."""
    rollup = Path(f"/proc/{pid}/smaps_rollup")
    while not stop.is_set():
        try:
            text = rollup.read_text(encoding="ascii", errors="replace")
        except (OSError, ValueError):
            return
        for line in text.splitlines():
            if line.startswith("Pss:"):
                out["pss"] = max(out.get("pss", 0), int(line.split()[1]))
            elif line.startswith("Rss:"):
                out["rss"] = max(out.get("rss", 0), int(line.split()[1]))
        out["samples"] = out.get("samples", 0) + 1
        stop.wait(SAMPLE_INTERVAL_S)


def run_measured(
    label: str,
    argv: list[str],
    log_path: Path,
    *,
    timeout_s: float = 120.0,
    env: dict[str, str] | None = None,
    stdin_devnull: bool = True,
    use_pty: bool = False,
) -> RunResult:
    """Ejecuta `argv` midiendo pared, CPU y memoria de pico.

    El proceso se mata al vencer `timeout_s`; el resultado dice explícitamente
    que expiró en vez de fingir un éxito.

    `use_pty` le da al hijo un terminal falso: mpv sólo emite su línea de estado
    —de donde salen frames perdidos y FPS— cuando su salida es una tty, así que
    sin esto el pase de presentación mide sin poder leer lo que mide.
    """
    log_path.parent.mkdir(parents=True, exist_ok=True)
    merged_env = dict(os.environ)
    if env:
        merged_env.update(env)

    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.monotonic()
    with log_path.open("wb") as log:
        master = slave = None
        if use_pty:
            master, slave = pty.openpty()
        process = subprocess.Popen(
            argv,
            stdout=slave if use_pty else log,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL if stdin_devnull else None,
            env=merged_env,
        )
        pty_reader = None
        if use_pty:
            os.close(slave)
            pty_reader = threading.Thread(
                target=_drain_pty, args=(master, log), daemon=True
            )
            pty_reader.start()
        memory: dict[str, int] = {}
        stop = threading.Event()
        sampler = threading.Thread(
            target=_sample_memory, args=(process.pid, stop, memory), daemon=True
        )
        sampler.start()

        timed_out = False
        try:
            process.wait(timeout=timeout_s)
        except subprocess.TimeoutExpired:
            timed_out = True
            process.kill()
            process.wait(timeout=10)
        finally:
            stop.set()
            sampler.join(timeout=1)
            if pty_reader is not None:
                pty_reader.join(timeout=2)

    wall = time.monotonic() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime)

    notes: list[str] = []
    if not memory.get("samples"):
        notes.append("proceso más corto que el intervalo de muestreo: sin PSS")

    return RunResult(
        label=label,
        argv=argv,
        exit_code=process.returncode,
        timed_out=timed_out,
        wall_s=round(wall, 4),
        cpu_s=round(cpu, 4),
        peak_pss_kib=memory.get("pss", 0),
        peak_rss_kib=memory.get("rss", 0),
        samples=memory.get("samples", 0),
        log_path=str(log_path),
        notes=notes,
    )


def capture(argv: list[str], *, timeout_s: float = 30.0) -> tuple[int, str]:
    """Ejecuta un comando corto y devuelve (código, salida combinada)."""
    try:
        completed = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=timeout_s,
            check=False,
            env={**os.environ, "LC_ALL": "C"},
        )
    except FileNotFoundError:
        return 127, ""
    except subprocess.TimeoutExpired:
        return 124, ""
    return completed.returncode, (completed.stdout or "") + (completed.stderr or "")


def capture_stdout(argv: list[str], *, timeout_s: float = 30.0) -> tuple[int, str]:
    """Como `capture`, pero sin mezclar stderr.

    Los avisos de ffprobe van a stderr y, mezclados, envenenan cualquier parseo
    posicional de su salida.
    """
    try:
        completed = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=timeout_s,
            check=False,
            env={**os.environ, "LC_ALL": "C"},
        )
    except FileNotFoundError:
        return 127, ""
    except subprocess.TimeoutExpired:
        return 124, ""
    return completed.returncode, completed.stdout or ""


def have(binary: str) -> bool:
    return shutil.which(binary) is not None


def write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def markdown_table(headers: list[str], rows: list[list[str]]) -> str:
    """Tabla markdown simple; las filas ya vienen formateadas."""
    lines = ["| " + " | ".join(headers) + " |", "|" + "|".join(["---"] * len(headers)) + "|"]
    for row in rows:
        lines.append("| " + " | ".join(row) + " |")
    return "\n".join(lines)


def mib(kib: int) -> str:
    return f"{kib / 1024:.1f}" if kib else "—"


def default_out_dir() -> Path:
    return Path(os.environ.get("TMPDIR", "/tmp")) / "fluorita-spike"
