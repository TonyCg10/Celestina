#!/usr/bin/env bash
# Stop depending on Noctalia — reversibly, and only when that is honest.
#
# Three things this deliberately does not do:
#
#   * uninstall anything. The package stays where it is; only its autostart is
#     disabled, so the way back is turning it on again rather than a reinstall.
#   * run without `--confirm`. Ending a session's old shell is not something to
#     do by mistake or as a side effect of some other script.
#   * proceed while the handover report is incomplete. That refusal is the
#     point: the shell will not help remove what it has not been proven to
#     replace.
#
# The rollback file is written before anything changes. If it cannot be
# written, nothing changes.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
state="${XDG_DATA_HOME:-$HOME/.local/share}/celestina"
rollback="$state/noctalia-rollback.txt"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/noctalia.desktop"

confirmed=0
for argument in "$@"; do
    case "$argument" in
    --confirm) confirmed=1 ;;
    *)
        echo "handover-remove: unknown argument '$argument'" >&2
        exit 64
        ;;
    esac
done

# The report decides, not this script and not the person running it in a hurry.
if ! bash "$root/celestina/scripts/handover-status.sh"; then
    echo
    echo "handover-remove: refused. Finish the checks above first." >&2
    exit 2
fi

if ((!confirmed)); then
    echo
    echo "handover-remove: this would disable Noctalia's autostart for this"
    echo "user. Re-run with --confirm if that is what you want."
    exit 64
fi

mkdir -p "$state"
{
    echo "# How to bring Noctalia back."
    echo "#"
    echo "# Written before anything was changed, on the run that disabled it."
    echo "# Nothing was uninstalled: only the autostart entry below was moved."
    echo
    if [[ -f $autostart ]]; then
        echo "mv '$autostart.celestina-disabled' '$autostart'"
    else
        echo "# There was no autostart entry at $autostart when this ran."
    fi
    echo "# Then log out and back in, or start noctalia by hand."
} > "$rollback"

echo "Rollback written to $rollback"

if [[ -f $autostart ]]; then
    mv -- "$autostart" "$autostart.celestina-disabled"
    echo "Disabled $autostart"
else
    echo "No autostart entry at $autostart; nothing to disable."
fi

echo
echo "Noctalia will not start with the next session. It is still running now:"
echo "stop it when you are ready, and read $rollback if you want it back."
