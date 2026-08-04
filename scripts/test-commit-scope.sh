#!/bin/sh
set -eu

# Commit-scope guard tests (`.githooks/commit-msg`).
#
# The first of these two halves matters most: compare the guard with real
# history, not only invented examples. A guard that only passes its own fixtures
# has not been tested against actual repository practice; history reveals when
# a rule is too strict.

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)
hook=$root/.githooks/commit-msg

failures=0
fail() {
    printf 'FAIL: %s\n' "$1" >&2
    failures=$((failures + 1))
}

[ -x "$hook" ] || fail "$hook is not executable"

expect_scope() {
    # expect_scope <expected: pass|fail> <subject> <files...>
    expected=$1
    subject=$(fixture_subject "$2")
    shift 2
    if printf '%s\n' "$@" | sh "$hook" --check "$subject" >/dev/null 2>&1; then
        actual=pass
    else
        actual=fail
    fi
    if [ "$actual" != "$expected" ]; then
        fail "expected $expected, got $actual -> \"$subject\" with: $*"
    fi
}

# The policy migration itself is interpreted by the previous HEAD and therefore
# still uses the legacy base subject. Once version_policy is committed, keep the
# same scope fixtures but add the required maintenance kind automatically.
typed_subjects=false
if git -C "$root" show HEAD:docs/projects.toml 2>/dev/null | \
    grep -q '^\[version_policy\]$'; then
    typed_subjects=true
fi

fixture_subject() {
    raw=$1
    if [ "$typed_subjects" != true ]; then
        printf '%s\n' "$raw"
        return
    fi
    case $raw in
        'fixup! '*|'squash! '*|'amend! '*)
            marker=${raw%%!*}'! '
            inner=${raw#*! }
            printf '%s%s\n' "$marker" "$(fixture_subject "$inner")"
            return
            ;;
        'Revert "'*'"')
            inner=${raw#Revert \"}
            inner=${inner%\"}
            printf 'Revert "%s"\n' "$(fixture_subject "$inner")"
            return
            ;;
    esac
    case $raw in
        *': '*)
            prefix=${raw%%:*}
            remainder=${raw#*:}
            case $prefix in
                *-bug|*-milestone|*-release|*-maintenance) ;;
                *) prefix=$prefix-maintenance ;;
            esac
            printf '%s:%s\n' "$prefix" "$remainder"
            ;;
        *) printf '%s\n' "$raw" ;;
    esac
}

# 1. Real history
# Every commit since adoption of the convention must pass. The range starts at
# the first commit written under this rule; older commits predate the contract.
# The anchor is overridable so the fixture can prove this loop fails when aimed
# at history that predates the convention.
first_commit=${COMMIT_SCOPE_START:-9ecc457}
if git -C "$root" rev-parse -q --verify "$first_commit^{commit}" >/dev/null 2>&1; then
    if ! git -C "$root" rev-parse -q --verify "$first_commit^" >/dev/null 2>&1; then
        fail "historical anchor $first_commit has no verifiable parent"
    else
        git -C "$root" log --format='%H' "$first_commit^..HEAD" | while read -r commit; do
        subject=$(git -C "$root" log -1 --format='%s' "$commit")
        # A merge does not declare its own scope.
        if [ "$(git -C "$root" rev-list --parents -n1 "$commit" | wc -w)" -gt 2 ]; then
            continue
        fi
        files=$(git -C "$root" show --no-renames --name-only --format='' "$commit" | grep -v '^$' || true)
        [ -n "$files" ] || continue
        if ! printf '%s\n' "$files" | sh "$hook" \
            --history-scope-only "$subject" >/dev/null 2>&1; then
            printf 'FAIL: history does not pass its own guard: %s %s\n' \
                "$(echo "$commit" | cut -c1-7)" "$subject" >&2
            printf '%s\n' "$files" | sh "$hook" \
                --history-scope-only "$subject" >&2 || true
            exit 1
        fi
        done || fail "a historical commit does not pass the guard"
    fi
else
    fail "historical anchor $first_commit does not exist; update it explicitly"
fi

# 2. Fixtures

# The guard exists to catch a subject whose prefix lies about its paths.
expect_scope fail 'siderita: update two applications' 'siderita/src/main.rs' 'celestina/src/main.cpp'
expect_scope fail 'grafita: update two applications' 'grafita/src/main.rs' 'fluorita/src/main.rs'
expect_scope fail 'celestina-style: update a foreign surface' 'celestina-style/qmldir' 'siderita/qml/Main.qml'
# One core cannot enter through another core's prefix.
expect_scope fail 'grafita-core: update a foreign core' 'celestina-rs/crates/fluorita-core/src/lib.rs'
# Original failure case: the entire tree under an arbitrary prefix.
expect_scope fail 'siderita: collect unrelated changes' \
    'siderita/src/main.rs' 'celestina/src/main.cpp' 'grafita/src/main.rs' \
    'celestina-style/qmldir' 'README.md'

# Legitimate changes must remain unobstructed.
expect_scope pass 'siderita: update one surface' 'siderita/src/main.rs' 'siderita/qml/Main.qml'
expect_scope pass 'suite: align two projects' 'siderita/src/main.rs' 'celestina/src/main.cpp'
# An application and its own core land together.
expect_scope pass 'magnetita: update its clipboard flow' \
    'magnetita/ROADMAP.md' 'celestina-rs/crates/magnetita-core/src/clipboard.rs'
expect_scope pass 'grafita: update its save flow' 'grafita/src/main.rs' 'celestina-rs/crates/grafita-core/src/save.rs'
# Registering a crate also edits the workspace manifest in the same commit.
expect_scope pass 'fluorita-core: register one crate' \
    'celestina-rs/crates/fluorita-core/src/lib.rs' 'celestina-rs/Cargo.toml' 'celestina-rs/Cargo.lock'
expect_scope fail 'fluorita-core: reject a manifest backup' \
    'celestina-rs/crates/fluorita-core/src/lib.rs' 'celestina-rs/Cargo.toml.backup'
# The primary prefix closes a unit with its local records.
expect_scope pass 'siderita: close one ledger unit' \
    'celestina-rs/crates/siderita-core/src/lib.rs' \
    'siderita/ROADMAP.md' 'siderita/STATUS.md' \
    'siderita/docs/plans/active/2026-08-03-unit.md' \
    'siderita/docs/evidence/2026-08-03-unit.md'
# A component prefix does not own its owner's persistent records.
expect_scope fail 'siderita-core: close one ledger unit' \
    'celestina-rs/crates/siderita-core/src/lib.rs' \
    'siderita/docs/plans/active/2026-08-03-unit.md'
expect_scope fail 'siderita: cross into another owner records' \
    'siderita/src/main.rs' 'magnetita/docs/plans/active/2026-08-03-unit.md'
# Roots are boundaries, not approximate lexical prefixes.
expect_scope fail 'grafita: reject a lookalike crate' \
    'celestina-rs/crates/grafita-evil/src/lib.rs'
# --no-renames gives the guard both source and destination of a move.
expect_scope fail 'siderita: move a foreign file into scope' \
    'celestina/src/foreign.cpp' 'siderita/src/foreign.cpp'
# The shell and its core share a prefix.
expect_scope pass 'celestina: update the shell core' \
    'celestina/src/main.cpp' 'celestina-rs/crates/celestina-shell-core/src/lib.rs'
# Ratchet ownership is exercised below in a repository whose HEAD already
# contains the new policy. The current checkout's HEAD may predate this worktree
# migration, and scope replay intentionally uses only committed rules.
# The ratchet row is the only shared exception; the guards themselves are not.
expect_scope fail 'siderita: reach into a suite guard' \
    'siderita/src/main.rs' 'scripts/check-architecture-contract.sh'

# Path scope is not enough for a shared ratchet. Exercise staged row ownership
# in an isolated repository so a local prefix cannot edit or remove foreign
# debt while still allowing the source and its earned reduction to land once.
ratchet_tmp=$(mktemp -d) || {
    fail "could not create the ratchet fixture repository"
    ratchet_tmp=
}
if [ -n "$ratchet_tmp" ]; then
    trap 'rm -R -- "$ratchet_tmp"' EXIT
    mkdir -p "$ratchet_tmp/docs/evidence" "$ratchet_tmp/scripts" \
        "$ratchet_tmp/siderita/qml/views" "$ratchet_tmp/siderita/docs/evidence" \
        "$ratchet_tmp/celestina-rs/crates/siderita-core/src"
    cp "$root/docs/projects.toml" "$ratchet_tmp/docs/projects.toml"
    # This fixture exercises ratchets, not the independently covered version
    # transition. Keeping its committed registry pre-adoption avoids inventing
    # product manifests in the temporary repository.
    sed -i '/^\[version_policy\]$/,/^$/d' \
        "$ratchet_tmp/docs/projects.toml"
    cp "$root/scripts/commit_scope.py" \
        "$root/scripts/project_registry.py" \
        "$root/scripts/architecture_scanners.py" \
        "$root/scripts/check-language-contract.py" \
        "$root/scripts/check-staged-units.py" \
        "$root/scripts/documentation_contract.py" \
        "$ratchet_tmp/scripts/"
    printf '# Temporary architecture debt.\nlines\tsiderita/qml/views/FolderView.qml\t3\nlines\tcelestina-rs/crates/siderita-core/src/lib.rs\t3\ncontrol\tsiderita/qml/views/Controls.qml:IndexOnlyControl\t2\n' \
        > "$ratchet_tmp/scripts/architecture-baseline.tsv"
    printf '# Temporary language debt.\n2\tsiderita/qml/views/FolderView.qml\n' \
        > "$ratchet_tmp/scripts/language-baseline.tsv"
    # Deliberate non-English fixture data is assembled from fragments so the
    # repository-level detector does not mistake its own negative test for debt.
    fixture_file_word=arch
    fixture_file_word=${fixture_file_word}ivo
    fixture_test_word=prue
    fixture_test_word=${fixture_test_word}ba
    fixture_change_word=cam
    fixture_change_word=${fixture_change_word}bio
    fixture_rules_word=reg
    fixture_rules_word=${fixture_rules_word}las
    fixture_debt_one="$fixture_file_word $fixture_test_word"
    fixture_debt_two="$fixture_change_word $fixture_rules_word"
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" 'line three' \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    printf '%s\n' 'line one' 'line two' 'line three' \
        > "$ratchet_tmp/celestina-rs/crates/siderita-core/src/lib.rs"
    printf '%s\n' 'IndexOnlyControl {' '}' 'IndexOnlyControl {' '}' \
        > "$ratchet_tmp/siderita/qml/views/Controls.qml"
    git -C "$ratchet_tmp" init -q
    git -C "$ratchet_tmp" config user.name Fixture
    git -C "$ratchet_tmp" config user.email fixture@example.invalid
    git -C "$ratchet_tmp" config core.hooksPath /dev/null
    git -C "$ratchet_tmp" add .
    git -C "$ratchet_tmp" commit -qm 'fixture: establish ratchets'

    reset_ratchet_fixture() {
        rm -f -- "$ratchet_tmp/.git/MERGE_HEAD" "$ratchet_tmp/.git/MERGE_MSG"
        git -C "$ratchet_tmp" reset --hard -q HEAD
        git -C "$ratchet_tmp" clean -fdq
    }
    check_ratchets() {
        python3 "$ratchet_tmp/scripts/commit_scope.py" --root "$ratchet_tmp" \
            --check-ratchets "$1" >/dev/null 2>&1
    }
    check_index() {
        python3 "$ratchet_tmp/scripts/commit_scope.py" --root "$ratchet_tmp" \
            --check-index "$1" >/dev/null 2>&1
    }
    check_merge() {
        git -C "$ratchet_tmp" rev-parse HEAD > "$ratchet_tmp/.git/MERGE_HEAD"
        printf '%s\n' 'Merge fixture' > "$ratchet_tmp/.git/MERGE_MSG"
        python3 "$ratchet_tmp/scripts/commit_scope.py" --root "$ratchet_tmp" \
            "$ratchet_tmp/.git/MERGE_MSG" >/dev/null 2>&1
    }

    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i 's/FolderView.qml\t3/FolderView.qml\t2/' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    git -C "$ratchet_tmp" add .
    check_ratchets siderita || fail "an earned architecture reduction was rejected"

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i '/^lines\tsiderita\/qml\/views\/FolderView.qml\t/d' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    git -C "$ratchet_tmp" add .
    if check_ratchets siderita; then
        fail "a lines row disappeared without durable resolution evidence"
    fi

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" 'line changed' \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i '/^lines\tsiderita\/qml\/views\/FolderView.qml\t/d' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    mkdir -p "$ratchet_tmp/siderita/docs/evidence"
    printf '%s\n' '# Architecture resolution' '' \
        '- **Resolved architecture debt:** `siderita/qml/views/FolderView.qml`' \
        > "$ratchet_tmp/siderita/docs/evidence/refactor.md"
    git -C "$ratchet_tmp" add .
    check_ratchets siderita || fail "a documented architecture resolution was rejected"

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" 'line changed' \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i '/^lines\tsiderita\/qml\/views\/FolderView.qml\t/d' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    mkdir -p "$ratchet_tmp/siderita/qml/views/docs/evidence"
    printf '%s\n' '# Fake nested evidence' '' \
        '- **Resolved architecture debt:** `siderita/qml/views/FolderView.qml`' \
        > "$ratchet_tmp/siderita/qml/views/docs/evidence/refactor.md"
    git -C "$ratchet_tmp" add .
    if check_ratchets siderita; then
        fail "nested project evidence bypassed the canonical evidence root"
    fi

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" 'line changed' \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i '/^lines\tsiderita\/qml\/views\/FolderView.qml\t/d' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    mkdir -p "$ratchet_tmp/docs/evidence"
    printf '%s\n' '# Suite architecture resolution' '' \
        '- **Resolved architecture debt:** `siderita/qml/views/FolderView.qml`' \
        > "$ratchet_tmp/docs/evidence/refactor.md"
    git -C "$ratchet_tmp" add .
    check_ratchets suite || fail "canonical suite resolution evidence was rejected"

    reset_ratchet_fixture
    printf '%s\n' 'line one' 'line two' 'line changed' \
        > "$ratchet_tmp/celestina-rs/crates/siderita-core/src/lib.rs"
    sed -i '/^lines\tcelestina-rs\/crates\/siderita-core\/src\/lib.rs\t/d' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    mkdir -p "$ratchet_tmp/celestina-rs/crates/siderita-core/docs/evidence"
    printf '%s\n' '# Fake component evidence' '' \
        '- **Resolved architecture debt:** `celestina-rs/crates/siderita-core/src/lib.rs`' \
        > "$ratchet_tmp/celestina-rs/crates/siderita-core/docs/evidence/refactor.md"
    git -C "$ratchet_tmp" add .
    if check_ratchets siderita-core; then
        fail "a component prefix retired debt through nested fake evidence"
    fi

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i 's/FolderView.qml\t3/FolderView.qml\t1/' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    git -C "$ratchet_tmp" add .
    if check_ratchets siderita; then
        fail "an architecture row did not match the staged source"
    fi

    reset_ratchet_fixture
    sed -i 's/FolderView.qml\t3/FolderView.qml\t2/' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    git -C "$ratchet_tmp" add .
    if check_ratchets siderita; then
        fail "an architecture ratchet changed without its source"
    fi

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i 's/FolderView.qml\t3/FolderView.qml\t2/' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    git -C "$ratchet_tmp" add .
    if check_ratchets magnetitad; then
        fail "a component prefix changed a foreign architecture row"
    fi

    # A staged registry is the future interpretation, never its own authority.
    # A normal commit must remain inside both the HEAD and INDEX scopes.
    reset_ratchet_fixture
    sed -i '/commit_prefix = "siderita"/,/^include_workspace_manifests/ s|commit_roots = \[|commit_roots = ["docs/", "scripts/", |' \
        "$ratchet_tmp/docs/projects.toml"
    printf '%s\n' '# Foreign suite policy' \
        > "$ratchet_tmp/scripts/foreign.py"
    git -C "$ratchet_tmp" add docs/projects.toml scripts/foreign.py
    if check_index 'siderita: expand my own authority'; then
        fail "a staged registry authorized its own broader project scope"
    fi

    # Python in INDEX is data for the next commit, never executable authority in
    # the current hook. Even a successful exit at module or callable scope must
    # therefore be ignored rather than caught after execution.
    reset_ratchet_fixture
    printf '%s\n' 'raise SystemExit(0)' \
        >> "$ratchet_tmp/scripts/architecture_scanners.py"
    git -C "$ratchet_tmp" add scripts/architecture_scanners.py
    git -C "$ratchet_tmp" show HEAD:scripts/architecture_scanners.py \
        > "$ratchet_tmp/scripts/architecture_scanners.py"
    check_index 'suite: ignore staged module execution' || \
        fail "a staged SystemExit module was executed"

    reset_ratchet_fixture
    printf '%s\n' '' \
        'def build_commit_scopes(registry):' \
        '    raise SystemExit(0)' \
        >> "$ratchet_tmp/scripts/project_registry.py"
    git -C "$ratchet_tmp" add scripts/project_registry.py
    git -C "$ratchet_tmp" show HEAD:scripts/project_registry.py \
        > "$ratchet_tmp/scripts/project_registry.py"
    check_index 'suite: ignore staged rule invocation' || \
        fail "a staged SystemExit callable was executed"

    reset_ratchet_fixture
    sed -i 's|siderita-core/src/lib.rs\t3|siderita-core/src/lib.rs\t4|' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    printf '%s\n' '' 'import os' 'os._exit(0)' \
        >> "$ratchet_tmp/scripts/architecture_scanners.py"
    git -C "$ratchet_tmp" add scripts/architecture-baseline.tsv \
        scripts/architecture_scanners.py
    git -C "$ratchet_tmp" show HEAD:scripts/architecture_scanners.py \
        > "$ratchet_tmp/scripts/architecture_scanners.py"
    if check_index 'suite: reject forbidden debt with staged exit'; then
        fail "staged os._exit(0) bypassed a forbidden ratchet increase"
    fi

    # Staged scanner semantics take effect only after the scanner lands. The
    # current commit remains measured by the implementation committed in HEAD.
    reset_ratchet_fixture
    printf '%s\n' 'IndexOnlyControl {' '}' \
        > "$ratchet_tmp/siderita/qml/views/Controls.qml"
    sed -i 's/Controls.qml:IndexOnlyControl\t2/Controls.qml:IndexOnlyControl\t1/' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    printf '%s\n' '' \
        'CONTROL = re.compile(r"^[ \t]*\b(IndexOnlyControl)[ \t\r\n]*\{", re.M)' \
        >> "$ratchet_tmp/scripts/architecture_scanners.py"
    git -C "$ratchet_tmp" add scripts/architecture_scanners.py \
        scripts/architecture-baseline.tsv siderita/qml/views/Controls.qml
    git -C "$ratchet_tmp" show HEAD:scripts/architecture_scanners.py \
        > "$ratchet_tmp/scripts/architecture_scanners.py"
    if check_ratchets suite; then
        fail "a staged architecture scanner changed current-commit measurement"
    fi

    reset_ratchet_fixture
    printf '%s\n' 'indexonlyword indexonlyword' \
        > "$ratchet_tmp/siderita/qml/views/IndexLanguage.qml"
    printf '%s\n' '' \
        'SPANISH_WORDS = re.compile(r"\bindexonlyword\b", re.IGNORECASE)' \
        >> "$ratchet_tmp/scripts/check-language-contract.py"
    git -C "$ratchet_tmp" add scripts/check-language-contract.py \
        siderita/qml/views/IndexLanguage.qml
    git -C "$ratchet_tmp" show HEAD:scripts/check-language-contract.py \
        > "$ratchet_tmp/scripts/check-language-contract.py"
    check_ratchets suite || \
        fail "a staged language scanner changed current-commit measurement"

    # The registry TOML is interpreted twice with the implementation from HEAD.
    # Neither a staged implementation nor a broken unstaged copy can run.
    reset_ratchet_fixture
    printf '%s\n' '' \
        '_fixture_build_commit_scopes = build_commit_scopes' \
        'def build_commit_scopes(registry):' \
        '    scopes = _fixture_build_commit_scopes(registry)' \
        '    scopes["index-only"] = scopes[registry["suite"]["commit_prefix"]]' \
        '    return scopes' \
        >> "$ratchet_tmp/scripts/project_registry.py"
    git -C "$ratchet_tmp" add scripts/project_registry.py
    printf '%s\n' 'raise RuntimeError("unstaged worktree registry must not execute")' \
        >> "$ratchet_tmp/scripts/project_registry.py"
    check_ratchets suite || fail "the hook executed staged or unstaged registry code"
    if check_ratchets index-only; then
        fail "a prefix invented by the INDEX authorized its own commit"
    fi
    if ! printf '%s\n' 'README.md' | \
        python3 "$ratchet_tmp/scripts/commit_scope.py" --root "$ratchet_tmp" \
            --check 'suite: inspect committed scope' >/dev/null 2>&1; then
        fail "--check executed the broken worktree registry"
    fi

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" 'line two' 'line three' \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i 's/^2\t/1\t/' "$ratchet_tmp/scripts/language-baseline.tsv"
    git -C "$ratchet_tmp" add .
    check_ratchets siderita || fail "an earned language reduction was rejected"

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" 'line two' 'line three' \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i '/FolderView.qml/d' "$ratchet_tmp/scripts/language-baseline.tsv"
    git -C "$ratchet_tmp" add .
    if check_ratchets siderita; then
        fail "a language row disappeared while staged debt remained"
    fi

    reset_ratchet_fixture
    printf '%s\n' 'line one' 'line two' 'line three' \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i '/FolderView.qml/d' "$ratchet_tmp/scripts/language-baseline.tsv"
    git -C "$ratchet_tmp" add .
    check_ratchets siderita || fail "fully translated staged content could not retire its row"

    # A scanner migration is the one way a language row falls without the file
    # that holds it: the rule changed, not the file. It takes both halves.
    reset_ratchet_fixture
    sed -i '/FolderView.qml/d' "$ratchet_tmp/scripts/language-baseline.tsv"
    mkdir -p "$ratchet_tmp/siderita/docs/evidence"
    printf '%s\n' '# Language guard migration' '' \
        '- **Resolved language debt:** `scripts/check-language-contract.py`' \
        > "$ratchet_tmp/siderita/docs/evidence/language.md"
    printf '%s\n' '# migrated scanner' >> "$ratchet_tmp/scripts/check-language-contract.py"
    git -C "$ratchet_tmp" add scripts/language-baseline.tsv \
        scripts/check-language-contract.py siderita/docs/evidence/language.md
    check_ratchets siderita || fail "a declared scanner migration could not retire its row"

    # The evidence alone proves nothing: without the scanner in the same commit
    # the measurement did not change and the row is simply being deleted.
    reset_ratchet_fixture
    sed -i '/FolderView.qml/d' "$ratchet_tmp/scripts/language-baseline.tsv"
    mkdir -p "$ratchet_tmp/siderita/docs/evidence"
    printf '%s\n' '# Language guard migration' '' \
        '- **Resolved language debt:** `scripts/check-language-contract.py`' \
        > "$ratchet_tmp/siderita/docs/evidence/language.md"
    git -C "$ratchet_tmp" add scripts/language-baseline.tsv \
        siderita/docs/evidence/language.md
    if check_ratchets siderita; then
        fail "a language row fell on evidence alone, with no scanner change"
    fi

    # And the scanner alone proves nothing either: a migration nobody wrote
    # down is indistinguishable from quietly dropping inconvenient debt.
    reset_ratchet_fixture
    sed -i '/FolderView.qml/d' "$ratchet_tmp/scripts/language-baseline.tsv"
    printf '%s\n' '# migrated scanner' >> "$ratchet_tmp/scripts/check-language-contract.py"
    git -C "$ratchet_tmp" add scripts/language-baseline.tsv \
        scripts/check-language-contract.py
    if check_ratchets siderita; then
        fail "a language row fell on a scanner change nobody declared"
    fi

    reset_ratchet_fixture
    git -C "$ratchet_tmp" rm -q siderita/qml/views/FolderView.qml
    sed -i '/^lines\tsiderita\/qml\/views\/FolderView.qml\t/d' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    sed -i '/FolderView.qml/d' "$ratchet_tmp/scripts/language-baseline.tsv"
    mkdir -p "$ratchet_tmp/siderita/docs/evidence"
    printf '%s\n' '# Architecture resolution' '' \
        '- **Resolved architecture debt:** `siderita/qml/views/FolderView.qml`' \
        > "$ratchet_tmp/siderita/docs/evidence/refactor.md"
    git -C "$ratchet_tmp" add scripts/architecture-baseline.tsv \
        scripts/language-baseline.tsv siderita/docs/evidence/refactor.md
    check_ratchets siderita || fail "a deleted source could not retire its ratchet rows"

    reset_ratchet_fixture
    printf '%s\n' '// Harmless merge fixture' \
        > "$ratchet_tmp/siderita/qml/views/MergeFixture.qml"
    git -C "$ratchet_tmp" add siderita/qml/views/MergeFixture.qml
    check_merge || fail "a merge with current ratchets and sources was rejected"

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    sed -i 's/FolderView.qml\t3/FolderView.qml\t2/' \
        "$ratchet_tmp/scripts/architecture-baseline.tsv"
    git -C "$ratchet_tmp" add scripts/architecture-baseline.tsv \
        siderita/qml/views/FolderView.qml
    if check_merge; then
        fail "a merge changed an architecture ratchet"
    fi

    reset_ratchet_fixture
    printf '%s\n' "$fixture_debt_one" "$fixture_debt_two" \
        > "$ratchet_tmp/siderita/qml/views/FolderView.qml"
    git -C "$ratchet_tmp" add siderita/qml/views/FolderView.qml
    if check_merge; then
        fail "a merge source diverged from its INDEX architecture baseline"
    fi

    reset_ratchet_fixture
    git -C "$ratchet_tmp" rm -q siderita/qml/views/FolderView.qml
    if check_merge; then
        fail "a merge deleted a source while retaining its INDEX ratchet row"
    fi
fi

# Format and vocabulary are also contractual; there is no silent bypass.
fixture_fix_word=arre
fixture_fix_word=${fixture_fix_word}gla
fixture_article_word=l
fixture_article_word=${fixture_article_word}a
fixture_view_word=vis
fixture_view_word=${fixture_view_word}ta
fixture_navigation_word=$(printf 'navegaci\303\263n')
fixture_non_english_subject="siderita: $fixture_fix_word $fixture_article_word $fixture_view_word"
expect_scope fail 'a subject without a prefix' 'siderita/src/main.rs'
expect_scope fail 'wip: change something' 'siderita/src/main.rs'
expect_scope fail 'Siderita: change something' 'siderita/src/main.rs'
expect_scope fail 'siderita: ' 'siderita/src/main.rs'
expect_scope fail "$fixture_non_english_subject" 'siderita/src/main.rs'
expect_scope fail "siderita: update $fixture_article_word $fixture_view_word" \
    'siderita/src/main.rs'
expect_scope fail "siderita: add $fixture_navigation_word support" 'siderita/src/main.rs'
expect_scope fail 'siderita: updating the view' 'siderita/src/main.rs'
expect_scope fail 'siderita: documentation cleanup' 'siderita/src/main.rs'
expect_scope pass 'siderita: update the view' 'siderita/src/main.rs'
expect_scope pass 'siderita: update X and Y' 'siderita/src/main.rs'
expect_scope pass 'siderita: audit the view contract' 'siderita/src/main.rs'
if [ "$typed_subjects" = true ]; then
    if printf '%s\n' 'siderita/src/main.rs' | sh "$hook" \
        --check 'siderita: update the view' >/dev/null 2>&1; then
        fail "a post-adoption subject omitted its required change kind"
    fi
    expect_scope pass 'siderita-bug: fix the view' 'siderita/src/main.rs'
fi
expect_scope pass 'Revert "siderita: update one surface"' 'siderita/src/main.rs'
expect_scope fail 'Revert "siderita: update one surface"' \
    'siderita/src/main.rs' 'celestina/src/main.cpp'
expect_scope pass 'fixup! siderita: update one surface' 'siderita/src/main.rs'
expect_scope fail 'fixup! siderita: update one surface' 'celestina/src/main.cpp'
expect_scope fail "fixup! $fixture_non_english_subject" 'siderita/src/main.rs'
if ! printf '%s\n' 'siderita/src/main.rs' | sh "$hook" \
    --history-scope-only 'siderita: documentation snapshot' >/dev/null 2>&1; then
    fail "history scope-only replay rejected an inherited non-imperative subject"
fi

# The registry, not a copied test table, knows newly registered seams.
expect_scope pass 'fluorita-qt: keep the render seam narrow' \
    'celestina-rs/crates/fluorita-qt/src/renderitem.cpp' \
    'celestina-rs/Cargo.toml' 'celestina-rs/Cargo.lock'
expect_scope fail 'fluorita-qt: cross a project boundary' \
    'celestina-rs/crates/fluorita-qt/src/renderitem.cpp' 'fluorita/qml/Main.qml'
expect_scope pass 'celestina-shell-core: extend the command vocabulary' \
    'celestina-rs/crates/celestina-shell-core/src/lib.rs' \
    'celestina-rs/Cargo.toml' 'celestina-rs/Cargo.lock'

if [ "$failures" -ne 0 ]; then
    printf '\n%d commit-scope test(s) failed.\n' "$failures" >&2
    exit 1
fi

printf 'Commit scope: OK\n'
