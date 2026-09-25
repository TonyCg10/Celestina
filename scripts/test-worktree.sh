#!/bin/sh
set -eu

# Fixture tests for scripts/worktree.sh: a repository with a registry and a
# bare origin, both inside one temporary directory that is removed on exit.

suite_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
worktree_entry=$suite_root/scripts/worktree.sh
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM
# Git reports physical paths; compare against the same spelling.
temporary=$(CDPATH= cd -- "$temporary" && pwd -P)

fail() {
    printf 'test-worktree: FAIL: %s\n' "$*" >&2
    exit 1
}

origin=$temporary/origin.git
repo=$temporary/repo
worktrees=$temporary/repo.worktrees
stdout_file=$temporary/stdout
stderr_file=$temporary/stderr

git init -q --bare "$origin"
git init -q "$repo"
git -C "$repo" symbolic-ref HEAD refs/heads/main
git -C "$repo" config user.name "Worktree Fixture"
git -C "$repo" config user.email "fixture@example.invalid"
# Keep the fixture independent of the caller's signing and push settings.
git -C "$repo" config commit.gpgsign false
git -C "$repo" config push.negotiate false
mkdir -p "$repo/docs"
{
    printf 'schema_version = 1\n\n'
    printf '[suite]\n'
    printf 'id = "suite"\n'
    printf 'commit_prefix = "suite"\n'
    printf 'active_plans = "docs/plans/active"\n'
    printf 'allow_all_commit_paths = true\n\n'
    printf '[commit_policy]\n'
    printf 'workspace_manifests = []\n\n'
    printf '[[projects]]\n'
    printf 'id = "app"\n'
    printf 'commit_prefix = "app"\n'
    printf 'path = "app"\n'
    printf 'active_plans = "app/docs/plans/active"\n'
    printf 'commit_roots = ["app/"]\n'
    printf 'include_workspace_manifests = false\n'
} > "$repo/docs/projects.toml"
git -C "$repo" add docs/projects.toml
git -C "$repo" commit -qm "fixture: establish the registry"
git -C "$repo" remote add origin "$origin"
git -C "$repo" push -q origin main

# Runs the entry from inside the canonical fixture checkout and records its
# exit status, stdout and stderr.
run_entry() {
    entry_status=0
    (cd "$repo" && sh "$worktree_entry" "$@") >"$stdout_file" 2>"$stderr_file" \
        || entry_status=$?
}

expect_status() {
    [ "$entry_status" -eq "$1" ] \
        || fail "$2: exit $entry_status instead of $1: $(cat "$stderr_file")"
}

expect_stderr() {
    grep -F -- "$1" "$stderr_file" >/dev/null \
        || fail "$2: stderr lacks '$1': $(cat "$stderr_file")"
}

# 1. Opening a unit creates its branch, worktree, marker and Cargo config.
run_entry open app APP-1
expect_status 0 "open app APP-1"
unit_dir=$worktrees/app-APP-1
git -C "$repo" worktree list --porcelain > "$temporary/list"
grep -Fx "worktree $unit_dir" "$temporary/list" >/dev/null \
    || fail "open app APP-1: worktree list lacks $unit_dir"
grep -Fx "branch refs/heads/unit/app/APP-1" "$temporary/list" >/dev/null \
    || fail "open app APP-1: worktree list lacks unit/app/APP-1"
grep -Fx 'project = "app"' "$unit_dir/.celestina-worktree" >/dev/null \
    || fail "open app APP-1: marker lacks the project"
grep -Fx 'unit = "APP-1"' "$unit_dir/.celestina-worktree" >/dev/null \
    || fail "open app APP-1: marker lacks the unit"
grep -Fx "canonical = \"$repo\"" "$unit_dir/.celestina-worktree" >/dev/null \
    || fail "open app APP-1: marker lacks the canonical checkout"
grep -Fx "target-dir = \"$worktrees/.cargo-target\"" "$unit_dir/.cargo/config.toml" \
    >/dev/null || fail "open app APP-1: Cargo config lacks the shared target directory"
[ "$(git -C "$unit_dir" rev-parse HEAD)" = "$(git -C "$repo" rev-parse origin/main)" ] \
    || fail "open app APP-1: HEAD is not origin/main"
printf 'ok %s\n' "open creates the unit branch, worktree, marker and Cargo config"

# 2. Opening the same unit again is refused.
run_entry open app APP-1
expect_status 1 "second open app APP-1"
expect_stderr "already exists" "second open app APP-1"
printf 'ok %s\n' "open refuses a unit that already has a worktree"

# 3. An unregistered project is refused.
run_entry open nope APP-2
expect_status 1 "open nope APP-2"
expect_stderr "unregistered project" "open nope APP-2"
[ ! -e "$worktrees/nope-APP-2" ] || fail "open nope APP-2: created a worktree"
printf 'ok %s\n' "open refuses an unregistered project"

# 4. Closing is refused while the branch has work that is not on origin/main.
printf 'change\n' > "$unit_dir/change.txt"
git -C "$unit_dir" add change.txt
git -C "$unit_dir" commit -qm "fixture: unit work"
run_entry close app APP-1
expect_status 1 "close app APP-1 with unpublished work"
expect_stderr "not on origin/main" "close app APP-1 with unpublished work"
[ -d "$unit_dir" ] || fail "close app APP-1: removed a worktree with unpublished work"
# Fixture only: publish the unit so that the branch is contained in origin/main.
git -C "$unit_dir" push -q origin HEAD:main
run_entry close app APP-1
expect_status 0 "close app APP-1 after publication"
[ ! -e "$unit_dir" ] || fail "close app APP-1: the worktree still exists"
if git -C "$repo" show-ref --verify --quiet refs/heads/unit/app/APP-1; then
    fail "close app APP-1: the branch still exists"
fi
printf 'ok %s\n' "close refuses unpublished work and removes a published unit"

# 5. The suite is a valid project id.
run_entry open suite LND-9
expect_status 0 "open suite LND-9"
[ -f "$worktrees/suite-LND-9/.celestina-worktree" ] \
    || fail "open suite LND-9: marker is missing"
printf 'ok %s\n' "open accepts the suite"

# 6. The two written files are excluded through the shared info/exclude, so the
#    worktree stays clean, and a later open does not duplicate the patterns.
[ -z "$(git -C "$worktrees/suite-LND-9" status --porcelain --untracked-files=all)" ] \
    || fail "open suite LND-9: the worktree is not clean"
exclude_file=$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)/info/exclude
for pattern in /.cargo/config.toml /.celestina-worktree; do
    count=$(grep -c -F -x -- "$pattern" "$exclude_file" || :)
    [ "$count" = 1 ] || fail "info/exclude lists $pattern $count times instead of once"
done
printf 'ok %s\n' "open excludes its own files once and leaves the worktree clean"
