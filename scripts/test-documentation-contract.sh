#!/bin/sh

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
fixture_root=$script_dir/fixtures/documentation
valid=$fixture_root/valid
invalid=$fixture_root/invalid
expected=$fixture_root/expected
checker=$script_dir/documentation_contract.py
context=$script_dir/agent-context.py

temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM

failures=0
fail() {
    printf 'FALLO: %s\n' "$1" >&2
    failures=$((failures + 1))
}

if ! python3 "$checker" --root "$valid" --quiet; then
    fail "la fixture positiva no satisface el contrato"
fi

missing_plan_id=$temporary/missing-plan-id
cp -R "$valid" "$missing_plan_id"
sed -i '/^- \*\*Plan ID:\*\*/d' \
    "$missing_plan_id/app/docs/plans/active/2026-08-03-app.md"
if python3 "$checker" --root "$missing_plan_id" --quiet \
    > "$temporary/missing-plan-id.out" 2>&1; then
    fail "el guard aceptó un plan activo sin Plan ID"
elif ! grep -F 'plan activo requiere metadata `Plan ID`' \
    "$temporary/missing-plan-id.out" >/dev/null; then
    fail "el Plan ID ausente no produjo diagnóstico estable"
fi

duplicate_plan_id=$temporary/duplicate-plan-id
cp -R "$valid" "$duplicate_plan_id"
cp "$duplicate_plan_id/app/docs/plans/active/2026-08-03-app.md" \
    "$duplicate_plan_id/app/docs/plans/active/2026-08-03-app-copy.md"
if python3 "$checker" --root "$duplicate_plan_id" --quiet \
    > "$temporary/duplicate-plan-id.out" 2>&1; then
    fail "el guard aceptó un Plan ID duplicado dentro del owner"
elif ! grep -F 'Plan ID `app` duplicado para owner `app`' \
    "$temporary/duplicate-plan-id.out" >/dev/null; then
    fail "el Plan ID duplicado no produjo diagnóstico estable"
fi

wrong_inventory_root=$temporary/wrong-inventory-root
cp -R "$valid" "$wrong_inventory_root"
wrong_inventory_plan=$wrong_inventory_root/app/docs/plans/active/2026-08-03-app.md
wrong_inventory_stable=$wrong_inventory_root/app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv
wrong_inventory_legacy_rel=app/docs/plans/active/APP-1B.numstat.tsv
wrong_inventory_legacy=$wrong_inventory_root/$wrong_inventory_legacy_rel
mv "$wrong_inventory_stable" "$wrong_inventory_legacy"
sed -i 's|(../../inventories/2026-08-03-app/APP-1B.numstat.tsv)|(APP-1B.numstat.tsv)|' \
    "$wrong_inventory_plan"
sed -i "s|app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv|$wrong_inventory_legacy_rel|g" \
    "$wrong_inventory_legacy"
if python3 "$checker" --root "$wrong_inventory_root" --quiet \
    > "$temporary/wrong-inventory-root.out" 2>&1; then
    fail "el guard aceptó un inventario fuera del root estable del plan"
elif ! grep -F 'debe vivir exactamente en `app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv`' \
    "$temporary/wrong-inventory-root.out" >/dev/null; then
    fail "el root de inventario incorrecto no produjo diagnóstico estable"
fi

nonboolean_suite=$temporary/nonboolean-suite
cp -R "$valid" "$nonboolean_suite"
sed -i 's/allow_all_commit_paths = true/allow_all_commit_paths = "false"/' \
    "$nonboolean_suite/docs/projects.toml"
if python3 "$checker" --root "$nonboolean_suite" --quiet \
    > "$temporary/nonboolean-suite.out" 2>&1; then
    fail "el registro aceptó allow_all_commit_paths no booleano"
elif ! grep -F 'suite.allow_all_commit_paths debe ser booleano' \
    "$temporary/nonboolean-suite.out" >/dev/null; then
    fail "allow_all_commit_paths no booleano no produjo diagnóstico estable"
fi

nonboolean_project=$temporary/nonboolean-project
cp -R "$valid" "$nonboolean_project"
sed -i '/commit_roots = \["app\/", "core\/crates\/app-core\/"\]/a include_workspace_manifests = "false"' \
    "$nonboolean_project/docs/projects.toml"
if python3 "$checker" --root "$nonboolean_project" --quiet \
    > "$temporary/nonboolean-project.out" 2>&1; then
    fail "el registro aceptó include_workspace_manifests no booleano"
elif ! grep -F 'projects[0].include_workspace_manifests debe ser booleano' \
    "$temporary/nonboolean-project.out" >/dev/null; then
    fail "include_workspace_manifests no booleano no produjo diagnóstico estable"
fi

git_case=$temporary/git-inventory
cp -R "$valid" "$git_case"
git -C "$git_case" init -q
git -C "$git_case" config user.name "Documentation Fixture"
git -C "$git_case" config user.email "fixture@example.invalid"
printf '\000\001\002\003' > "$git_case/core/crates/app-core/src/icon.bin"
printf 'pub fn mode_fixture() {}\n' > "$git_case/core/crates/app-core/src/mode.rs"
git -C "$git_case" add .
git -C "$git_case" commit -qm "fixture: establish base"
base_revision=$(git -C "$git_case" rev-parse HEAD)
git_source=$git_case/core/crates/app-core/src/lib.rs
git_plan=$git_case/app/docs/plans/active/2026-08-03-app.md
git_evidence=$git_case/app/docs/evidence/2026-08-03-fixture.md
git_inventory=$git_case/app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv
git_binary=$git_case/core/crates/app-core/src/icon.bin
git_mode=$git_case/core/crates/app-core/src/mode.rs

printf 'pub fn changed_fixture() {}\n' >> "$git_source"
printf '\003\002\001\000' > "$git_binary"
chmod +x "$git_mode"
printf 'this change belongs to another unit\n' > "$git_case/unrelated.txt"
sed -i 's/4 files, +66\/-1/6 files, +10\/-7/' "$git_plan"
sed -i 's/The fixture passes\./The current Git fixture passes./' "$git_evidence"
source_hash=$(sha256sum "$git_source")
source_hash=${source_hash%% *}
binary_hash=$(sha256sum "$git_binary")
binary_hash=${binary_hash%% *}
mode_hash=$(sha256sum "$git_mode")
mode_hash=${mode_hash%% *}
plan_hash=$(sha256sum "$git_plan")
plan_hash=${plan_hash%% *}
evidence_hash=$(sha256sum "$git_evidence")
evidence_hash=${evidence_hash%% *}
{
    printf '# APP-1B exact change inventory\n\n'
    printf 'Base revision\t%s\n' "$base_revision"
    printf 'Pathspec\tapp/docs/plans/active/2026-08-03-app.md\n'
    printf 'Pathspec\tapp/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv\n'
    printf 'Pathspec\tcore/crates/app-core/\n'
    printf 'Pathspec\tapp/docs/evidence/2026-08-03-fixture.md\n\n'
    printf 'added\tdeleted\tcontent\tpath\n'
    printf '1\t0\t%s\tcore/crates/app-core/src/lib.rs\n' "$source_hash"
    printf '%s\t%s\t%s\tcore/crates/app-core/src/icon.bin\n' - - "$binary_hash"
    printf '0\t0\t%s\tcore/crates/app-core/src/mode.rs\n' "$mode_hash"
    printf '1\t1\t%s\tapp/docs/plans/active/2026-08-03-app.md\n' "$plan_hash"
    printf '1\t1\t%s\tapp/docs/evidence/2026-08-03-fixture.md\n' "$evidence_hash"
    printf '7\t5\tself\tapp/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv\n'
} > "$git_inventory"

if ! python3 "$checker" --root "$git_case" --quiet; then
    fail "el inventario Git actual válido fue rechazado"
fi

cp "$git_inventory" "$temporary/git-inventory.valid.tsv"
sed -i "s/$source_hash/ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff/" \
    "$git_inventory"
if python3 "$checker" --root "$git_case" --quiet \
    > "$temporary/git-stale-hash.out" 2>&1; then
    fail "el guard aceptó un SHA-256 stale"
elif ! grep -F "SHA-256 stale" "$temporary/git-stale-hash.out" >/dev/null; then
    fail "el SHA-256 stale no produjo diagnóstico estable"
fi
cp "$temporary/git-inventory.valid.tsv" "$git_inventory"

sed -i '\|core/crates/app-core/src/lib.rs|d' "$git_inventory"
if python3 "$checker" --root "$git_case" --quiet \
    > "$temporary/git-missing-path.out" 2>&1; then
    fail "el guard aceptó un inventario que omitía una ruta cambiada"
elif ! grep -F 'omite rutas cambiadas según Git' \
    "$temporary/git-missing-path.out" >/dev/null; then
    fail "la ruta Git omitida no produjo diagnóstico estable"
fi
cp "$temporary/git-inventory.valid.tsv" "$git_inventory"

cp "$git_plan" "$temporary/git-plan.valid.md"
sed -i 's/6 files, +10\/-7/6 files, +11\/-7/' "$git_plan"
stale_plan_hash=$(sha256sum "$git_plan")
stale_plan_hash=${stale_plan_hash%% *}
sed -i "s/^1\t0\t$source_hash/2\t0\t$source_hash/" "$git_inventory"
sed -i "s/$plan_hash/$stale_plan_hash/" "$git_inventory"
if python3 "$checker" --root "$git_case" --quiet \
    > "$temporary/git-stale-numstat.out" 2>&1; then
    fail "el guard aceptó un numstat stale"
elif ! grep -F "numstat stale" "$temporary/git-stale-numstat.out" >/dev/null; then
    fail "el numstat stale no produjo diagnóstico estable"
fi
cp "$temporary/git-plan.valid.md" "$git_plan"
cp "$temporary/git-inventory.valid.tsv" "$git_inventory"

git -C "$git_case" add \
    app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv \
    app/docs/plans/active/2026-08-03-app.md \
    core/crates/app-core/src/icon.bin \
    core/crates/app-core/src/mode.rs \
    core/crates/app-core/src/lib.rs \
    app/docs/evidence/2026-08-03-fixture.md
git -C "$git_case" commit -qm "fixup! app: record verified inventory"
if ! python3 "$checker" --root "$git_case" --quiet; then
    fail "el inventario histórico limpio fue rechazado"
fi
printf 'pub fn later_change() {}\n' >> "$git_source"
git -C "$git_case" add "$git_source"
git -C "$git_case" commit -qm "app-core: change source later"
if ! python3 "$checker" --root "$git_case" --quiet; then
    fail "un cambio posterior invalidó el inventario histórico"
fi
printf '\n' >> "$git_inventory"
git -C "$git_case" add "$git_inventory"
git -C "$git_case" commit -qm "app: move inventory endpoint"
if python3 "$checker" --root "$git_case" --quiet \
    > "$temporary/git-multicommit-range.out" 2>&1; then
    fail "el guard aceptó un inventario histórico repartido entre commits"
elif ! grep -F 'debe ser el padre directo del commit del inventario' \
    "$temporary/git-multicommit-range.out" >/dev/null; then
    fail "el rango histórico multicommit no produjo diagnóstico estable"
fi

# Un inventario histórico vive fuera del directorio móvil de planes. El plan
# puede pasar de active/ a archive/ sin reescribir ni desplazar el TSV: el guard
# conserva C1 como endpoint y contrasta allí el host original del ledger.
archive_case=$temporary/git-archive-lifecycle
cp -R "$valid" "$archive_case"
archive_active_plan_rel=app/docs/plans/active/2026-08-03-app.md
archive_final_plan_rel=app/docs/plans/archive/2026-08-03-app.md
archive_inventory_rel=app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv
archive_old_inventory_rel=app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv
archive_evidence_rel=app/docs/evidence/2026-08-03-fixture.md
archive_source_rel=core/crates/app-core/src/lib.rs
archive_plan=$archive_case/$archive_active_plan_rel
archive_inventory=$archive_case/$archive_inventory_rel
archive_evidence=$archive_case/$archive_evidence_rel
archive_source=$archive_case/$archive_source_rel
mkdir -p "$archive_case/app/docs/inventories"
mv "$archive_case/$archive_old_inventory_rel" "$temporary/unused-archive-inventory.tsv"
sed -i '/| APP-1A |/d; /| APP-1B |/d' "$archive_plan"
git -C "$archive_case" init -q
git -C "$archive_case" config user.name "Documentation Fixture"
git -C "$archive_case" config user.email "fixture@example.invalid"
git -C "$archive_case" add .
git -C "$archive_case" commit -qm "fixture: establish archive base"
archive_base=$(git -C "$archive_case" rev-parse HEAD)

printf 'pub fn archived_fixture() {}\n' >> "$archive_source"
sed -i 's/The fixture passes\./The immutable archive fixture passes./' "$archive_evidence"
printf '%s\n' '| APP-1B | `app:` | done | [Exact inventory](../../inventories/2026-08-03-app/APP-1B.numstat.tsv) | Preserve one immutable endpoint | `ARCHIVE_DIFFSTAT` | [Fixture evidence](../../evidence/2026-08-03-fixture.md) | None |' \
    >> "$archive_plan"

archive_source_stat=$(git -C "$archive_case" diff --numstat --no-renames "$archive_base" -- "$archive_source_rel")
set -- $archive_source_stat
archive_source_added=$1
archive_source_deleted=$2
archive_plan_stat=$(git -C "$archive_case" diff --numstat --no-renames "$archive_base" -- "$archive_active_plan_rel")
set -- $archive_plan_stat
archive_plan_added=$1
archive_plan_deleted=$2
archive_evidence_stat=$(git -C "$archive_case" diff --numstat --no-renames "$archive_base" -- "$archive_evidence_rel")
set -- $archive_evidence_stat
archive_evidence_added=$1
archive_evidence_deleted=$2
archive_zero_hash=0000000000000000000000000000000000000000000000000000000000000000

write_archive_inventory() {
    self_added=$1
    self_deleted=$2
    source_hash=$3
    plan_hash=$4
    evidence_hash=$5
    {
        printf '# APP-1B immutable inventory\n\n'
        printf 'Base revision\t%s\n' "$archive_base"
        printf 'Pathspec\t%s\n' "$archive_source_rel"
        printf 'Pathspec\t%s\n' "$archive_active_plan_rel"
        printf 'Pathspec\t%s\n' "$archive_evidence_rel"
        printf 'Pathspec\t%s\n\n' "$archive_inventory_rel"
        printf 'added\tdeleted\tcontent\tpath\n'
        printf '%s\t%s\t%s\t%s\n' "$archive_source_added" "$archive_source_deleted" "$source_hash" "$archive_source_rel"
        printf '%s\t%s\t%s\t%s\n' "$archive_plan_added" "$archive_plan_deleted" "$plan_hash" "$archive_active_plan_rel"
        printf '%s\t%s\t%s\t%s\n' "$archive_evidence_added" "$archive_evidence_deleted" "$evidence_hash" "$archive_evidence_rel"
        printf '%s\t%s\tself\t%s\n' "$self_added" "$self_deleted" "$archive_inventory_rel"
    } > "$archive_inventory"
}

write_archive_inventory 0 0 "$archive_zero_hash" "$archive_zero_hash" "$archive_zero_hash"
archive_inventory_stat=$(git diff --no-index --numstat /dev/null "$archive_inventory" || true)
set -- $archive_inventory_stat
archive_inventory_added=$1
archive_inventory_deleted=$2
archive_total_added=$((archive_source_added + archive_plan_added + archive_evidence_added + archive_inventory_added))
archive_total_deleted=$((archive_source_deleted + archive_plan_deleted + archive_evidence_deleted + archive_inventory_deleted))
sed -i "s/ARCHIVE_DIFFSTAT/4 files, +$archive_total_added\/-$archive_total_deleted/" "$archive_plan"
archive_source_hash=$(sha256sum "$archive_source")
archive_source_hash=${archive_source_hash%% *}
archive_plan_hash=$(sha256sum "$archive_plan")
archive_plan_hash=${archive_plan_hash%% *}
archive_evidence_hash=$(sha256sum "$archive_evidence")
archive_evidence_hash=${archive_evidence_hash%% *}
write_archive_inventory "$archive_inventory_added" "$archive_inventory_deleted" \
    "$archive_source_hash" "$archive_plan_hash" "$archive_evidence_hash"

cp "$archive_inventory" "$temporary/archive-inventory.valid.tsv"
sed -i "\|$archive_active_plan_rel|d" "$archive_inventory"
if python3 "$checker" --root "$archive_case" --quiet \
    > "$temporary/archive-dirty-host.out" 2>&1; then
    fail "el guard aceptó un inventario dirty sin su plan host actual"
elif ! grep -F 'no contiene el plan que aloja el ledger' \
    "$temporary/archive-dirty-host.out" >/dev/null; then
    fail "el plan host ausente en inventario dirty no produjo diagnóstico estable"
fi
cp "$temporary/archive-inventory.valid.tsv" "$archive_inventory"

git -C "$archive_case" add \
    "$archive_source_rel" "$archive_active_plan_rel" \
    "$archive_evidence_rel" "$archive_inventory_rel"
git -C "$archive_case" commit -qm "app: preserve immutable inventory endpoint"
archive_endpoint=$(git -C "$archive_case" rev-parse HEAD)
if ! python3 "$checker" --root "$archive_case" --quiet; then
    fail "el inventario estable fue rechazado en su endpoint histórico"
fi

# El mismo movimiento sin unidad administrativa debe fallar una vez
# materializado en Git. La copia conserva C1 como padre directo de C2.
archive_missing_case=$temporary/git-archive-without-unit
mkdir -p "$archive_missing_case"
cp -R "$archive_case/." "$archive_missing_case"
mkdir -p "$archive_missing_case/app/docs/plans/archive"
git -C "$archive_missing_case" mv "$archive_active_plan_rel" "$archive_final_plan_rel"
archive_missing_plan=$archive_missing_case/$archive_final_plan_rel
sed -i 's/- \*\*Status:\*\* active/- **Status:** done/' "$archive_missing_plan"
sed -i '/- \*\*Opened:\*\* 2026-08-03/a - **Closed:** 2026-08-03\n- **Successor:** none' \
    "$archive_missing_plan"
sed -i 's/- \*\*Status:\*\* active/- **Status:** done/; s/- \*\*Active implementation checkpoint:\*\* APP-1/- **Active implementation checkpoint:** none/' \
    "$archive_missing_case/app/ROADMAP.md"
sed -i 's|docs/plans/active/2026-08-03-app.md|docs/plans/archive/2026-08-03-app.md|' \
    "$archive_missing_case/app/VALIDATION.md"
git -C "$archive_missing_case" add -A -- \
    app/ROADMAP.md app/VALIDATION.md app/docs/plans
git -C "$archive_missing_case" commit -qm "app: archive plan without inventory"
if python3 "$checker" --root "$archive_missing_case" --quiet \
    > "$temporary/archive-without-unit.out" 2>&1; then
    fail "el guard aceptó un active->archive histórico sin unidad administrativa"
elif ! grep -F 'requiere una unidad done con inventario del mismo endpoint' \
    "$temporary/archive-without-unit.out" >/dev/null; then
    fail "el active->archive sin unidad no produjo diagnóstico estable"
fi

# C2 archiva el plan mediante una unidad administrativa propia. Su inventario
# permanece en el root estable y reclama tanto D active como A archive.
mkdir -p "$archive_case/app/docs/plans/archive"
git -C "$archive_case" mv "$archive_active_plan_rel" "$archive_final_plan_rel"
archive_plan=$archive_case/$archive_final_plan_rel
archive_transition_inventory_rel=app/docs/inventories/2026-08-03-app/APP-ARCHIVE.numstat.tsv
archive_transition_evidence_rel=app/docs/evidence/2026-08-03-archive-plan.md
archive_roadmap_rel=app/ROADMAP.md
archive_validation_rel=app/VALIDATION.md
archive_transition_inventory=$archive_case/$archive_transition_inventory_rel
archive_transition_evidence=$archive_case/$archive_transition_evidence_rel
sed -i 's/- \*\*Status:\*\* active/- **Status:** done/' "$archive_plan"
sed -i '/- \*\*Opened:\*\* 2026-08-03/a - **Closed:** 2026-08-03\n- **Successor:** none' "$archive_plan"
sed -i 's/- \*\*Status:\*\* active/- **Status:** done/; s/- \*\*Active implementation checkpoint:\*\* APP-1/- **Active implementation checkpoint:** none/' \
    "$archive_case/$archive_roadmap_rel"
sed -i 's|docs/plans/active/2026-08-03-app.md|docs/plans/archive/2026-08-03-app.md|' \
    "$archive_case/$archive_validation_rel"
printf '%s\n' '| APP-ARCHIVE | `app:` | done | [Exact inventory](../../inventories/2026-08-03-app/APP-ARCHIVE.numstat.tsv) | Archive the completed plan | `ARCHIVE_MOVE_DIFFSTAT` | [Archive evidence](../../evidence/2026-08-03-archive-plan.md) | None |' \
    >> "$archive_plan"
{
    printf '# Archived plan transition evidence\n\n'
    printf '%s\n' '- **Date:** 2026-08-03'
    printf '%s\n' '- **Scope:** APP-ARCHIVE'
    printf '%s\n' '- **Environment:** hermetic Git fixture'
    printf '%s\n\n' '- **Artifact:** archived plan transition'
    printf '## Procedure\n\n'
    printf 'Archive the active plan in the same commit as its exact inventory.\n\n'
    printf '## Result\n\n'
    printf 'C2 records the active deletion and archive addition.\n\n'
    printf '## Limits\n\n'
    printf 'This record proves only the documentation fixture transition.\n'
} > "$archive_transition_evidence"

archive_transition_active_stat=$(git -C "$archive_case" diff --numstat --no-renames \
    "$archive_endpoint" -- "$archive_active_plan_rel")
set -- $archive_transition_active_stat
archive_transition_active_added=$1
archive_transition_active_deleted=$2
archive_transition_plan_stat=$(git -C "$archive_case" diff --numstat --no-renames \
    "$archive_endpoint" -- "$archive_final_plan_rel")
set -- $archive_transition_plan_stat
archive_transition_plan_added=$1
archive_transition_plan_deleted=$2
archive_transition_roadmap_stat=$(git -C "$archive_case" diff --numstat --no-renames \
    "$archive_endpoint" -- "$archive_roadmap_rel")
set -- $archive_transition_roadmap_stat
archive_transition_roadmap_added=$1
archive_transition_roadmap_deleted=$2
archive_transition_validation_stat=$(git -C "$archive_case" diff --numstat --no-renames \
    "$archive_endpoint" -- "$archive_validation_rel")
set -- $archive_transition_validation_stat
archive_transition_validation_added=$1
archive_transition_validation_deleted=$2
archive_transition_evidence_stat=$(git diff --no-index --numstat /dev/null \
    "$archive_transition_evidence" || true)
set -- $archive_transition_evidence_stat
archive_transition_evidence_added=$1
archive_transition_evidence_deleted=$2

write_archive_transition_inventory() {
    self_added=$1
    self_deleted=$2
    plan_hash=$3
    roadmap_hash=$4
    validation_hash=$5
    evidence_hash=$6
    {
        printf '# APP-ARCHIVE exact change inventory\n\n'
        printf 'Base revision\t%s\n' "$archive_endpoint"
        printf 'Pathspec\t%s\n' "$archive_active_plan_rel"
        printf 'Pathspec\t%s\n' "$archive_final_plan_rel"
        printf 'Pathspec\t%s\n' "$archive_roadmap_rel"
        printf 'Pathspec\t%s\n' "$archive_validation_rel"
        printf 'Pathspec\t%s\n' "$archive_transition_evidence_rel"
        printf 'Pathspec\t%s\n\n' "$archive_transition_inventory_rel"
        printf 'added\tdeleted\tcontent\tpath\n'
        printf '%s\t%s\tdeleted\t%s\n' \
            "$archive_transition_active_added" "$archive_transition_active_deleted" \
            "$archive_active_plan_rel"
        printf '%s\t%s\t%s\t%s\n' \
            "$archive_transition_plan_added" "$archive_transition_plan_deleted" \
            "$plan_hash" "$archive_final_plan_rel"
        printf '%s\t%s\t%s\t%s\n' \
            "$archive_transition_roadmap_added" "$archive_transition_roadmap_deleted" \
            "$roadmap_hash" "$archive_roadmap_rel"
        printf '%s\t%s\t%s\t%s\n' \
            "$archive_transition_validation_added" "$archive_transition_validation_deleted" \
            "$validation_hash" "$archive_validation_rel"
        printf '%s\t%s\t%s\t%s\n' \
            "$archive_transition_evidence_added" "$archive_transition_evidence_deleted" \
            "$evidence_hash" "$archive_transition_evidence_rel"
        printf '%s\t%s\tself\t%s\n' \
            "$self_added" "$self_deleted" "$archive_transition_inventory_rel"
    } > "$archive_transition_inventory"
}

write_archive_transition_inventory 0 0 \
    "$archive_zero_hash" "$archive_zero_hash" "$archive_zero_hash" "$archive_zero_hash"
archive_transition_inventory_stat=$(git diff --no-index --numstat /dev/null \
    "$archive_transition_inventory" || true)
set -- $archive_transition_inventory_stat
archive_transition_inventory_added=$1
archive_transition_inventory_deleted=$2
archive_transition_total_added=$((
    archive_transition_active_added + archive_transition_plan_added +
    archive_transition_roadmap_added + archive_transition_validation_added +
    archive_transition_evidence_added + archive_transition_inventory_added
))
archive_transition_total_deleted=$((
    archive_transition_active_deleted + archive_transition_plan_deleted +
    archive_transition_roadmap_deleted + archive_transition_validation_deleted +
    archive_transition_evidence_deleted + archive_transition_inventory_deleted
))
sed -i "s/ARCHIVE_MOVE_DIFFSTAT/6 files, +$archive_transition_total_added\/-$archive_transition_total_deleted/" \
    "$archive_plan"
archive_transition_plan_hash=$(sha256sum "$archive_plan")
archive_transition_plan_hash=${archive_transition_plan_hash%% *}
archive_transition_roadmap_hash=$(sha256sum "$archive_case/$archive_roadmap_rel")
archive_transition_roadmap_hash=${archive_transition_roadmap_hash%% *}
archive_transition_validation_hash=$(sha256sum "$archive_case/$archive_validation_rel")
archive_transition_validation_hash=${archive_transition_validation_hash%% *}
archive_transition_evidence_hash=$(sha256sum "$archive_transition_evidence")
archive_transition_evidence_hash=${archive_transition_evidence_hash%% *}
write_archive_transition_inventory \
    "$archive_transition_inventory_added" "$archive_transition_inventory_deleted" \
    "$archive_transition_plan_hash" "$archive_transition_roadmap_hash" \
    "$archive_transition_validation_hash" "$archive_transition_evidence_hash"
if ! python3 "$checker" --root "$archive_case" --quiet; then
    fail "la unidad administrativa dirty del archivo fue rechazada"
fi
git -C "$archive_case" add -A -- \
    "$archive_roadmap_rel" "$archive_validation_rel" \
    app/docs/plans "$archive_transition_evidence_rel" \
    "$archive_transition_inventory_rel"
git -C "$archive_case" commit -qm "app: archive completed plan"
archive_transition_endpoint=$(git -C "$archive_case" rev-parse HEAD)
if ! python3 "$checker" --root "$archive_case" --quiet; then
    fail "el plan archivado con unidad administrativa fue rechazado"
fi
archive_observed_endpoint=$(git -C "$archive_case" log -1 --format=%H -- "$archive_inventory_rel")
if [ "$archive_observed_endpoint" != "$archive_endpoint" ]; then
    fail "el ciclo de archivo reescribió el endpoint del inventario estable"
fi
archive_transition_observed_endpoint=$(git -C "$archive_case" log -1 --format=%H -- \
    "$archive_transition_inventory_rel")
if [ "$archive_transition_observed_endpoint" != "$archive_transition_endpoint" ]; then
    fail "la unidad administrativa no comparte endpoint con C2"
fi

# Un plan creado directamente bajo archive/ sigue necesitando su ledger done,
# pero no una fila artificial D active: el sibling nunca existió en el padre.
direct_plan_rel=app/docs/plans/archive/2026-08-03-direct.md
direct_inventory_rel=app/docs/inventories/2026-08-03-direct/APP-DIRECT.numstat.tsv
direct_evidence_rel=app/docs/evidence/2026-08-03-direct-archive.md
direct_plan=$archive_case/$direct_plan_rel
direct_inventory=$archive_case/$direct_inventory_rel
direct_evidence=$archive_case/$direct_evidence_rel
mkdir -p "$archive_case/app/docs/inventories/2026-08-03-direct"
{
    printf '# Direct-born archived plan\n\n'
    printf '%s\n' '- **Opened:** 2026-08-03'
    printf '%s\n' '- **Plan ID:** direct'
    printf '%s\n' '- **Status:** done'
    printf '%s\n' '- **Closed:** 2026-08-03'
    printf '%s\n' '- **Scope:** app'
    printf '%s\n' '- **Implementation checkpoint:** APP-DIRECT'
    printf '%s\n' '- **Author-validation checkpoint:** none'
    printf '%s\n\n' '- **Successor:** none'
    printf '## Hypothesis\n\nA direct archived record has no active predecessor.\n\n'
    printf '## Tangible outcome\n\nThe historical guard accepts its direct addition.\n\n'
    printf '## Scope\n\n- Direct archive fixture.\n\n'
    printf '## Exclusions\n\n- Active plan movement.\n\n'
    printf '## Build order\n\n1. Add the completed historical record.\n\n'
    printf '## Implementation exit\n\nThe direct archive fixture passes.\n\n'
    printf '## Change and commit ledger\n\n'
    printf '%s\n' '| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |'
    printf '%s\n' '|---|---|---|---|---|---|---|---|'
    printf '%s\n' '| APP-DIRECT | `app:` | done | [Exact inventory](../../inventories/2026-08-03-direct/APP-DIRECT.numstat.tsv) | Record a plan born in archive | `DIRECT_ARCHIVE_DIFFSTAT` | [Direct evidence](../../evidence/2026-08-03-direct-archive.md) | None |'
} > "$direct_plan"
{
    printf '# Direct archive evidence\n\n'
    printf '%s\n' '- **Date:** 2026-08-03'
    printf '%s\n' '- **Scope:** APP-DIRECT'
    printf '%s\n' '- **Environment:** hermetic Git fixture'
    printf '%s\n\n' '- **Artifact:** direct archived plan'
    printf '## Procedure\n\nAdd the completed plan directly under archive.\n\n'
    printf '## Result\n\nThe parent contains no active sibling with the same basename.\n\n'
    printf '## Limits\n\nThis record proves only the direct-add exemption.\n'
} > "$direct_evidence"

direct_plan_stat=$(git diff --no-index --numstat /dev/null "$direct_plan" || true)
set -- $direct_plan_stat
direct_plan_added=$1
direct_plan_deleted=$2
direct_evidence_stat=$(git diff --no-index --numstat /dev/null "$direct_evidence" || true)
set -- $direct_evidence_stat
direct_evidence_added=$1
direct_evidence_deleted=$2

write_direct_archive_inventory() {
    self_added=$1
    self_deleted=$2
    plan_hash=$3
    evidence_hash=$4
    {
        printf '# APP-DIRECT exact change inventory\n\n'
        printf 'Base revision\t%s\n' "$archive_transition_endpoint"
        printf 'Pathspec\t%s\n' "$direct_plan_rel"
        printf 'Pathspec\t%s\n' "$direct_evidence_rel"
        printf 'Pathspec\t%s\n\n' "$direct_inventory_rel"
        printf 'added\tdeleted\tcontent\tpath\n'
        printf '%s\t%s\t%s\t%s\n' \
            "$direct_plan_added" "$direct_plan_deleted" "$plan_hash" "$direct_plan_rel"
        printf '%s\t%s\t%s\t%s\n' \
            "$direct_evidence_added" "$direct_evidence_deleted" \
            "$evidence_hash" "$direct_evidence_rel"
        printf '%s\t%s\tself\t%s\n' \
            "$self_added" "$self_deleted" "$direct_inventory_rel"
    } > "$direct_inventory"
}

write_direct_archive_inventory 0 0 "$archive_zero_hash" "$archive_zero_hash"
direct_inventory_stat=$(git diff --no-index --numstat /dev/null "$direct_inventory" || true)
set -- $direct_inventory_stat
direct_inventory_added=$1
direct_inventory_deleted=$2
direct_total_added=$((direct_plan_added + direct_evidence_added + direct_inventory_added))
direct_total_deleted=$((direct_plan_deleted + direct_evidence_deleted + direct_inventory_deleted))
sed -i "s/DIRECT_ARCHIVE_DIFFSTAT/3 files, +$direct_total_added\/-$direct_total_deleted/" \
    "$direct_plan"
direct_plan_hash=$(sha256sum "$direct_plan")
direct_plan_hash=${direct_plan_hash%% *}
direct_evidence_hash=$(sha256sum "$direct_evidence")
direct_evidence_hash=${direct_evidence_hash%% *}
write_direct_archive_inventory \
    "$direct_inventory_added" "$direct_inventory_deleted" \
    "$direct_plan_hash" "$direct_evidence_hash"
if ! python3 "$checker" --root "$archive_case" --quiet; then
    fail "el alta dirty directa bajo archive fue rechazada"
fi
git -C "$archive_case" add \
    "$direct_plan_rel" "$direct_inventory_rel" "$direct_evidence_rel"
git -C "$archive_case" commit -qm "app: record direct archived plan"
if ! python3 "$checker" --root "$archive_case" --quiet; then
    fail "el alta histórica directa bajo archive exigió un movimiento inexistente"
fi

cp "$archive_plan" "$temporary/archive-plan.valid.md"
sed -i 's/- \*\*Plan ID:\*\* app/- **Plan ID:** APP-OTHER/' "$archive_plan"
if python3 "$checker" --root "$archive_case" --quiet \
    > "$temporary/archive-plan-id.out" 2>&1; then
    fail "el guard aceptó un Plan ID distinto después de archivar"
elif ! grep -F 'endpoint histórico de APP-1B requiere un único plan host con Plan ID `APP-OTHER`' \
    "$temporary/archive-plan-id.out" >/dev/null; then
    fail "el Plan ID divergente no produjo diagnóstico estable"
fi
cp "$temporary/archive-plan.valid.md" "$archive_plan"

sed -i 's/| APP-1B |/| APP-RENAMED |/' "$archive_plan"
if python3 "$checker" --root "$archive_case" --quiet \
    > "$temporary/archive-unit-id.out" 2>&1; then
    fail "el guard aceptó una unidad distinta después de archivar"
elif ! grep -F 'endpoint histórico de APP-RENAMED requiere un único plan host' \
    "$temporary/archive-unit-id.out" >/dev/null; then
    fail "la unidad histórica divergente no produjo diagnóstico estable"
fi
cp "$temporary/archive-plan.valid.md" "$archive_plan"

# Dos inventarios del mismo commit pueden compartir el plan que aloja el
# ledger, pero no atribuirse la misma ruta de implementación. Esta prueba se
# ejecuta después del commit para cubrir exactamente la lectura de CI.
overlap_case=$temporary/git-historical-overlap
cp -R "$valid" "$overlap_case"
overlap_plan=$overlap_case/app/docs/plans/active/2026-08-03-app.md
overlap_inventory_b=$overlap_case/app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv
mv "$overlap_inventory_b" "$temporary/unused-base-inventory.tsv"
sed -i '/APP-1B/d' "$overlap_plan"
git -C "$overlap_case" init -q
git -C "$overlap_case" config user.name "Documentation Fixture"
git -C "$overlap_case" config user.email "fixture@example.invalid"
git -C "$overlap_case" add .
git -C "$overlap_case" commit -qm "fixture: establish overlap base"
overlap_base=$(git -C "$overlap_case" rev-parse HEAD)
overlap_source_rel=core/crates/app-core/src/lib.rs
overlap_plan_rel=app/docs/plans/active/2026-08-03-app.md
overlap_inventory_b_rel=app/docs/inventories/2026-08-03-app/APP-1B.numstat.tsv
overlap_inventory_c_rel=app/docs/inventories/2026-08-03-app/APP-1C.numstat.tsv
overlap_evidence_b_rel=app/docs/evidence/2026-08-03-overlap-b.md
overlap_evidence_c_rel=app/docs/evidence/2026-08-03-overlap-c.md
overlap_source=$overlap_case/$overlap_source_rel
overlap_inventory_b=$overlap_case/$overlap_inventory_b_rel
overlap_inventory_c=$overlap_case/$overlap_inventory_c_rel
overlap_evidence_b=$overlap_case/$overlap_evidence_b_rel
overlap_evidence_c=$overlap_case/$overlap_evidence_c_rel

printf 'pub fn shared_overlap() {}\n' >> "$overlap_source"
{
    printf '# APP-1B overlap evidence\n\n'
    printf '%s\n' '- **Date:** 2026-08-03'
    printf '%s\n' '- **Scope:** APP-1B'
    printf '%s\n' '- **Environment:** hermetic fixture'
    printf '%s\n\n' '- **Artifact:** historical overlap guard'
    printf '## Procedure\n\nRun the overlap fixture.\n\n'
    printf '## Result\n\nThe first unit records its intended paths.\n\n'
    printf '## Limits\n\nThis is synthetic evidence.\n'
} > "$overlap_evidence_b"
{
    printf '# APP-1C overlap evidence\n\n'
    printf '%s\n' '- **Date:** 2026-08-03'
    printf '%s\n' '- **Scope:** APP-1C'
    printf '%s\n' '- **Environment:** hermetic fixture'
    printf '%s\n\n' '- **Artifact:** historical overlap guard'
    printf '## Procedure\n\nRun the overlap fixture.\n\n'
    printf '## Result\n\nThe second unit records its intended paths.\n\n'
    printf '## Limits\n\nThis is synthetic evidence.\n'
} > "$overlap_evidence_c"
{
    printf '%s\n' '| APP-1B | `app:` | done | [Exact inventory](../../inventories/2026-08-03-app/APP-1B.numstat.tsv) | Record the first ownership claim | `OVERLAP_B` | [Overlap B](../../evidence/2026-08-03-overlap-b.md) | None |'
    printf '%s\n' '| APP-1C | `app:` | done | [Exact inventory](../../inventories/2026-08-03-app/APP-1C.numstat.tsv) | Record the conflicting ownership claim | `OVERLAP_C` | [Overlap C](../../evidence/2026-08-03-overlap-c.md) | None |'
} >> "$overlap_plan"

overlap_source_stat=$(git -C "$overlap_case" diff --numstat --no-renames "$overlap_base" -- "$overlap_source_rel")
set -- $overlap_source_stat
overlap_source_added=$1
overlap_source_deleted=$2
overlap_plan_stat=$(git -C "$overlap_case" diff --numstat --no-renames "$overlap_base" -- "$overlap_plan_rel")
set -- $overlap_plan_stat
overlap_plan_added=$1
overlap_plan_deleted=$2
overlap_evidence_b_stat=$(git diff --no-index --numstat /dev/null "$overlap_evidence_b" || true)
set -- $overlap_evidence_b_stat
overlap_evidence_b_added=$1
overlap_evidence_b_deleted=$2
overlap_evidence_c_stat=$(git diff --no-index --numstat /dev/null "$overlap_evidence_c" || true)
set -- $overlap_evidence_c_stat
overlap_evidence_c_added=$1
overlap_evidence_c_deleted=$2
zero_hash=0000000000000000000000000000000000000000000000000000000000000000

write_overlap_inventory() {
    inventory_path=$1
    inventory_rel=$2
    evidence_rel=$3
    evidence_hash=$4
    evidence_added=$5
    evidence_deleted=$6
    self_added=$7
    self_deleted=$8
    {
        printf '# Historical overlap inventory\n\n'
        printf 'Base revision\t%s\n' "$overlap_base"
        printf 'Pathspec\t%s\n' "$overlap_source_rel"
        printf 'Pathspec\t%s\n' "$overlap_plan_rel"
        printf 'Pathspec\t%s\n' "$evidence_rel"
        printf 'Pathspec\t%s\n\n' "$inventory_rel"
        printf 'added\tdeleted\tcontent\tpath\n'
        printf '%s\t%s\t%s\t%s\n' "$overlap_source_added" "$overlap_source_deleted" "$overlap_source_hash" "$overlap_source_rel"
        printf '%s\t%s\t%s\t%s\n' "$overlap_plan_added" "$overlap_plan_deleted" "$overlap_plan_hash" "$overlap_plan_rel"
        printf '%s\t%s\t%s\t%s\n' "$evidence_added" "$evidence_deleted" "$evidence_hash" "$evidence_rel"
        printf '%s\t%s\tself\t%s\n' "$self_added" "$self_deleted" "$inventory_rel"
    } > "$inventory_path"
}

overlap_source_hash=$zero_hash
overlap_plan_hash=$zero_hash
write_overlap_inventory "$overlap_inventory_b" "$overlap_inventory_b_rel" \
    "$overlap_evidence_b_rel" "$zero_hash" \
    "$overlap_evidence_b_added" "$overlap_evidence_b_deleted" 0 0
write_overlap_inventory "$overlap_inventory_c" "$overlap_inventory_c_rel" \
    "$overlap_evidence_c_rel" "$zero_hash" \
    "$overlap_evidence_c_added" "$overlap_evidence_c_deleted" 0 0
overlap_inventory_b_stat=$(git diff --no-index --numstat /dev/null "$overlap_inventory_b" || true)
set -- $overlap_inventory_b_stat
overlap_inventory_b_added=$1
overlap_inventory_b_deleted=$2
overlap_inventory_c_stat=$(git diff --no-index --numstat /dev/null "$overlap_inventory_c" || true)
set -- $overlap_inventory_c_stat
overlap_inventory_c_added=$1
overlap_inventory_c_deleted=$2
overlap_b_added=$((overlap_source_added + overlap_plan_added + overlap_evidence_b_added + overlap_inventory_b_added))
overlap_b_deleted=$((overlap_source_deleted + overlap_plan_deleted + overlap_evidence_b_deleted + overlap_inventory_b_deleted))
overlap_c_added=$((overlap_source_added + overlap_plan_added + overlap_evidence_c_added + overlap_inventory_c_added))
overlap_c_deleted=$((overlap_source_deleted + overlap_plan_deleted + overlap_evidence_c_deleted + overlap_inventory_c_deleted))
sed -i "s|OVERLAP_B|4 files, +$overlap_b_added/-$overlap_b_deleted|" "$overlap_plan"
sed -i "s|OVERLAP_C|4 files, +$overlap_c_added/-$overlap_c_deleted|" "$overlap_plan"

overlap_source_hash=$(sha256sum "$overlap_source")
overlap_source_hash=${overlap_source_hash%% *}
overlap_plan_hash=$(sha256sum "$overlap_plan")
overlap_plan_hash=${overlap_plan_hash%% *}
overlap_evidence_b_hash=$(sha256sum "$overlap_evidence_b")
overlap_evidence_b_hash=${overlap_evidence_b_hash%% *}
overlap_evidence_c_hash=$(sha256sum "$overlap_evidence_c")
overlap_evidence_c_hash=${overlap_evidence_c_hash%% *}
write_overlap_inventory "$overlap_inventory_b" "$overlap_inventory_b_rel" \
    "$overlap_evidence_b_rel" "$overlap_evidence_b_hash" \
    "$overlap_evidence_b_added" "$overlap_evidence_b_deleted" \
    "$overlap_inventory_b_added" "$overlap_inventory_b_deleted"
write_overlap_inventory "$overlap_inventory_c" "$overlap_inventory_c_rel" \
    "$overlap_evidence_c_rel" "$overlap_evidence_c_hash" \
    "$overlap_evidence_c_added" "$overlap_evidence_c_deleted" \
    "$overlap_inventory_c_added" "$overlap_inventory_c_deleted"

git -C "$overlap_case" add .
git -C "$overlap_case" commit -qm "app: record conflicting historical inventories"
if python3 "$checker" --root "$overlap_case" --quiet \
    > "$temporary/git-historical-overlap.out" 2>&1; then
    fail "el guard aceptó inventarios históricos que se solapan"
elif ! grep -F 'reclaman las mismas rutas' \
    "$temporary/git-historical-overlap.out" >/dev/null; then
    fail "el solapamiento histórico no produjo diagnóstico estable"
fi

worktree_case=$temporary/ignored-worktree
mkdir -p "$worktree_case/.claude/worktrees/ignored"
cp -R "$valid/." "$worktree_case/"
cp "$fixture_root/ignored-worktree/CLAUDE.fixture" \
    "$worktree_case/.claude/worktrees/ignored/CLAUDE.md"
if ! python3 "$checker" --root "$worktree_case" --quiet; then
    fail "el guard recorrió .claude/worktrees"
fi

if ! python3 "$context" --root "$valid" app/src/main.rs \
    > "$temporary/app-context.txt"; then
    fail "agent-context no resolvió una ruta del proyecto"
elif ! diff -u "$expected/app-context.txt" "$temporary/app-context.txt"; then
    fail "agent-context devolvió un orden incorrecto para app"
fi

if ! python3 "$context" --root "$valid" core/crates/app-core/src/lib.rs \
    > "$temporary/core-context.txt"; then
    fail "agent-context no resolvió un source root compartido"
elif ! diff -u "$expected/core-context.txt" "$temporary/core-context.txt"; then
    fail "agent-context omitió el AGENTS físico o el proyecto owner"
fi

if python3 "$context" --root "$valid" ../outside \
    > "$temporary/outside.out" 2>&1; then
    fail "agent-context aceptó una ruta fuera del repositorio"
elif ! grep -F "path sale del repositorio" "$temporary/outside.out" >/dev/null; then
    fail "agent-context falló fuera del repo sin diagnóstico estable"
fi

expect_failure() {
    case_name=$1
    expected_text=$2
    work=$temporary/$case_name
    mkdir -p "$work"
    cp -R "$valid/." "$work/"
    cp -R "$invalid/$case_name/." "$work/"
    if python3 "$checker" --root "$work" --quiet \
        > "$temporary/$case_name.out" 2>&1; then
        fail "la fixture negativa $case_name pasó"
        return
    fi
    if ! grep -F "$expected_text" "$temporary/$case_name.out" >/dev/null; then
        fail "la fixture $case_name no emitió: $expected_text"
    fi
}

expect_failure vendor-specific "archivo normativo específico de proveedor prohibido"
for vendor_path in CLAUDE.md GEMINI.md .cursorrules \
    .github/copilot-instructions.md .github/instructions/project.instructions.md; do
    if ! grep -F "$vendor_path" "$temporary/vendor-specific.out" >/dev/null; then
        fail "la fixture vendor-specific no detectó $vendor_path"
    fi
done
expect_failure broken-link "enlace local roto"
expect_failure broken-anchor "anchor local no existe"
expect_failure bad-plan 'plans/active requiere `Status: active`'
expect_failure bad-ledger "ledger carece de columnas: diffstat"
expect_failure active-no-plan "requiere exactamente un plan activo"
expect_failure inactive-checkpoint 'requiere `Active implementation checkpoint: none`'
expect_failure checkpoint-mismatch "no coincide con ROADMAP"
expect_failure duplicate-active-plan "tiene varios planes activos"
expect_failure orphan-plan "plan activo huérfano"
expect_failure wrong-prefix 'prefijo fuera del owner `app`'
expect_failure suite-prefix-local 'prefijo fuera del owner `app`'
expect_failure component-prefix-ledger 'prefijo fuera del owner `app`'
expect_failure foreign-owner-evidence 'Automated evidence fuera del owner `app`'
expect_failure suite-project-evidence 'Automated evidence fuera del owner `suite`'
expect_failure overlapping-pathspec 'reclaman las mismas rutas'
expect_failure bad-done-diffstat 'unidad done requiere diffstat `N files, +X/-Y`'
expect_failure done-without-inventory 'unidad done requiere un único enlace a inventario `.numstat.tsv`'
expect_failure done-broken-inventory 'inventario `.numstat.tsv` no es resoluble'
expect_failure done-without-evidence 'requiere expediente resoluble bajo `evidence/`'
expect_failure done-broken-evidence 'requiere expediente resoluble bajo `evidence/`'
expect_failure noncanonical-done-evidence 'requiere expediente resoluble bajo `evidence/`'
expect_failure bad-numstat-syntax 'content debe ser SHA-256 o marcador de cierre permitido'
expect_failure finalized-numstat-marker 'content debe ser SHA-256 o marcador de cierre permitido'
expect_failure wrong-numstat-self 'marcador `self` sólo pertenece al propio inventario'
expect_failure bad-numstat-base 'requiere una única línea `Base revision<TAB><40 hex>`'
if ! grep -F 'requiere al menos un `Pathspec`' \
    "$temporary/bad-numstat-base.out" >/dev/null; then
    fail "el inventario sin frontera Pathspec no produjo diagnóstico estable"
fi
expect_failure duplicate-numstat-base 'requiere una única línea `Base revision<TAB><40 hex>`'
expect_failure duplicate-numstat-path 'ruta numstat duplicada'
expect_failure numstat-sum-mismatch 'Diffstat declara +66/-1'
expect_failure numstat-file-count 'Diffstat declara 4 archivos'
expect_failure numstat-missing-plan 'no contiene el plan que aloja el ledger'
expect_failure numstat-missing-evidence 'no contiene ningún expediente enlazado'
expect_failure bad-decision "estado de decisión inválido"
expect_failure bad-local-decision "estado de decisión inválido"
expect_failure nested-bad-decision "estado de decisión inválido"
expect_failure orphan-decision-index "registro huérfano no enlazado"
expect_failure duplicate-decision-index "registro debe enlazarse exactamente una vez"
expect_failure stale-decision-status 'Status `proposed` no coincide con metadata `accepted`'
expect_failure stale-decision-entry "entrada stale requiere un registro canónico"
expect_failure orphan-discussion-index "registro huérfano no enlazado"
expect_failure stale-discussion-status 'Status `concluded` no coincide con metadata `open`'
expect_failure empty-decision-section 'sección obligatoria `Decision` está vacía'
expect_failure empty-discussion-section 'sección obligatoria `Strongest Case` está vacía'
expect_failure concluded-pending-discussion "conserva Conclusion Pending"
expect_failure applied-without-canonical-link "requiere enlace al hogar canónico actualizado"
expect_failure bad-local-discussion 'discusión requiere heading `Strongest Case`'
expect_failure nested-bad-discussion 'discusión requiere heading `Strongest Case`'
expect_failure bad-decision-name 'decisión requiere nombre `NNNN-short-topic.md`'
expect_failure bad-discussion-name 'discusión requiere nombre `YYYY-MM-DD-short-topic.md`'
expect_failure bad-evidence-name 'evidencia requiere nombre `YYYY-MM-DD-short-topic.md`'
expect_failure bad-active-plan-name 'plan activo requiere nombre `YYYY-MM-DD-short-topic.md`'
expect_failure bad-archive-plan-name 'plan archivado requiere nombre `YYYY-MM-DD-short-topic.md`'
expect_failure bad-archived-plan 'plan archivado requiere `Closed: YYYY-MM-DD`'
expect_failure bad-validation "requiere Status en"
expect_failure missing-validation-fields 'requiere metadata `Related Implementation`'
expect_failure duplicate-validation "VAL-APP-DUP duplicado"
expect_failure passed-without-evidence "VAL-APP-PASSED cerrado requiere evidencia"
expect_failure passed-evidence-readme 'passed requiere expediente `.md` resoluble bajo `evidence/`'
expect_failure passed-evidence-unregistered 'passed requiere expediente `.md` resoluble bajo `evidence/`'
expect_failure failed-evidence-noncanonical 'failed requiere expediente `.md` resoluble bajo `evidence/`'
expect_failure empty-validation-cell 'validación línea 5: `requires` vacío'
expect_failure invalid-validation-id 'ID `VAL-*` inválido'
expect_failure unmarked-validation-heading 'caso manual debe comenzar con ID `VAL-*` válido'
expect_failure malformed-validation-heading 'caso manual debe comenzar con ID `VAL-*` válido'
expect_failure failed-without-remediation-link 'failed requiere remediación enlazada a un plan active/archive resoluble'
expect_failure failed-broken-remediation 'failed requiere remediación enlazada a un plan active/archive resoluble'
expect_failure failed-remediation-nonplan 'failed requiere remediación enlazada a un plan active/archive resoluble'
expect_failure failed-remediation-unregistered 'failed requiere remediación enlazada a un plan active/archive resoluble'
expect_failure failed-remediation-missing-unit 'ledger no contiene la unidad `APP-MISSING`'
expect_failure obsolete-without-result 'cerrado requiere resultado observado'
expect_failure obsolete-without-decision 'obsolete requiere enlace resoluble a decisión o evidencia'
expect_failure obsolete-target-unregistered 'obsolete requiere enlace resoluble a decisión o evidencia'
expect_failure obsolete-target-index 'obsolete requiere enlace resoluble a decisión o evidencia'
expect_failure empty-evidence-section 'sección obligatoria `Procedure` está vacía'
expect_failure missing-script "script registrado no existe"

if [ "$failures" -ne 0 ]; then
    printf '\nContrato documental: %s fixture(s) fallaron.\n' "$failures" >&2
    exit 1
fi

printf 'Contrato documental y agent-context: OK\n'
