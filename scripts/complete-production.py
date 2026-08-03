#!/usr/bin/env python3
"""Build, verify, deploy and compare the canonical author-test artifact."""

from __future__ import annotations

import argparse
import subprocess
import sys
import tomllib
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("project")
    args = parser.parse_args()

    suite = Path(__file__).resolve().parent.parent
    with (suite / "docs/projects.toml").open("rb") as stream:
        registry = tomllib.load(stream)
    project = next(
        (item for item in registry["projects"] if item["id"] == args.project), None
    )
    if project is None:
        parser.error(f"proyecto no registrado: {args.project}")
    if not project.get("deployable", False):
        parser.error(
            f"{args.project} no es desplegable; completa y despliega sus consumidores"
        )

    for key in ("build_script", "verify_script", "deploy_script", "status_script"):
        script = suite / project[key]
        print(f">> {script.relative_to(suite)}", flush=True)
        subprocess.run([str(script)], cwd=suite, check=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode) from error
