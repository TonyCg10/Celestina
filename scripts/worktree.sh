#!/bin/sh
set -eu

# Open or close the session worktree of one ledger unit.
#
#   worktree.sh open PROJECT UNIT [--from BRANCH]
#       Fetch origin/main, create the branch unit/<project>/<unit> from it, or
#       from the local branch BRANCH for a unit stacked on another unit's
#       branch, and add its worktree at
#       <parent>/<basename>.worktrees/<project>-<unit>/,
#       outside the repository. Inside it, write the untracked files
#       .cargo/config.toml (one Cargo target directory shared by every session)
#       and .celestina-worktree (the marker production_artifact.py refuses
#       production runs under), and exclude both through the repository's
#       shared info/exclude so that a session never stages them.
#   worktree.sh close PROJECT UNIT
#       Refuse while the worktree holds uncommitted changes, or while the
#       branch has commits that are not on origin/main and origin/main tracks
#       no inventory <root>/<plan-slug>/<UNIT>.numstat.tsv, where <root> is
#       docs/inventories for the suite and <path>/docs/inventories for a
#       project registered with that path (the landing publishes a squashed
#       commit, so after a landing only its inventory proves the unit landed);
#       also refuse while the branch holds a commit made after the landing
#       commit that added that inventory, which the landing never saw;
#       then remove the worktree and delete the branch. When no other session
#       worktree remains, also remove the shared session Cargo target, a cache
#       the next session's first build regenerates.
#
# A fetch that takes longer than CELESTINA_NETWORK_TIMEOUT seconds (300 by
# default) is a refusal, not a hang.
#
# Exit 2 on usage errors and 1 on a refusal, with one line on stderr.

usage() {
    printf '%s\n' 'usage: scripts/worktree.sh open PROJECT UNIT [--from BRANCH] | close PROJECT UNIT' >&2
    exit 2
}

refuse() {
    printf 'worktree: %s\n' "$*" >&2
    exit 1
}

[ "$#" -eq 3 ] || [ "$#" -eq 5 ] || usage
command=$1
project=$2
unit=$3
from=
case $command in
    open | close) ;;
    *) usage ;;
esac
if [ "$#" -eq 5 ]; then
    [ "$command" = open ] && [ "$4" = --from ] && [ -n "$5" ] || usage
    from=$5
fi

# Both ids become path and branch components; accept only ledger-shaped ids.
for identifier in "$project" "$unit"; do
    case $identifier in
        '' | [!A-Za-z0-9]* | *[!A-Za-z0-9._-]*)
            refuse "invalid identifier: '$identifier'"
            ;;
    esac
done

repo_root=$(git rev-parse --show-toplevel 2>/dev/null) \
    || refuse "not inside a Git checkout"
marker_name=.celestina-worktree
[ ! -e "$repo_root/$marker_name" ] \
    || refuse "this is a session worktree; run this from the canonical checkout"

registry=$repo_root/docs/projects.toml
registry_status=0
# Prints the owner's inventory root; exit 1 names an unregistered project.
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
inventory_root=$(python3 - "$registry" "$project" "$script_dir" <<'EOF'
import sys

registry_path, project, scripts = sys.argv[1], sys.argv[2], sys.argv[3]
sys.path.insert(0, scripts)
from pathlib import Path

try:
    from project_registry import load_registry
except ImportError as error:
    print(f"worktree: cannot load the registry reader: {error}", file=sys.stderr)
    sys.exit(2)

try:
    registry = load_registry(Path(registry_path))
except ValueError as error:
    print(f"worktree: {error}", file=sys.stderr)
    sys.exit(2)
projects = registry["projects"]
if project == "suite":
    print("docs/inventories")
    sys.exit(0)
for entry in projects:
    if isinstance(entry, dict) and entry.get("id") == project:
        path = entry.get("path")
        if not isinstance(path, str) or not path:
            print(f"worktree: {project} registers no path in {registry_path}", file=sys.stderr)
            sys.exit(2)
        print(f"{path.rstrip('/')}/docs/inventories")
        sys.exit(0)
sys.exit(1)
EOF
) || registry_status=$?
case $registry_status in
    0) ;;
    1) refuse "unregistered project: $project" ;;
    *) exit 1 ;;
esac

repo_parent=$(dirname -- "$repo_root")
repo_name=$(basename -- "$repo_root")
worktrees=$repo_parent/$repo_name.worktrees
unit_dir=$worktrees/$project-$unit
branch=unit/$project/$unit

network_timeout=${CELESTINA_NETWORK_TIMEOUT:-300}
case $network_timeout in
    '' | *[!0-9]*) network_timeout=300 ;;
esac
remote() {
    # A stalled remote must not hang the session (TOOL-19); coreutils'
    # timeout ends it where it exists.
    if command -v timeout >/dev/null 2>&1; then
        timeout "$network_timeout" "$@"
    else
        "$@"
    fi
}

# open starts from the newest origin/main, and close must see a landing made
# from any clone.
remote git -C "$repo_root" fetch --quiet origin main \
    || refuse "cannot fetch main from origin (or it took over $network_timeout s)"

if [ "$command" = open ]; then
    [ ! -e "$unit_dir" ] || refuse "worktree already exists: $unit_dir"
    if git -C "$repo_root" show-ref --verify --quiet "refs/heads/$branch"; then
        refuse "branch already exists: $branch"
    fi
    start=origin/main
    if [ -n "$from" ]; then
        # A stacked unit starts from its dependency's local branch; the
        # landing accepts it once that dependency has landed.
        git -C "$repo_root" show-ref --verify --quiet "refs/heads/$from" \
            || refuse "no such branch: $from"
        start=refs/heads/$from
    fi
    mkdir -p -- "$worktrees"
    git -C "$repo_root" worktree add --quiet --no-track -b "$branch" \
        "$unit_dir" "$start" || refuse "cannot add the worktree $unit_dir"
    # info/exclude lives in the common Git directory, so it covers every
    # worktree and the canonical checkout, where neither file exists.
    common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute \
        --git-common-dir) || refuse "cannot locate the common Git directory"
    exclude_file=$common_dir/info/exclude
    mkdir -p -- "$common_dir/info"
    # A last byte other than a newline would glue the next pattern onto it.
    if [ -s "$exclude_file" ] && [ -n "$(tail -c 1 -- "$exclude_file")" ]; then
        printf '\n' >> "$exclude_file"
    fi
    for pattern in /.cargo/config.toml "/$marker_name"; do
        if [ ! -f "$exclude_file" ] \
            || ! grep -q -F -x -- "$pattern" "$exclude_file"; then
            printf '%s\n' "$pattern" >> "$exclude_file"
        fi
    done
    mkdir -p -- "$unit_dir/.cargo"
    printf '[build]\ntarget-dir = "%s"\n' "$worktrees/.cargo-target" \
        > "$unit_dir/.cargo/config.toml"
    printf 'project = "%s"\nunit = "%s"\ncanonical = "%s"\n' \
        "$project" "$unit" "$repo_root" > "$unit_dir/$marker_name"
    printf '%s\n' "$unit_dir"
    exit 0
fi

git -C "$repo_root" show-ref --verify --quiet "refs/heads/$branch" \
    || refuse "no such branch: $branch"
pending=$(git -C "$repo_root" rev-list origin/main.."$branch") \
    || refuse "cannot compare $branch with origin/main"
if [ -n "$pending" ]; then
    # Only the owner's own inventory root counts: another project's unit or a
    # tracked fixture may carry the same unit id.
    tracked=$(git -C "$repo_root" ls-tree -r --name-only origin/main -- \
        "$inventory_root/") || refuse "cannot list the files of origin/main"
    landed=
    while IFS= read -r path; do
        case ${path#"$inventory_root"/} in
            */*/*) ;;
            */"$unit.numstat.tsv")
                landed=$path
                break
                ;;
        esac
    done <<EOF
$tracked
EOF
    [ -n "$landed" ] || refuse "$branch has commits that are not on origin/main" \
        "and origin/main has no inventory $inventory_root/<plan>/$unit.numstat.tsv"
    # The landing sealed what the branch held then; a commit made on the
    # branch after that commit would be lost with the branch.
    landing_commit=$(git -C "$repo_root" log -1 --format=%H --diff-filter=A \
        origin/main -- "$landed") || refuse "cannot find the commit that landed $landed"
    landing_time=$(git -C "$repo_root" log -1 --format=%ct "$landing_commit") \
        || refuse "cannot read the time of $landing_commit"
    later=$(git -C "$repo_root" log --format='%H %ct' origin/main.."$branch" \
        | awk -v landed="$landing_time" '$2 > landed { print substr($1, 1, 12) }') \
        || refuse "cannot compare $branch with the landing commit"
    [ -z "$later" ] || refuse "$branch has commits made after its landing" \
        "($(printf '%s' "$later" | tr '\n' ' ' | sed 's/ $//')); keep them on another" \
        "branch, or delete $branch by hand"
fi
if [ -d "$unit_dir" ]; then
    # Only the two files this entry wrote may be left behind; anything else
    # is session work, and the marker stays until nothing else remains.
    changes=$(git -C "$unit_dir" status --porcelain --untracked-files=all) \
        || refuse "cannot read the status of $unit_dir"
    foreign=$(printf '%s\n' "$changes" \
        | grep -v -x -e "?? $marker_name" -e '?? .cargo/config.toml' || :)
    [ -z "$foreign" ] || refuse "the worktree $unit_dir has uncommitted changes"
    rm -f -- "$unit_dir/$marker_name" "$unit_dir/.cargo/config.toml"
    rmdir -- "$unit_dir/.cargo" 2>/dev/null || :
    git -C "$repo_root" worktree remove "$unit_dir" \
        || refuse "cannot remove the worktree $unit_dir"
fi
git -C "$repo_root" branch --quiet -D "$branch" \
    || refuse "cannot delete the branch $branch"

# The shared Cargo target serves open sessions only; the landing worktree
# never builds, so it does not keep the cache alive.
session=$(find "$worktrees" -mindepth 1 -maxdepth 1 ! -name .cargo-target \
    ! -name .landing -print 2>/dev/null | head -n 1)
if [ -z "$session" ] && [ -d "$worktrees/.cargo-target" ]; then
    rm -rf -- "$worktrees/.cargo-target" \
        || refuse "cannot remove the session Cargo target $worktrees/.cargo-target"
fi
