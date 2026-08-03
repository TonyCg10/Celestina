#!/bin/sh
set -eu

# Pruebas del guard de alcance de commits (`.githooks/commit-msg`).
#
# Dos mitades, y la primera importa más: el guard se contrasta contra el
# historial real, no sólo contra ejemplos inventados. Un guard que sólo pasa sus
# propias fixtures es un guard que nadie ha probado contra cómo se trabaja aquí;
# si una regla es demasiado estricta, el historial es quien lo dice.

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)
hook=$root/.githooks/commit-msg

fallos=0
fail() {
    printf 'FALLO: %s\n' "$1" >&2
    fallos=$((fallos + 1))
}

[ -x "$hook" ] || fail "$hook no es ejecutable"

comprobar() {
    # comprobar <esperado: ok|ko> <asunto> <ficheros...>
    esperado=$1
    asunto=$2
    shift 2
    if printf '%s\n' "$@" | sh "$hook" --check "$asunto" >/dev/null 2>&1; then
        obtenido=ok
    else
        obtenido=ko
    fi
    if [ "$obtenido" != "$esperado" ]; then
        fail "esperaba $esperado y salió $obtenido -> \"$asunto\" con: $*"
    fi
}

# ── 1. El historial real ──────────────────────────────────────────────────
# Todo commit desde que la convención se aplica debe pasar. El rango arranca en
# el primer commit escrito ya bajo esta regla; los anteriores son de antes de
# que existiera y no se les puede pedir cuentas.
# Overridable para poder comprobar que este bucle sabe fallar: apuntándolo a
# historia anterior a la convención, debe delatar los commits que no la cumplen.
desde=${COMMIT_SCOPE_DESDE:-9ecc457}
if git -C "$root" rev-parse -q --verify "$desde^{commit}" >/dev/null 2>&1; then
    if ! git -C "$root" rev-parse -q --verify "$desde^" >/dev/null 2>&1; then
        fail "el ancla histórica $desde no tiene padre verificable"
    else
        git -C "$root" log --format='%H' "$desde^..HEAD" | while read -r commit; do
        asunto=$(git -C "$root" log -1 --format='%s' "$commit")
        # Una fusión no declara alcance propio.
        if [ "$(git -C "$root" rev-list --parents -n1 "$commit" | wc -w)" -gt 2 ]; then
            continue
        fi
        ficheros=$(git -C "$root" show --no-renames --name-only --format='' "$commit" | grep -v '^$' || true)
        [ -n "$ficheros" ] || continue
        if ! printf '%s\n' "$ficheros" | sh "$hook" --check "$asunto" >/dev/null 2>&1; then
            printf 'FALLO: el historial no pasa su propio guard: %s %s\n' \
                "$(echo "$commit" | cut -c1-7)" "$asunto" >&2
            printf '%s\n' "$ficheros" | sh "$hook" --check "$asunto" >&2 || true
            exit 1
        fi
        done || fail "un commit del historial no pasa el guard"
    fi
else
    fail "el ancla histórica $desde no existe; actualízala explícitamente"
fi

# ── 2. Fixtures ───────────────────────────────────────────────────────────

# Lo que el guard existe para cazar: un prefijo que miente.
comprobar ko 'siderita: algo' 'siderita/src/main.rs' 'celestina/src/main.cpp'
comprobar ko 'grafita: algo' 'grafita/src/main.rs' 'fluorita/src/main.rs'
comprobar ko 'celestina-style: algo' 'celestina-style/qmldir' 'siderita/qml/Main.qml'
# Un core no se cuela por el prefijo de otro.
comprobar ko 'grafita-core: algo' 'celestina-rs/crates/fluorita-core/src/lib.rs'
# El caso original: el árbol entero bajo un prefijo cualquiera.
comprobar ko 'siderita: cambios pendientes' \
    'siderita/src/main.rs' 'celestina/src/main.cpp' 'grafita/src/main.rs' \
    'celestina-style/qmldir' 'README.md'

# Lo legítimo, que no debe estorbar.
comprobar ok 'siderita: algo' 'siderita/src/main.rs' 'siderita/qml/Main.qml'
comprobar ok 'suite: algo transversal' 'siderita/src/main.rs' 'celestina/src/main.cpp'
# Una app con core propio los lleva juntos.
comprobar ok 'magnetita: algo' \
    'magnetita/ROADMAP.md' 'celestina-rs/crates/magnetita-core/src/clipboard.rs'
comprobar ok 'grafita: algo' 'grafita/src/main.rs' 'celestina-rs/crates/grafita-core/src/save.rs'
# Dar de alta un crate edita el manifiesto del workspace en el mismo commit.
comprobar ok 'fluorita-core: algo' \
    'celestina-rs/crates/fluorita-core/src/lib.rs' 'celestina-rs/Cargo.toml' 'celestina-rs/Cargo.lock'
comprobar ko 'fluorita-core: no confundas un backup' \
    'celestina-rs/crates/fluorita-core/src/lib.rs' 'celestina-rs/Cargo.toml.backup'
# El prefijo principal cierra la unidad con sus registros locales.
comprobar ok 'siderita: close one ledger unit' \
    'celestina-rs/crates/siderita-core/src/lib.rs' \
    'siderita/ROADMAP.md' 'siderita/STATUS.md' \
    'siderita/docs/plans/active/2026-08-03-unit.md' \
    'siderita/docs/evidence/2026-08-03-unit.md'
# Un prefijo de componente no posee los registros persistentes del owner.
comprobar ko 'siderita-core: close one ledger unit' \
    'celestina-rs/crates/siderita-core/src/lib.rs' \
    'siderita/docs/plans/active/2026-08-03-unit.md'
comprobar ko 'siderita: cross into another owner records' \
    'siderita/src/main.rs' 'magnetita/docs/plans/active/2026-08-03-unit.md'
# Los roots son fronteras, no prefijos léxicos aproximados.
comprobar ko 'grafita: reject a lookalike crate' \
    'celestina-rs/crates/grafita-evil/src/lib.rs'
# --no-renames entrega al guard origen y destino de un movimiento.
comprobar ko 'siderita: move a foreign file into scope' \
    'celestina/src/foreign.cpp' 'siderita/src/foreign.cpp'
# El shell y su core comparten prefijo.
comprobar ok 'celestina: algo' \
    'celestina/src/main.cpp' 'celestina-rs/crates/celestina-shell-core/src/lib.rs'

# El formato y el vocabulario también son contrato: no hay bypass silencioso.
comprobar ko 'un asunto sin prefijo' 'siderita/src/main.rs'
comprobar ko 'wip: lo que sea' 'siderita/src/main.rs'
comprobar ko 'Siderita: cambia algo' 'siderita/src/main.rs'
comprobar ko 'siderita: ' 'siderita/src/main.rs'
comprobar ok 'Revert "siderita: update one surface"' 'siderita/src/main.rs'
comprobar ko 'Revert "siderita: update one surface"' \
    'siderita/src/main.rs' 'celestina/src/main.cpp'
comprobar ok 'fixup! siderita: update one surface' 'siderita/src/main.rs'
comprobar ko 'fixup! siderita: update one surface' 'celestina/src/main.cpp'

# El registro, no una tabla copiada en el test, conoce nuevos seams.
comprobar ok 'fluorita-qt: keep the render seam narrow' \
    'celestina-rs/crates/fluorita-qt/src/renderitem.cpp' \
    'celestina-rs/Cargo.toml' 'celestina-rs/Cargo.lock'
comprobar ko 'fluorita-qt: cross a project boundary' \
    'celestina-rs/crates/fluorita-qt/src/renderitem.cpp' 'fluorita/qml/Main.qml'
comprobar ok 'celestina-shell-core: extend the command vocabulary' \
    'celestina-rs/crates/celestina-shell-core/src/lib.rs' \
    'celestina-rs/Cargo.toml' 'celestina-rs/Cargo.lock'

if [ "$fallos" -ne 0 ]; then
    printf '\n%d prueba(s) del guard de alcance fallaron.\n' "$fallos" >&2
    exit 1
fi

printf 'Alcance de commits: OK\n'
