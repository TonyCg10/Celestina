# Evidence: the module's lint is ordered behind the module

- **Date:** 2026-08-05
- **Scope:** `STYLE-G7-B` of [the shared reading controls plan](../plans/active/2026-08-04-shared-reading-controls.md)
- **Environment:** Arch Linux, CMake 4.4.2, Qt 6.11.1, existing incremental build in `celestina-style/build`
- **Artifact:** none of its own; this changes how the existing build tree is ordered

## What failed

`celestina/scripts/verify-production.sh` runs this project's verification, which
builds `all_qmllint`. It failed twice in a row with:

```text
Could not open:
celestina-style/build/meta_types/celestina-style_json_file_list.txt.timestamp
```

The file was present and readable the moment the build stopped, which rules out
a missing file and points at two writers instead of one.

## Procedure

```sh
cmake --build build --target all_qmllint
grep -l json_file_list.txt.timestamp build/CMakeFiles/*.dir/build.make
cmake -S . -B build && grep 'celestina-style_qmllint.dir/all:' build/CMakeFiles/Makefile2
celestina/scripts/complete-production.sh
celestina/scripts/verify-production.sh
celestina/scripts/verify-production.sh
celestina/scripts/status-production.sh
```

The generated graph was read before and after each attempted ordering change;
the canonical verifier, rather than the isolated target alone, decided whether
the writer race was closed.

## Diagnosis

`qt_add_qml_module` puts the **same** `cmake_automoc_parser` invocation into two
generated targets. Both were found in the existing build tree:

```sh
grep -l json_file_list.txt.timestamp celestina-style/build/CMakeFiles/*.dir/build.make
# celestina-style/build/CMakeFiles/celestina-style_qmltyperegistration.dir/build.make
# celestina-style/build/CMakeFiles/celestina-style.dir/build.make
```

That command writes `meta_types/<target>_json_file_list.txt.timestamp` through
`--timestamp-file-path`. The timestamp is **not declared as an output of any
rule**, so the build system has nothing to order the two copies by.

The first attempted fix ordered the lint target behind the library. Its
generated graph read:

```text
CMakeFiles/celestina-style_qmllint.dir/all: CMakeFiles/celestina-style.dir/all
CMakeFiles/celestina-style_qmllint.dir/all: CMakeFiles/all_qmltyperegistrations.dir/all
```

That did not order the two writers. Make may build both prerequisites of the
lint target in parallel, so the library's parser and the parser reached through
`all_qmltyperegistrations` still overlapped. The next canonical verification
failed with the same timestamp diagnostic and invalidated its old seal.

## The change

One `add_dependencies(celestina-style_qmltyperegistration celestina-style)`,
guarded by `if(TARGET …)`. The dependency is attached to the target containing
the second parser invocation, not to their later lint consumer. The generated
graph now reads:

```text
CMakeFiles/celestina-style_qmltyperegistration.dir/all: CMakeFiles/celestina-style.dir/all
CMakeFiles/all_qmltyperegistrations.dir/all: CMakeFiles/celestina-style_qmltyperegistration.dir/all
CMakeFiles/celestina-style_qmllint.dir/all: CMakeFiles/all_qmltyperegistrations.dir/all
```

The module's parser therefore finishes before type registration can start its
copy, and lint reaches them in that order.

## Result

| Check | Result |
|---|---|
| Writer-to-writer ordering edge | present after regeneration in `build/CMakeFiles/Makefile2` |
| `cmake --build build --target all_qmllint` from the existing incremental tree | passed |
| First `celestina/scripts/verify-production.sh` after the lint-only ordering attempt | failed with the original timestamp diagnostic and cleared the verification seal |
| `celestina/scripts/complete-production.sh` after the writer-to-writer fix | built once, verified, deployed and reported all seven Celestina artifacts current; the session was not activated |
| Two immediate additional `celestina/scripts/verify-production.sh` runs | both passed on the same incremental tree and resealed the same artifact |
| Final `celestina/scripts/status-production.sh` | current and verified |

## Limits

- Six earlier forced-stale isolated builds passed even though the canonical
  verifier still failed. That disproved the isolated target as sufficient
  evidence; closure therefore uses the registered verifier and its two
  immediate repetitions.
- No cache or build tree was deleted, and no `clean` was run: everything above
  was measured against the tree that produced the two failures.
- This changes build ordering only. No QML, no token and no artifact content
  changes, so the module's own tests and lint output are unaffected.
