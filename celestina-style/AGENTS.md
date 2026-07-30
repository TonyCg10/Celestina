# celestina-style — delta local

El `AGENTS.md` de la raíz sigue aplicando. Este directorio es el lenguaje visual
genérico de la suite, no una extensión de ninguna app.

## Frontera, tokens y API pública

- El módulo contiene solo QML, fuentes, iconos y recursos genéricos. No importa
  módulos de Siderita/Magnetita, no conoce controladores de app ni codifica rutas
  o vocabulario de una pantalla concreta.
- `CelestinaTheme.qml` es el único origen de primitivas y derivaciones visuales.
  Un rol nuevo es semántico, incluye su pareja superficie/tinta cuando proceda y
  lo define cada esquema. Los consumidores no derivan colores, opacidades,
  anatomía o movimiento localmente; la geometría propia de una pantalla sí queda
  en la app.
- La API de un componente es cerrada y estrecha: roles/enums, propiedades
  requeridas y señales orientadas a intención. No se expone un controlador de
  app para ahorrar diseño de API.
- Un tipo público aparece de forma coherente en `qmldir` y `CMakeLists.txt`, con
  los singletons marcados igual. Todo recurso referenciado existe y figura tanto
  en el QRC correspondiente como en `CMakeLists.txt`.
- Un componente compartido aterriza con consumidor real conforme al contrato
  raíz. Ese consumidor añade el symlink canónico y su registro en `build.rs`;
  nunca se publica una copia. Antes de cambiar semántica o defaults, busca y
  revisa todos los consumidores.

## Estados e interacción

- Todo control interactivo compartido cubre los estados pertinentes: enabled,
  hover, pressed, selected/checked y `visualFocus`; ofrece teclado equivalente y
  rol, nombre y acción `Accessible`.
- Una capa modal implementa el contrato completo de foco y bloqueo, no solo un
  scrim que capture clics. Los consumidores no deben poder disparar atajos de la
  superficie cubierta.
- `CelestinaTheme.reducedMotion` es la única entrada de movimiento reducido.
  Todo `Behavior`, `Transition` o animación nuevo/modificado la consume; no
  inventes flags locales por app. El token y su inyección desde los hosts ya
  existen, pero eso no sustituye la auditoría de animaciones heredadas ni la
  comprobación interactiva con el modo encendido y apagado.

## Matriz mínima de verificación

| Cambio | Evidencia obligatoria |
| --- | --- |
| Token o implementación interna | `scripts/check-style-contract.sh`, `qmllint` del módulo y galería sin errores. |
| Tipo, singleton o recurso público | Paridad `qmldir`/CMake/QRC, symlink y registro del primer consumidor, build del módulo y del consumidor. |
| Color, superficie o tipografía | Galería/captura de todos los roles afectados y contraste sobre su superficie real. |
| Control, foco, modal o movimiento | Ratón y teclado, `visualFocus`, árbol/acciones accesibles, apertura/cierre modal y `reducedMotion` encendido/apagado. |
| Cambio semántico compartido | Buscar todos los consumidores y ejecutar el build/smoke relevante de cada uno; documentar cualquier incompatibilidad deliberada. |
