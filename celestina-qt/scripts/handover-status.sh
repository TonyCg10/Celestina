#!/usr/bin/env bash
# What this shell has taken over from Noctalia, and what it has not.
#
# Read-only by construction: it starts no process that changes anything, writes
# nothing, and touches neither the session nor the author's configuration. It is
# safe to run at any time, including while both shells are running.
#
# The model lives in `celestina-shell-core::handover`; the validations that
# count as recorded are read from VALIDATION.md, because that file is where the
# author writes down what they actually watched work.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
validation="$root/celestina/VALIDATION.md"

if [[ ! -f $validation ]]; then
    echo "handover: no VALIDATION.md at $validation" >&2
    exit 1
fi

# A validation counts as recorded only when its own section says it passed.
# Anything else — deferred, failed, a result nobody wrote — is not a pass, and
# reading it loosely is how a handover proceeds on checks that never happened.
passed=()
current=""
while IFS= read -r line; do
    if [[ $line =~ ^##[[:space:]]+(VAL-[A-Za-z0-9-]+) ]]; then
        current="${BASH_REMATCH[1]}"
        continue
    fi
    if [[ -n $current && $line =~ ^-[[:space:]]+\*\*Status:\*\*[[:space:]]*passed ]]; then
        passed+=("$current")
        current=""
    fi
done < "$validation"

echo "Handover status"
echo
if ((${#passed[@]})); then
    echo "Recorded as passed: ${passed[*]}"
else
    echo "Recorded as passed: nothing yet"
fi
echo

blocked=0
while IFS=$'\t' read -r name implemented validated; do
    if [[ $implemented == "-" ]]; then
        printf '  [ ] %s — nothing in this shell provides it yet\n' "$name"
        blocked=1
        continue
    fi
    if [[ $validated != "-" ]] && ! printf '%s\n' "${passed[@]:-}" | grep -qx "$validated"; then
        printf '  [~] %s — built in %s, but %s has not been recorded\n' \
            "$name" "$implemented" "$validated"
        blocked=1
        continue
    fi
    printf '  [x] %s — %s\n' "$name" "$implemented"
done < <(sed -n 's/^ *name: "\(.*\)",$/\1/p;s/^ *implemented_by: Some("\(.*\)"),$/\1/p;s/^ *implemented_by: None,$/-/p;s/^ *validated_by: Some("\(.*\)"),$/\1/p;s/^ *validated_by: None,$/-/p' \
    "$root/celestina-rs/crates/celestina-shell-core/src/handover.rs" \
    | paste - - -)

echo
if ((blocked)); then
    echo "Noctalia is still needed. Removal is refused while anything above is"
    echo "unbuilt or unrecorded — see VAL-R8 in celestina/VALIDATION.md."
    exit 2
fi

echo "Every responsibility is built and recorded. Removal may be offered:"
echo "  celestina/scripts/handover-remove.sh --confirm"
