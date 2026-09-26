#!/bin/sh
set -eu

# Open or close the session worktree of one ledger unit.
#
#   worktree.sh open PROJECT UNIT
#       Fetch origin/main, create the branch unit/<project>/<unit> from it and
#       add its worktree at <parent>/<basename>.worktrees/<project>-<unit>/,
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
#       then remove the worktree and delete the branch.
#
# Exit 2 on usage errors and 1 on a refusal, with one line on stderr.

usage() {
    printf '%s\n' 'usage: scripts/worktree.sh open|close PROJECT UNIT' >&2
    exit 2
}

refuse() {
    printf 'worktree: %s\n' "$*" >&2
    exit 1
}

[ "$#" -eq 3 ] || usage
command=$1
project=$2
unit=$3
case $command in
    open | close) ;;
    *) usage ;;
esac

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
inventory_root=$(python3 - "$registry" "$project" <<'EOF'
import sys
import tomllib

registry_path, project = sys.argv[1], sys.argv[2]
try:
    with open(registry_path, "rb") as handle:
        registry = tomllib.load(handle)
except (OSError, tomllib.TOMLDecodeError) as error:
    print(f"worktree: cannot read {registry_path}: {error}", file=sys.stderr)
    sys.exit(2)
projects = registry.get("projects", [])
if not isinstance(projects, list):
    print(f"worktree: {registry_path} has no [[projects]] list", file=sys.stderr)
    sys.exit(2)
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

# open starts from the newest origin/main, and close must see a landing made
# from any clone.
git -C "$repo_root" fetch --quiet origin main \
    || refuse "cannot fetch main from origin"

if [ "$command" = open ]; then
    [ ! -e "$unit_dir" ] || refuse "worktree already exists: $unit_dir"
    if git -C "$repo_root" show-ref --verify --quiet "refs/heads/$branch"; then
        refuse "branch already exists: $branch"
    fi
    mkdir -p -- "$worktrees"
    git -C "$repo_root" worktree add --quiet --no-track -b "$branch" \
        "$unit_dir" origin/main || refuse "cannot add the worktree $unit_dir"
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
