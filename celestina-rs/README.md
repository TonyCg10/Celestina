# Celestina Rust cores

Este workspace contiene lógica de dominio reutilizable y neutral a la interfaz
para la suite Celestina. La presentación continúa en cada aplicación.

## Crates

| Crate | Responsabilidad |
|---|---|
| `celestina-core` | Generaciones y cancelación cooperativa compartidas |
| `siderita-core` | Identidad, snapshots, publicación vigente y executor acotado |
| `siderita-qt` | Tokens de vista estables y adaptación del core para Qt/QML |
| `celestina-dotfiles-core` | Planificación de cambios de dotfiles sin mutar el sistema |

Las aplicaciones siguen siendo proyectos y releases independientes. Durante
el desarrollo pueden usar dependencias `path`; una instalación o release debe
consumir versiones fijadas de estos crates.

## Comprobaciones

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

El primer corte no tiene dependencias de terceros. `siderita-core` es de solo
lectura y `celestina-dotfiles-core` solo produce planes: todavía no existe una
API para aplicar cambios. El QObject CXX-Qt se incorporará como feature del
adaptador cuando Qt esté disponible en el entorno declarado.
