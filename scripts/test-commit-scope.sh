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
    revisados=0
    git -C "$root" log --format='%H' "$desde..HEAD" | while read -r commit; do
        asunto=$(git -C "$root" log -1 --format='%s' "$commit")
        case $asunto in
            fixup!*|squash!*|amend!*|Revert\ *) continue ;;
        esac
        # Una fusión no declara alcance propio.
        if [ "$(git -C "$root" rev-list --parents -n1 "$commit" | wc -w)" -gt 2 ]; then
            continue
        fi
        ficheros=$(git -C "$root" show --name-only --format='' "$commit" | grep -v '^$' || true)
        [ -n "$ficheros" ] || continue
        if ! printf '%s\n' "$ficheros" | sh "$hook" --check "$asunto" >/dev/null 2>&1; then
            printf 'FALLO: el historial no pasa su propio guard: %s %s\n' \
                "$(echo "$commit" | cut -c1-7)" "$asunto" >&2
            printf '%s\n' "$ficheros" | sh "$hook" --check "$asunto" >&2 || true
            exit 1
        fi
        revisados=$((revisados + 1))
    done || fail "un commit del historial no pasa el guard"
else
    printf 'aviso: %s no existe; se omite el contraste con el historial.\n' "$desde" >&2
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
# El shell y su core comparten prefijo.
comprobar ok 'celestina: algo' \
    'celestina/src/main.cpp' 'celestina-rs/crates/celestina-shell-core/src/lib.rs'

# Lo que el guard deliberadamente no juzga.
comprobar ok 'un asunto sin prefijo' 'siderita/src/main.rs' 'celestina/src/main.cpp'
comprobar ok 'wip: lo que sea' 'siderita/src/main.rs' 'celestina/src/main.cpp'
comprobar ok 'Revert "siderita: algo"' 'siderita/src/main.rs' 'celestina/src/main.cpp'

if [ "$fallos" -ne 0 ]; then
    printf '\n%d prueba(s) del guard de alcance fallaron.\n' "$fallos" >&2
    exit 1
fi

printf 'Alcance de commits: OK\n'
