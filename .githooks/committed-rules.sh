#!/bin/sh
set -eu

# Shared body of the repository hooks: run scripts/git_hooks.py with the rules
# committed in HEAD.
#
#   committed-rules.sh HOOK [ARGUMENTS...]
#
# HEAD's scripts/ is extracted into a temporary directory and the guards run
# from there, so a staged or unstaged edit of a guard never judges the commit
# that carries it; it becomes the rule once it is committed (AGENTS.md "Git and
# commits"). While HEAD predates scripts/git_hooks.py, the worktree's runner
# drives HEAD's guards; that happens once, in the commit that adds it.

hook=$1
shift
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)

rules=$(mktemp -d "${TMPDIR:-/tmp}/celestina-rules.XXXXXX")
trap 'rm -rf -- "$rules"' EXIT
trap 'exit 130' HUP INT TERM
if git -C "$root" rev-parse --quiet --verify 'HEAD^{commit}' >/dev/null 2>&1 \
    && git -C "$root" cat-file -e 'HEAD:scripts' 2>/dev/null; then
    git -C "$root" archive --format=tar HEAD -- scripts | tar -x -f - -C "$rules"
fi
runner=$rules/scripts/git_hooks.py
if [ ! -f "$runner" ]; then
    runner=$root/scripts/git_hooks.py
fi
mkdir -p -- "$rules/scripts"
python3 "$runner" --root "$root" --rules "$rules/scripts" "$hook" "$@"
