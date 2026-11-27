# Evidence: a held key repeats (CEL-I-018)

- **Date:** 2026-11-27
- **Scope:** `CEL-I-018`: `celestina/crates/celestina-wayland/src/{handlers,input,popup,runtime}.rs`, `celestina/crates/celestina-ui/src/{input,slider}.rs` and `tests/field.rs`, `celestina/src/launcher/mod.rs`, `src/island/{mod,menu}.rs`, `src/tray/menu.rs`, `src/island/sheets.rs`, `src/sheet/{mod,body}.rs`, `src/osd/popup.rs`, `crates/celestina-ui/src/secret.rs` and `tests/secret.rs`, tests in `src/overview/mod.rs` and `src/polkit/state/tests.rs`, the ledger row and `VAL-CEL-I-018`
- **Environment:** worktree `celestina-CEL-I-018` rebased on `main` at `8be8690a` (with `CEL-I-014` and `CEL-I-015`); nested niri on the nest's own bus; debug and release builds in a private target directory
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** none; nothing changes on screen except that a held key keeps acting where it should

## Cause

The runtime created the keyboard with `SeatState::get_keyboard(qh, &seat, None)`. In smithay-client-toolkit 0.21 only a keyboard made with `get_keyboard_with_repeat` repeats: the toolkit then runs a calloop timer with the compositor's `repeat_info` and calls the repeat callback it was given. A held key acted once everywhere: the arrows in the launcher and the Island, and letters and Backspace in every field. The launcher session found it on 2026-11-27 in a nested niri: `wtype -P a` held 1.5 s typed one `a`.

## Change

- The runtime makes the keyboard with `get_keyboard_with_repeat` on its own event loop. The toolkit takes the delay and rate from the compositor's `repeat_info`, and stops on the key's release, on a keyboard leave and when the focused surface dies. The callback ignores a keyboard the runtime has released. The keymap keeps modifiers from repeating.
- A repeat is its own event, `InputEvent::KeyRepeat`, with the serial of the key's press, and it takes no new grab serial. It goes where a press goes: to the open popup first, else to the focused surface. Its `Debug` output hides typed text as a press's does.
- Only code that asks for repeats acts on them:
  - a focused text field's edits and a focused slider's steps (`celestina-ui`);
  - the launcher's Up and Down, and its field;
  - Up and Down in the Island's and the tray's menus;
  - the surfaces that hand keys to `celestina-ui` (the Island's page, the Island's sheet input and the menu windows behind it, the on-screen display's popup) forward them.
- Everything that acts on a press ignores repeats, so it acts once:
  - Return, Enter and Space activate once, Escape closes one level, Tab moves the focus once;
  - Delete in the window selector asks to close one window;
  - Tab and `1`/`2` in the overview switch its mode once;
  - polkit's password field takes no repeat, so a key held while the dialog appears cannot type into it after the focus guard; the secret field wipes a repeat's text as it wipes a release's;
  - the session lock keeps its keyboard as it was, with no repeat (the author's choice on 2026-11-27).
- The launcher's hold-Backspace (`CEL-I-014`) keeps its own timing: a repeat while the key is held starts the sweep at once, repeats during the sweep do nothing, and with no hold a repeat deletes as the field does.

The unit's first version filtered Return, KP_Enter, Escape and Space out of the repeats. Review round 1 showed three press-acting keys that the filter missed: Delete in the selector, Tab in the overview, and polkit's focus guard. The author chose the repeat event instead on 2026-11-27.

## Procedure

```sh
cd celestina
cargo fmt --all -- --check                                       # ok
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo test --workspace --locked --no-fail-fast   # 2785 passed, 0 failed, 2 ignored
```

The counts are the sum of every `test result` line (48 lines). The new tests failed before the change (RED): compile errors in `celestina-wayland` and as failures in `celestina-ui` (slider) and the launcher (4 of 5). The selector's Delete test passed from the start, because a repeat matched no arm there. The overview and polkit tests pass with no change in their code: they prove the two ignore repeats. Review round 2 found the Island's sheet input dropping every repeat; `a_held_arrow_keeps_stepping_a_level_sheets_focused_slider` failed before its arm and passes after it. The secret field's repeat test passed before its fix too: the wipe it adds cannot be seen from outside.

One run of the full suite failed `connectivity::tests::failed_retries_start_no_watcher_and_a_lane_keeps_one_for_life` and `providers::brightness::tests::after_the_piece_leaves_a_redetection_reads_nothing` under load. Both passed when the suite ran again. Another run failed 13 of the lock's process tests because the session filled the `/tmp` tmpfs with its own build directory; with the build directory moved to disk they passed, and the second passed 5 times in 5 alone. Neither touches input.

The nest used the guarded kit of `CEL-I-014`: `env -i`, `--own-bus`, fake `wpctl` and `pactl`, the nest lock, and nested niri with its default repeat (delay 600 ms, rate 25 per second). `wtype -P KEY` holds a key and `wtype -p KEY` releases it. The shots are of the release build of the branch's last code commit, rebased on `8be8690a`. The launcher showed the host's applications, eight rows. Its arrows wrap around the rows, so the ninth step lands on the first row again.

## Result

- [launcher-held-down.png](./2026-11-27-cel-i-018-key-repeat/launcher-held-down.png): the launcher opened fresh each time (first row selected), Down held for 0.40, 0.68, 0.76 and 0.84 s. The selection ends on row 2 (the press alone, before the 600 ms delay), then rows 5 and 7, then wraps to row 1 after eight steps. That is 25 steps a second after the delay.
- [field-held-keys.png](./2026-11-27-cel-i-018-key-repeat/field-held-keys.png), top to bottom:
  - `a` held for 1.0 s types 11 letters (1 + 0.4 s × 25);
  - held 2.0 s more, it types 36 more (1 + 1.4 s × 25);
  - a Backspace tap deletes one letter;
  - Backspace held: 0.8 s in, the sweep marks the letters it is taking;
  - at 1.5 s the query is empty, as `CEL-I-014` designed.
- A debug build repeats Down at less than 25 a second. The toolkit starts each gap after the callback returns, and the debug launcher takes long to lay out its rows. The release build keeps the rate.

## Limits

- One output, the nest's virtual keyboard; the author's own keyboard and settings are `VAL-CEL-I-018`.
- Held Return, Escape, Space, Tab, the selector's Delete and the overview were not tried in the nest; unit tests cover them.
- A held Space now types spaces in a field, and a held Delete deletes forward in it (both through the field).

## Follow-up

- The two tests that failed under load above.
- Repeat in the session lock's and polkit's password fields, if the author wants it.
- The lock's runtime maps a `wl_seat` v10 compositor-side repeat (`repeat_key`) to a press (`crates/celestina-wayland/src/lock/handlers.rs`), so on such a compositor a held key would repeat in the lock's password field. That mapping predates this unit, which leaves the lock alone; review round 3 found it.

## Landing

- **Base revision:** `8be8690a801a8225b6a4826314de2f062f0633a4`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:16aaa597f989bba3f4ccd7d387676bf4d0605c5ef64db5c7391d1ae0903c70be, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
