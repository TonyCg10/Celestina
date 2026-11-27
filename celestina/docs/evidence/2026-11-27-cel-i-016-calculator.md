# Evidence: the launcher's calculator (CEL-I-016)

- **Date:** 2026-11-27
- **Scope:** `CEL-I-016`: `celestina-rs/crates/celestina-shell/src/calc.rs`, `celestina/crates/celestina-paint/src/theme.rs`, `celestina/src/clipboard/{worker,mod}.rs`, `celestina/src/launcher/{state,view,mod}.rs`, `celestina/src/copy.rs`, `celestina/src/app.rs`, the ledger row and `VAL-CEL-I-016`
- **Environment:** worktree `celestina-CEL-I-016`, rebased on `main` at `8be8690a`; debug and release builds in private target directories; the nest ran on its own bus with the clipboard duty on
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the launcher preview the author approved on 2026-11-26, private artifact <https://claude.ai/artifact/QoGCyzhoqoM5P4pH2ASHWB>; [design](../../../docs/superpowers/specs/2026-11-26-celestina-launcher-design.md) §5.7, [plan](../../../docs/superpowers/plans/2026-11-26-celestina-cel-i-016-plan.md)

## Change

- **Parser:** `celestina_shell::calc` evaluates numbers with a comma or a point, `+ - * / × ÷ ^ %`, parentheses, implicit multiplication before a parenthesis or a function, `sqrt √ sin cos tan ln log` and `pi π e`. The query is an expression only when it parses whole, has a binary operator, a percent or a function, and gives a finite result. Input is bounded at 256 characters and 32 nested parentheses. The result keeps 12 significant digits, with a comma and an exponent past 1e15 or below 1e-9.
- **Success role:** `Role::Success` (the scheme's `success`, `#8ee0b4`).
- **Clipboard:** `ClipboardHandle::offer_text` and `ClipboardDuty::offer_text` ask the worker to own the selection with a text. The worker records the text as a copy unless the history is paused, never reads its own offer back (the identity MIME, as for a re-offered entry), and refuses a text it cannot keep. The command's `Debug` never prints the text.
- **Launcher:** `Row::Calc` comes first and selected. Enter or a click returns `LauncherAction::Copy`, which does not close: the glyph turns to `check`, the detail turns to the copied word in the success colour, and the app closes the launcher 350 ms later. With the clipboard duty off, Enter does nothing. When the clipboard refuses the text, the row goes back and Enter can try again.

## Procedure

```sh
cd celestina
cargo fmt --all -- --check                                       # ok
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings   # clean (CI's stable)
cargo test --workspace --locked --no-fail-fast   # 2792 passed, 1 failed, 2 ignored
```

- **The one failure** is `celestina-lock`'s load-sensitive `a_sigterm_right_after_the_spawn_does_not_kill_the_lock`; run alone on a quiet machine, its binary passes 3 of 3.
- **Red steps.** The parser's tests failed with stubbed functions. The clipboard's tests failed to compile before the change, and four mutations (pause ignored, identity dropped, recording while paused, no refusal) were each caught. The launcher's tests failed to compile before the row existed.
- **The nest:** the guarded kit, nested niri at scale 1.5, `LANG=es_US.UTF-8`, the clipboard duty on. The probe typed `2*(3+4)^2`, pressed Enter, ran `wl-paste` inside the nest, then reopened the launcher and typed `80*15%`.
- **The smoke:** `smoke.sh --binary <release> --only launcher` on a release build in a private target: `smoke: OK (only: launcher)`.

## Result

- [row.png](./2026-11-27-cel-i-016-calculator/row.png): `= 98` as the first row, with the copy word and the Enter key cap.
- [copied.png](./2026-11-27-cel-i-016-calculator/copied.png): 120 ms after Enter, the check glyph and the copied word in the success colour.
- [closed.png](./2026-11-27-cel-i-016-calculator/closed.png): the launcher closed a second later.
- **Paste:** `wl-paste` inside the nest printed `98`.
- [percent.png](./2026-11-27-cel-i-016-calculator/percent.png): `80*15%` gives `= 12`.
- **Review:** 1 round (`/code-review` at high reasoning, Opus; the launcher and parser implementers were Sonnet, the clipboard implementer Opus):
  - fixed: a refused copy left the row marked copied and closed the launcher;
  - noted: the key cap uses the 12 px secondary size and the plain divider (the preview: 11 px and a stronger hairline the paint crate lacks).

## Limits

- The row's entrance animation was not checked frame by frame.
- The worker's refusal of a text arrives asynchronously and only reaches the log; a calculator result is always short enough to be kept, so it does not reach the row.

## Follow-up

None.

## Landing

- **Base revision:** `bc4b78078a019bbe44ca48175ac78d414114c56b`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases launcher only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:1e11f76d2f57f3214a558053acfd97fcd5e0fbe1a67f0a35ce125d842a3da66a, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8731a7a17db7376842d494426d0118ee0687c295cdbe2da22108cc682dc5fdd1, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
