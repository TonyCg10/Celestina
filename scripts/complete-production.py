#!/usr/bin/env python3
"""Build, verify, deploy and compare the canonical author-test artifact."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

from project_registry import load_registry


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("project")
    args = parser.parse_args()

    suite = Path(__file__).resolve().parent.parent
    try:
        registry = load_registry(suite / "docs/projects.toml")
    except ValueError as error:
        parser.error(str(error))
    project = next(
        (item for item in registry["projects"] if item["id"] == args.project), None
    )
    if project is None:
        parser.error(f"unregistered project: {args.project}")
    if not project.get("deployable", False):
        parser.error(
            f"{args.project} is not deployable; complete and deploy its consumers"
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
