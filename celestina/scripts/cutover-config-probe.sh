#!/bin/sh
set -eu

# cutover-config-probe.sh FILE — checks a copy of a niri config for the
# Celestina cutover (CEL-2-M, design §5.3 and §8).
#
# FILE is a copy the author made of `~/.config/niri/config.kdl`; this script
# never reads `~/.config` itself and refuses a path under it (or under
# `$XDG_CONFIG_HOME`), comparing canonical paths so a symlink, a `..` or a
# `//` cannot smuggle the live file in, and reading only the canonical path
# once it passed, so a symlink swapped after the check is not followed; a
# landing or a smoke can never run it against the live file. `HOME` must be
# set (usage error otherwise): without it the refusal has no `~/.config` to
# compare against. Known limit: a hard link to the live file is the same
# inode under another path and cannot be told apart by path; the author's
# copies are made with `cp`, never `ln`. It first offers the file to
# `niri validate -c FILE` (the suite's own niri under
# `~/.local/lib/celestina/niri` when it is there, as dev-session.sh prefers,
# else the one on PATH; skipped with a notice when neither exists), then
# fails with one line per finding when:
#
#   - a `spawn`, `spawn-at-startup`, `spawn-sh` or `spawn-sh-at-startup` node
#     still names `noctalia`;
#   - the `Mod+Shift+A` bind is missing or does not spawn `swaylock` (the
#     lock's recovery path, design §6, is kept verbatim);
#   - a startup node still carries `caffeine-enable` or
#     `nightlight-force-toggle` (the deleted line 73 of design §2.1);
#   - no `spawn-at-startup` (or `spawn-sh-at-startup`) node starts
#     `celestina`.
#
# What the compositor does not read is removed first, so a line it ignores
# cannot satisfy a check: `//` comments to the end of their line, `/* ... */`
# block comments (nested as KDL allows, across lines), and slashdashed nodes
# (`/-node ...`, which disables the node together with its children block,
# so `/-binds {` disables every bind inside) or slashdashed blocks
# (`/-{ ... }`). Quoted strings are followed so a `//`, a `/*` or a brace
# inside one is text, not syntax; raw strings (`r#"..."#`) and multi-line
# strings are not understood and are the one layout this stripping misreads.
# The checks run one node per line: a node whose children are written on the
# following lines (`Mod+Shift+A {` / `spawn "swaylock" ...;` / `}`) is joined
# with them first, so a bind is read with its spawn whichever way it is laid
# out; a block whose children carry braces of their own (`binds {`) is not
# joined. Files pulled in with niri's `include` node are not followed: a bind
# or a startup line kept in an included file gives a false finding here, and
# a `noctalia` line there is not seen; the author's config uses no `include`.
# Exit 0 with `cutover-config-probe: OK`, 1 on a finding, 2 on usage.

usage() {
    echo "usage: scripts/cutover-config-probe.sh FILE   (a copy of a niri config)" >&2
    exit 2
}
[ "$#" -eq 1 ] || usage
[ -n "${HOME:-}" ] || {
    echo "cutover-config-probe: HOME is not set; the live config cannot be told apart" >&2
    usage
}
file=$1
case "$file" in
    /*) ;;
    *) file=$(pwd)/$file ;;
esac
[ -f "$file" ] && [ -r "$file" ] || {
    echo "cutover-config-probe: $file is not a readable file" >&2
    exit 2
}
# The canonical path of an existing file or directory (symlinks, `.`, `..`
# and `//` resolved), or nothing when it does not exist.
canonical() {
    if command -v realpath >/dev/null 2>&1; then
        realpath -e -- "$1" 2>/dev/null || true
    else
        readlink -f -- "$1" 2>/dev/null || true
    fi
}
real_file=$(canonical "$file")
[ -n "$real_file" ] || {
    echo "cutover-config-probe: $file cannot be resolved" >&2
    exit 2
}
for config_home in "$HOME/.config" "${XDG_CONFIG_HOME:-}"; do
    [ -n "$config_home" ] || continue
    real_home=$(canonical "$config_home")
    [ -n "$real_home" ] || continue
    case "$real_file" in
        "$real_home"/*)
            echo "cutover-config-probe: $file is under $config_home; pass a copy" >&2
            exit 2
            ;;
    esac
done

niri_binary=${CELESTINA_NIRI_PREFIX:-$HOME/.local/lib/celestina}/niri
if [ ! -x "$niri_binary" ]; then
    if command -v niri >/dev/null 2>&1; then
        niri_binary=niri
    else
        niri_binary=
    fi
fi
# From here on only the canonical path is read: `$file` was checked, and a
# symlink swapped since then must not reach the live file.
if [ -n "$niri_binary" ]; then
    if ! "$niri_binary" validate -c "$real_file" >/dev/null 2>&1; then
        echo "cutover-config-probe: $niri_binary validate refuses $file:" >&2
        "$niri_binary" validate -c "$real_file" >&2 || true
        exit 1
    fi
    echo "cutover-config-probe: $niri_binary validate accepts the file"
else
    echo "cutover-config-probe: niri is absent; the file was not validated"
fi

# The file as the compositor reads it: comments and slashdashed nodes
# removed (`strip`, a character state machine that follows quoted strings,
# nested block comments and the braces of a disabled node), with every
# newline kept so the lines stay in place. Then a node that opens a brace and
# closes it on a later line is joined with the lines that follow, unless the
# next line carries a brace of its own (the node is a block of nodes, like
# `binds {`, and stands alone), so each node the checks below read is on one
# line.
strip() {
    awk '
    # Modes: 0 code, 1 inside "..." (with backslash escapes), 2 inside a
    # block comment (depth counted), 3 skipping a slashdashed node to its end
    # (a newline or `;` outside its children block, or the brace that closes
    # that block), 4 skipping a slashdashed block to its closing brace.
    BEGIN { mode = 0; depth = 0; esc = 0 }
    {
        line = $0
        out = ""
        n = length(line)
        i = 1
        while (i <= n) {
            c = substr(line, i, 1)
            two = substr(line, i, 2)
            if (mode == 1) {
                out = out c
                if (esc) esc = 0
                else if (c == "\\") esc = 1
                else if (c == "\"") mode = 0
                i++
            } else if (mode == 2) {
                if (two == "/*") { depth++; i += 2 }
                else if (two == "*/") {
                    depth--; i += 2
                    # Closed: back to the code, or to the node skip the
                    # comment interrupted.
                    if (depth == 0) { mode = (held != "") ? held : 0; held = "" }
                }
                else i++
            } else if (mode == 3 || mode == 4) {
                if (skip_str) {
                    if (skip_esc) skip_esc = 0
                    else if (c == "\\") skip_esc = 1
                    else if (c == "\"") skip_str = 0
                    i++
                } else if (two == "/*") {
                    # A comment inside the disabled node: skipped with it.
                    cdepth = 1; i += 2
                    while (i <= n && cdepth > 0) {
                        two = substr(line, i, 2)
                        if (two == "/*") { cdepth++; i += 2 }
                        else if (two == "*/") { cdepth--; i += 2 }
                        else i++
                    }
                    if (cdepth > 0) { held = mode; mode = 2; depth = cdepth }
                } else if (two == "//") {
                    i = n + 1
                } else if (c == "\"") {
                    skip_str = 1; i++
                } else if (c == "{") {
                    depth++; i++
                } else if (c == "}") {
                    depth--; i++
                    if (depth <= 0) { mode = 0; depth = 0 }
                } else if (c == ";" && depth == 0 && mode == 3) {
                    mode = 0; i++
                } else {
                    i++
                }
            } else {
                if (two == "/*") { mode = 2; depth = 1; i += 2 }
                else if (two == "//") { i = n + 1 }
                else if (two == "/-") {
                    i += 2
                    # Blanks between the slashdash and what it disables.
                    while (i <= n && substr(line, i, 1) ~ /[ \t]/) i++
                    if (substr(line, i, 1) == "{") { mode = 4; depth = 1; i++ }
                    else { mode = 3; depth = 0 }
                }
                else if (c == "\"") { mode = 1; esc = 0; out = out c; i++ }
                else { out = out c; i++ }
            }
        }
        # The end of the line ends a slashdashed node that opened no block.
        if (mode == 3 && depth == 0) mode = 0
        print out
    }
    '
}
code=$(strip < "$real_file" | awk '
    function depth(s,    n, i, c) {
        n = 0
        for (i = 1; i <= length(s); i++) {
            c = substr(s, i, 1)
            if (c == "{") n++
            else if (c == "}") n--
        }
        return n
    }
    {
        d = depth($0)
        if (pending != "") {
            if (index($0, "{") > 0) {
                print pending
                pending = ""
            } else {
                pending = pending " " $0
                open += d
                if (open <= 0) {
                    print pending
                    pending = ""
                }
                next
            }
        }
        if (d > 0) {
            pending = $0
            open = d
            next
        }
        print
    }
    END { if (pending != "") print pending }
' || true)
findings=0
finding() {
    echo "cutover-config-probe: $1" >&2
    findings=$((findings + 1))
}

spawns=$(printf '%s\n' "$code" | grep -E '(^|[[:space:]{])spawn(-at-startup|-sh|-sh-at-startup)?[[:space:]]' || true)
if printf '%s\n' "$spawns" | grep -q 'noctalia'; then
    finding "a spawn line still names noctalia: $(printf '%s\n' "$spawns" | grep -c 'noctalia') line(s)"
fi

recovery=$(printf '%s\n' "$code" | grep -E '^[[:space:]]*Mod\+Shift\+A([[:space:]]|\{)' || true)
if [ -z "$recovery" ]; then
    finding "the Mod+Shift+A bind is missing (the swaylock recovery path)"
elif ! printf '%s\n' "$recovery" | grep -q '"swaylock"'; then
    finding "the Mod+Shift+A bind does not spawn swaylock"
fi

startup=$(printf '%s\n' "$code" | grep -E '^[[:space:]]*spawn(-sh)?-at-startup[[:space:]]' || true)
if printf '%s\n' "$startup" | grep -Eq 'caffeine-enable|nightlight-force-toggle'; then
    finding "a startup line still forces caffeine or the night light through noctalia"
fi
if ! printf '%s\n' "$startup" | grep -Eq '(^|[^A-Za-z0-9_-])celestina([^A-Za-z0-9_-]|$)'; then
    finding "no spawn-at-startup line starts celestina"
fi

[ "$findings" -eq 0 ] || {
    echo "cutover-config-probe: $findings finding(s) in $file" >&2
    exit 1
}
echo "cutover-config-probe: OK"
