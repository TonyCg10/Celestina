#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
checker=$script_dir/check-staged-units.py
commit_checker=$script_dir/commit_scope.py
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM
case_root=$temporary/staged-unit

mkdir -p "$case_root/docs/plans/active" \
    "$case_root/app/docs/plans/active" \
    "$case_root/app/docs/inventories/2026-08-03-unit" \
    "$case_root/app/docs/evidence" \
    "$case_root/app/src"
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
} > "$case_root/docs/projects.toml"
plan_rel=app/docs/plans/active/2026-08-03-unit.md
inventory_rel=app/docs/inventories/2026-08-03-unit/UNIT-A.numstat.tsv
inventory_b_rel=app/docs/inventories/2026-08-03-unit/UNIT-B.numstat.tsv
evidence_rel=app/docs/evidence/2026-08-03-unit.md
evidence_b_rel=app/docs/evidence/2026-08-03-unit-b.md
source_rel=app/src/main.rs
source_b_rel=app/src/second.rs
plan=$case_root/$plan_rel
inventory=$case_root/$inventory_rel
inventory_b=$case_root/$inventory_b_rel
evidence=$case_root/$evidence_rel
evidence_b=$case_root/$evidence_b_rel
source=$case_root/$source_rel
source_b=$case_root/$source_b_rel
{
    printf '# Delivery plan\n\nThe unit is active.\n\n'
    printf '## Change and commit ledger\n\n'
    printf '| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |\n'
    printf '|---|---|---|---|---|---|---|---|\n'
} > "$plan"
printf 'pub fn base() {}\n' > "$source"
printf 'pub fn second_base() {}\n' > "$source_b"

git -C "$case_root" init -q
git -C "$case_root" config user.name "Staged Fixture"
git -C "$case_root" config user.email "fixture@example.invalid"
git -C "$case_root" add .
git -C "$case_root" commit -qm "fixture: establish staged base"
base=$(git -C "$case_root" rev-parse HEAD)

printf 'pub fn changed() {}\n' >> "$source"
printf 'pub fn second_changed() {}\n' >> "$source_b"
printf '| UNIT-A | `app:` | done | [Exact inventory](../../inventories/2026-08-03-unit/UNIT-A.numstat.tsv) | Change A | exact | evidence A | None |\n' >> "$plan"
printf '| UNIT-B | `app:` | done | [Second inventory](../../inventories/2026-08-03-unit/UNIT-B.numstat.tsv) | Change B | exact | evidence B | None |\n' >> "$plan"
printf '# Evidence\n\nThe staged fixture passed.\n' > "$evidence"
printf '# Evidence B\n\nThe second staged unit passed.\n' > "$evidence_b"
source_hash=$(sha256sum "$source")
source_hash=${source_hash%% *}
source_b_hash=$(sha256sum "$source_b")
source_b_hash=${source_b_hash%% *}
plan_hash=$(sha256sum "$plan")
plan_hash=${plan_hash%% *}
evidence_hash=$(sha256sum "$evidence")
evidence_hash=${evidence_hash%% *}
evidence_b_hash=$(sha256sum "$evidence_b")
evidence_b_hash=${evidence_b_hash%% *}
source_stat=$(git -C "$case_root" diff --numstat --no-renames "$base" -- "$source_rel")
set -- $source_stat
source_added=$1
source_deleted=$2
source_b_stat=$(git -C "$case_root" diff --numstat --no-renames "$base" -- "$source_b_rel")
set -- $source_b_stat
source_b_added=$1
source_b_deleted=$2
plan_stat=$(git -C "$case_root" diff --numstat --no-renames "$base" -- "$plan_rel")
set -- $plan_stat
plan_added=$1
plan_deleted=$2
evidence_stat=$(git diff --no-index --numstat /dev/null "$evidence" || true)
set -- $evidence_stat
evidence_added=$1
evidence_deleted=$2
evidence_b_stat=$(git diff --no-index --numstat /dev/null "$evidence_b" || true)
set -- $evidence_b_stat
evidence_b_added=$1
evidence_b_deleted=$2

write_inventory() {
    self_added=$1
    self_deleted=$2
    {
        printf '# Exact staged inventory\n\n'
        printf 'Base revision\t%s\n' "$base"
        printf 'Pathspec\t%s\n' "$source_rel"
        printf 'Pathspec\t%s\n' "$plan_rel"
        printf 'Pathspec\t%s\n' "$evidence_rel"
        printf 'Pathspec\t%s\n\n' "$inventory_rel"
        printf 'added\tdeleted\tcontent\tpath\n'
        printf '%s\t%s\t%s\t%s\n' "$source_added" "$source_deleted" "$source_hash" "$source_rel"
        printf '%s\t%s\t%s\t%s\n' "$plan_added" "$plan_deleted" "$plan_hash" "$plan_rel"
        printf '%s\t%s\t%s\t%s\n' "$evidence_added" "$evidence_deleted" "$evidence_hash" "$evidence_rel"
        printf '%s\t%s\tself\t%s\n' "$self_added" "$self_deleted" "$inventory_rel"
    } > "$inventory"
}

write_inventory_b() {
    self_added=$1
    self_deleted=$2
    {
        printf '# Second exact staged inventory\n\n'
        printf 'Base revision\t%s\n' "$base"
        printf 'Pathspec\t%s\n' "$source_b_rel"
        printf 'Pathspec\t%s\n' "$plan_rel"
        printf 'Pathspec\t%s\n' "$evidence_b_rel"
        printf 'Pathspec\t%s\n\n' "$inventory_b_rel"
        printf 'added\tdeleted\tcontent\tpath\n'
        printf '%s\t%s\t%s\t%s\n' "$source_b_added" "$source_b_deleted" "$source_b_hash" "$source_b_rel"
        printf '%s\t%s\t%s\t%s\n' "$plan_added" "$plan_deleted" "$plan_hash" "$plan_rel"
        printf '%s\t%s\t%s\t%s\n' "$evidence_b_added" "$evidence_b_deleted" "$evidence_b_hash" "$evidence_b_rel"
        printf '%s\t%s\tself\t%s\n' "$self_added" "$self_deleted" "$inventory_b_rel"
    } > "$inventory_b"
}

write_inventory 0 0
inventory_stat=$(git diff --no-index --numstat /dev/null "$inventory" || true)
set -- $inventory_stat
write_inventory "$1" "$2"
write_inventory_b 0 0
inventory_b_stat=$(git diff --no-index --numstat /dev/null "$inventory_b" || true)
set -- $inventory_b_stat
write_inventory_b "$1" "$2"
git -C "$case_root" add \
    "$source_rel" "$source_b_rel" "$plan_rel" \
    "$evidence_rel" "$evidence_b_rel" "$inventory_rel" "$inventory_b_rel"

python3 "$checker" --root "$case_root" --quiet
python3 "$checker" --root "$case_root" --quiet "$inventory_rel" "$inventory_b_rel"
python3 "$commit_checker" --root "$case_root" \
    --check-index 'app: record exact delivery'
if python3 "$commit_checker" --root "$case_root" \
    --check-index 'suite: absorb app delivery' \
    > "$temporary/wrong-subject-prefix.out" 2>&1; then
    printf 'FALLO: commit-msg aceptó suite: para una unidad app:\n' >&2
    exit 1
elif ! grep -F 'el lote requiere asunto `app:`, no `suite:`' \
    "$temporary/wrong-subject-prefix.out" >/dev/null; then
    printf 'FALLO: el asunto divergente no produjo diagnóstico estable\n' >&2
    exit 1
fi
printf 'Merge branch fixture\n' > "$temporary/merge-message.txt"
printf '%s\n' "$base" > "$case_root/.git/MERGE_HEAD"
if python3 "$commit_checker" --root "$case_root" \
    "$temporary/merge-message.txt" \
    > "$temporary/merge-delivery.out" 2>&1; then
    printf 'FALLO: commit-msg aceptó cerrar una unidad dentro de un merge\n' >&2
    exit 1
elif ! grep -F 'un merge no puede cerrar unidades de entrega' \
    "$temporary/merge-delivery.out" >/dev/null; then
    printf 'FALLO: el lote dentro del merge no produjo diagnóstico estable\n' >&2
    exit 1
fi
rm -f "$case_root/.git/MERGE_HEAD"

cp "$plan" "$temporary/plan.staged.md"
sed -i '/| UNIT-A |/d' "$plan"
printf '\n[Orphan inventory](../../inventories/2026-08-03-unit/UNIT-A.numstat.tsv)\n' \
    >> "$plan"
git -C "$case_root" add "$plan_rel"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/orphan-inventory.out" 2>&1; then
    printf 'FALLO: el checker aceptó un inventario enlazado fuera del ledger\n' >&2
    exit 1
elif ! grep -F 'requiere una única fila done host' \
    "$temporary/orphan-inventory.out" >/dev/null; then
    printf 'FALLO: el inventario huérfano no produjo diagnóstico estable\n' >&2
    exit 1
fi
cp "$temporary/plan.staged.md" "$plan"
git -C "$case_root" add "$plan_rel"

sed -i 's/| UNIT-A | `app:`/| UNIT-A | `app-core:`/' "$plan"
git -C "$case_root" add "$plan_rel"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/wrong-prefix.out" 2>&1; then
    printf 'FALLO: el checker aceptó el prefijo de otro owner/componente\n' >&2
    exit 1
elif ! grep -F 'fila done UNIT-A de app requiere prefijo app' \
    "$temporary/wrong-prefix.out" >/dev/null; then
    printf 'FALLO: el prefijo incorrecto no produjo diagnóstico estable\n' >&2
    exit 1
fi
cp "$temporary/plan.staged.md" "$plan"
git -C "$case_root" add "$plan_rel"

printf '\nDraft prose not included in this commit.\n' >> "$plan"
python3 "$checker" --root "$case_root" --quiet
cp "$temporary/plan.staged.md" "$plan"

sed -i \
    's|../../inventories/2026-08-03-unit/UNIT-A|../../inventories/2026-08-03-unit/../2026-08-03-unit/UNIT-A|' \
    "$plan"
python3 "$checker" --root "$case_root" --quiet
cp "$temporary/plan.staged.md" "$plan"

mv "$plan" "$temporary/plan-missing-from-worktree.md"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/missing-host.out" 2>&1; then
    printf 'FALLO: el checker aceptó un plan host ausente del worktree\n' >&2
    exit 1
elif ! grep -F 'plan host staged no existe en el worktree' \
    "$temporary/missing-host.out" >/dev/null; then
    printf 'FALLO: el host ausente no produjo diagnóstico estable\n' >&2
    exit 1
fi
mv "$temporary/plan-missing-from-worktree.md" "$plan"

cp "$inventory_b" "$temporary/inventory-b.valid.tsv"
sed -i "s|$source_b_hash|$source_hash|; s|$source_b_rel|$source_rel|g" "$inventory_b"
git -C "$case_root" add "$inventory_b_rel"
git -C "$case_root" reset -q HEAD -- "$source_b_rel"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/overlap.out" 2>&1; then
    printf 'FALLO: el checker aceptó inventarios staged solapados\n' >&2
    exit 1
elif ! grep -F 'inventarios staged se solapan' "$temporary/overlap.out" >/dev/null; then
    printf 'FALLO: el solapamiento staged no produjo diagnóstico estable\n' >&2
    exit 1
fi
cp "$temporary/inventory-b.valid.tsv" "$inventory_b"
git -C "$case_root" add "$inventory_b_rel" "$source_b_rel"

git -C "$case_root" reset -q HEAD -- "$source_rel"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/missing.out" 2>&1; then
    printf 'FALLO: el checker aceptó una ruta inventariada no staged\n' >&2
    exit 1
elif ! grep -F 'inventario contiene rutas no staged' "$temporary/missing.out" >/dev/null; then
    printf 'FALLO: la ruta omitida no produjo diagnóstico estable\n' >&2
    exit 1
fi
git -C "$case_root" add "$source_rel"

printf 'extra\n' > "$case_root/extra.txt"
git -C "$case_root" add extra.txt
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/extra.out" 2>&1; then
    printf 'FALLO: el checker aceptó una ruta staged extra\n' >&2
    exit 1
elif ! grep -F 'staging contiene rutas fuera del lote inventariado' \
    "$temporary/extra.out" >/dev/null; then
    printf 'FALLO: la ruta extra no produjo diagnóstico estable\n' >&2
    exit 1
fi
git -C "$case_root" reset -q HEAD -- extra.txt

git -C "$case_root" reset -q HEAD -- "$inventory_rel"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/reference.out" 2>&1; then
    printf 'FALLO: el checker aceptó un inventario enlazado pero no staged\n' >&2
    exit 1
elif ! grep -F 'referencia inventarios nuevos que no están staged' \
    "$temporary/reference.out" >/dev/null; then
    printf 'FALLO: el inventario no staged no produjo diagnóstico estable\n' >&2
    exit 1
fi
git -C "$case_root" add "$inventory_rel"

cp "$plan" "$temporary/plan-with-two-units.md"
sed -i '/Second inventory/d' "$plan"
git -C "$case_root" add "$plan_rel"
cp "$temporary/plan-with-two-units.md" "$plan"
sed -i \
    's|(../../inventories/2026-08-03-unit/UNIT-B.numstat.tsv)|(<../../inventories/2026-08-03-unit/UNIT-B.numstat.tsv> "record")|' \
    "$plan"
git -C "$case_root" reset -q HEAD -- \
    "$source_b_rel" "$evidence_b_rel" "$inventory_b_rel"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/partial-plan.out" 2>&1; then
    printf 'FALLO: el checker aceptó una unidad done fuera del staging parcial\n' >&2
    exit 1
elif ! grep -F 'plan host deja inventarios done fuera del índice' \
    "$temporary/partial-plan.out" >/dev/null; then
    printf 'FALLO: el staging parcial del plan no produjo diagnóstico estable\n' >&2
    exit 1
fi
git -C "$case_root" add \
    "$plan_rel" "$source_b_rel" "$evidence_b_rel" "$inventory_b_rel"

sed -i 's/pub fn changed()/pub fn staged()/' "$source"
git -C "$case_root" add "$source_rel"
if python3 "$checker" --root "$case_root" --quiet \
    > "$temporary/hash.out" 2>&1; then
    printf 'FALLO: el checker aceptó un hash staged obsoleto\n' >&2
    exit 1
elif ! grep -F 'SHA-256 staged no coincide' "$temporary/hash.out" >/dev/null; then
    printf 'FALLO: el hash staged obsoleto no produjo diagnóstico estable\n' >&2
    exit 1
fi

archive_root=$temporary/archive-unit
mkdir -p "$archive_root/docs/plans/active" \
    "$archive_root/app/docs/plans/active" \
    "$archive_root/app/docs/plans/archive" \
    "$archive_root/app/docs/inventories/2026-08-03-archive" \
    "$archive_root/app/docs/evidence"
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
} > "$archive_root/docs/projects.toml"
old_plan_rel=app/docs/plans/active/2026-08-03-archive.md
new_plan_rel=app/docs/plans/archive/2026-08-03-archive.md
old_inventory_rel=app/docs/inventories/2026-08-03-archive/ARCHIVE-OLD.numstat.tsv
new_inventory_rel=app/docs/inventories/2026-08-03-archive/ARCHIVE-MOVE.numstat.tsv
archive_evidence_rel=app/docs/evidence/2026-08-03-archive.md
old_plan=$archive_root/$old_plan_rel
old_inventory=$archive_root/$old_inventory_rel
new_plan=$archive_root/$new_plan_rel
new_inventory=$archive_root/$new_inventory_rel
archive_evidence=$archive_root/$archive_evidence_rel
{
    printf '# Archived plan\n\n'
    printf '## Change and commit ledger\n\n'
    printf '| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |\n'
    printf '|---|---|---|---|---|---|---|---|\n'
    printf '| ARCHIVE-OLD | `app:` | done | [Historical inventory](../../inventories/2026-08-03-archive/ARCHIVE-OLD.numstat.tsv) | Historical unit | exact | evidence | None |\n'
} > "$old_plan"
printf '# Previous inventory\n\nThis record is already tracked.\n' > "$old_inventory"
git -C "$archive_root" init -q
git -C "$archive_root" config user.name "Archive Fixture"
git -C "$archive_root" config user.email "fixture@example.invalid"
git -C "$archive_root" add .
git -C "$archive_root" commit -qm "fixture: establish archive base"
archive_base=$(git -C "$archive_root" rev-parse HEAD)

mv "$old_plan" "$new_plan"
git -C "$archive_root" add -A
if python3 "$checker" --root "$archive_root" --quiet \
    > "$temporary/archive-without-unit.out" 2>&1; then
    printf 'FALLO: el checker aceptó archivar un plan sin unidad administrativa\n' >&2
    exit 1
elif ! grep -F 'archivar un plan requiere una unidad administrativa' \
    "$temporary/archive-without-unit.out" >/dev/null; then
    printf 'FALLO: el archivado sin unidad no produjo diagnóstico estable\n' >&2
    exit 1
fi
printf '| ARCHIVE-MOVE | `app:` | done | [Archive inventory](../../inventories/2026-08-03-archive/ARCHIVE-MOVE.numstat.tsv) | Archive plan | exact | evidence | None |\n' >> "$new_plan"
printf '# Archive evidence\n\nThe move is exact.\n' > "$archive_evidence"
new_plan_hash=$(sha256sum "$new_plan")
new_plan_hash=${new_plan_hash%% *}
archive_evidence_hash=$(sha256sum "$archive_evidence")
archive_evidence_hash=${archive_evidence_hash%% *}
old_plan_stat=$(git -C "$archive_root" diff --numstat --no-renames \
    "$archive_base" -- "$old_plan_rel")
set -- $old_plan_stat
old_plan_added=$1
old_plan_deleted=$2
new_plan_stat=$(git diff --no-index --numstat /dev/null "$new_plan" || true)
set -- $new_plan_stat
new_plan_added=$1
new_plan_deleted=$2
archive_evidence_stat=$(git diff --no-index --numstat /dev/null \
    "$archive_evidence" || true)
set -- $archive_evidence_stat
archive_evidence_added=$1
archive_evidence_deleted=$2

write_archive_inventory() {
    archive_self_added=$1
    archive_self_deleted=$2
    {
        printf '# Archive transition inventory\n\n'
        printf 'Base revision\t%s\n' "$archive_base"
        printf 'Pathspec\tapp/docs/plans/\n'
        printf 'Pathspec\t%s\n' "$new_inventory_rel"
        printf 'Pathspec\t%s\n\n' "$archive_evidence_rel"
        printf 'added\tdeleted\tcontent\tpath\n'
        printf '%s\t%s\tdeleted\t%s\n' \
            "$old_plan_added" "$old_plan_deleted" "$old_plan_rel"
        printf '%s\t%s\t%s\t%s\n' \
            "$new_plan_added" "$new_plan_deleted" "$new_plan_hash" "$new_plan_rel"
        printf '%s\t%s\t%s\t%s\n' \
            "$archive_evidence_added" "$archive_evidence_deleted" \
            "$archive_evidence_hash" "$archive_evidence_rel"
        printf '%s\t%s\tself\t%s\n' \
            "$archive_self_added" "$archive_self_deleted" "$new_inventory_rel"
    } > "$new_inventory"
}

write_archive_inventory 0 0
archive_inventory_stat=$(git diff --no-index --numstat /dev/null \
    "$new_inventory" || true)
set -- $archive_inventory_stat
write_archive_inventory "$1" "$2"
git -C "$archive_root" add -A
python3 "$checker" --root "$archive_root" --quiet

printf '\nmutation\n' >> "$old_inventory"
git -C "$archive_root" add "$old_inventory_rel"
if python3 "$checker" --root "$archive_root" --quiet \
    > "$temporary/immutable-inventory.out" 2>&1; then
    printf 'FALLO: el checker aceptó modificar un inventario histórico\n' >&2
    exit 1
elif ! grep -F 'los inventarios históricos son inmutables' \
    "$temporary/immutable-inventory.out" >/dev/null; then
    printf 'FALLO: la mutación histórica no produjo diagnóstico estable\n' >&2
    exit 1
fi

mixed_root=$temporary/mixed-prefixes
mkdir -p "$mixed_root/docs/plans/active" \
    "$mixed_root/app/docs/plans/active" \
    "$mixed_root/app/docs/inventories/2026-08-03-app" \
    "$mixed_root/app/src" \
    "$mixed_root/core/docs/plans/active" \
    "$mixed_root/core/docs/inventories/2026-08-03-core" \
    "$mixed_root/core/src"
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
    printf 'include_workspace_manifests = false\n\n'
    printf '[[projects]]\n'
    printf 'id = "core"\n'
    printf 'commit_prefix = "core"\n'
    printf 'path = "core"\n'
    printf 'active_plans = "core/docs/plans/active"\n'
    printf 'commit_roots = ["core/"]\n'
    printf 'include_workspace_manifests = false\n'
} > "$mixed_root/docs/projects.toml"
printf 'base\n' > "$mixed_root/app/src/main.rs"
printf 'base\n' > "$mixed_root/core/src/lib.rs"
git -C "$mixed_root" init -q
git -C "$mixed_root" config user.name "Mixed Fixture"
git -C "$mixed_root" config user.email "fixture@example.invalid"
git -C "$mixed_root" add .
git -C "$mixed_root" commit -qm "fixture: establish mixed base"
mixed_base=$(git -C "$mixed_root" rev-parse HEAD)
mixed_app_plan_rel=app/docs/plans/active/2026-08-03-app.md
mixed_core_plan_rel=core/docs/plans/active/2026-08-03-core.md
mixed_app_inventory_rel=app/docs/inventories/2026-08-03-app/APP-A.numstat.tsv
mixed_core_inventory_rel=core/docs/inventories/2026-08-03-core/CORE-A.numstat.tsv
{
    printf '# App plan\n\n## Change and commit ledger\n\n'
    printf '| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |\n'
    printf '|---|---|---|---|---|---|---|---|\n'
    printf '| APP-A | `app:` | done | [Inventory](../../inventories/2026-08-03-app/APP-A.numstat.tsv) | App change | exact | evidence | None |\n'
} > "$mixed_root/$mixed_app_plan_rel"
{
    printf '# Core plan\n\n## Change and commit ledger\n\n'
    printf '| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |\n'
    printf '|---|---|---|---|---|---|---|---|\n'
    printf '| CORE-A | `core:` | done | [Inventory](../../inventories/2026-08-03-core/CORE-A.numstat.tsv) | Core change | exact | evidence | None |\n'
} > "$mixed_root/$mixed_core_plan_rel"
{
    printf '# App inventory\n\n'
    printf 'Base revision\t%s\n\n' "$mixed_base"
    printf 'added\tdeleted\tcontent\tpath\n'
    printf '0\t0\tself\t%s\n' "$mixed_app_inventory_rel"
} > "$mixed_root/$mixed_app_inventory_rel"
{
    printf '# Core inventory\n\n'
    printf 'Base revision\t%s\n\n' "$mixed_base"
    printf 'added\tdeleted\tcontent\tpath\n'
    printf '0\t0\tself\t%s\n' "$mixed_core_inventory_rel"
} > "$mixed_root/$mixed_core_inventory_rel"
git -C "$mixed_root" add -A
if python3 "$commit_checker" --root "$mixed_root" \
    --check-index 'suite: merge incompatible delivery units' \
    > "$temporary/mixed-prefixes.out" 2>&1; then
    printf 'FALLO: commit-msg aceptó owners incompatibles bajo suite:\n' >&2
    exit 1
elif ! grep -F 'requiere un único prefijo de commit: app, core' \
    "$temporary/mixed-prefixes.out" >/dev/null; then
    printf 'FALLO: el lote multiowner no produjo diagnóstico estable\n' >&2
    exit 1
fi

printf 'Inventarios staged: OK\n'
