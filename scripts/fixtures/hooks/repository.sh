# Shared fixture of the repository hooks, sourced by test-staged-units.sh and
# test-commit-scope.sh.
#
#   make_hook_repository DIRECTORY SCRIPTS_DIRECTORY HOOKS_DIRECTORY
#
# Builds a Git repository at DIRECTORY from the valid documentation fixture,
# with the guard modules of SCRIPTS_DIRECTORY committed in its first commit
# and the hooks of HOOKS_DIRECTORY enabled through core.hooksPath, so that a
# commit there runs the real hooks against rules committed in HEAD.

make_hook_repository() {
    hook_repository=$1
    hook_scripts=$2
    hook_wrappers=$3
    cp -R "$hook_scripts/fixtures/documentation/valid" "$hook_repository"
    # The fixture's closed unit points at a commit this repository never had.
    rm -rf -- "$hook_repository/app/docs/inventories"
    sed -i '/| APP-1B |/d' "$hook_repository/app/docs/plans/active/2026-08-03-app.md"
    sed -i 's/`APP-1B`/`APP-1A`/' "$hook_repository/app/VALIDATION.md"
    # The staged-unit guard needs a plan root for every owner.
    sed -i '/^path = "core"/a active_plans = "core/docs/plans/active"' \
        "$hook_repository/docs/projects.toml"
    mkdir -p "$hook_repository/core/docs/plans/active" \
        "$hook_repository/core/docs/plans/archive" \
        "$hook_repository/scripts" "$hook_repository/.githooks"
    printf '# Active core plans\n\nNone.\n' > "$hook_repository/core/docs/plans/active/README.md"
    printf '# Archived core plans\n\nNone.\n' > "$hook_repository/core/docs/plans/archive/README.md"
    for module in "$hook_scripts"/*.py; do
        case ${module##*/} in
            test-*) ;;
            *) cp "$module" "$hook_repository/scripts/" ;;
        esac
    done
    cp "$hook_scripts/check-documentation-contract.sh" "$hook_repository/scripts/"
    printf '%s\n' '# Legacy non-English line ratchet.' '# suspicious_lines<TAB>path' \
        > "$hook_repository/scripts/language-baseline.tsv"
    printf '%s\n' '# Format: class<TAB>key<TAB>current maximum' \
        > "$hook_repository/scripts/architecture-baseline.tsv"
    cp "$hook_wrappers"/* "$hook_repository/.githooks/"
    git -C "$hook_repository" init -q
    git -C "$hook_repository" config user.name "Hook Fixture"
    git -C "$hook_repository" config user.email "hooks@example.invalid"
    git -C "$hook_repository" add -A
    git -C "$hook_repository" -c core.hooksPath=/dev/null commit -qm \
        'suite-maintenance: Establish the hook fixture'
    git -C "$hook_repository" config core.hooksPath .githooks
}
