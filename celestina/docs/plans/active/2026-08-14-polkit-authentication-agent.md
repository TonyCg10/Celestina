# Polkit authentication agent

- **Opened:** 2026-08-14
- **Plan ID:** polkit-authentication-agent
- **Status:** active
- **Scope:** celestina
- **Implementation checkpoint:** R8
- **Author-validation checkpoint:** VAL-R8

## Hypothesis

Celestina can serve this session's authorization prompts by registering as its
`org.freedesktop.PolicyKit1.AuthenticationAgent` and delegating every
verification to `polkit-agent-helper-1`, such that no failure of the agent can
produce an authorization that `polkitd` did not grant.

## Tangible outcome

An action that needs authorization — `pkexec`, a mount, a system setting —
raises a Celestina prompt naming the real action, and completes or is denied
according to what the system helper decided. With the agent stopped, the same
action fails exactly as it does on this machine today, which has no graphical
agent at all.

## Scope

- Registering and unregistering the agent for this session's subject, and
  surviving `polkitd` restarts.
- `BeginAuthentication` and `CancelAuthentication`, including several
  concurrent requests and the identities each one offers.
- The helper conversation: spawning `polkit-agent-helper-1`, writing cookie and
  response, reading the verdict, and treating everything unexpected as denial.
- The prompt surface: a dedicated layer surface with a keyboard grab, showing
  the action id, the message and the chosen identity as `polkitd` supplied
  them.
- Keeping the response out of the diagnostic journal, the provider channel, the
  shell's properties and every log this project owns.

## Exclusions

- Any PAM conversation of Celestina's own: the helper owns verification, and
  this plan never opens that boundary.
- Registering as the system-wide or any other session's agent.
- Remembering, caching or pre-filling a response.
- Policy authoring: this shell shows and forwards decisions, and defines no
  rules about who may do what.

## Build order

1. The helper conversation alone, driven by a fake request: spawn, cookie,
   response, verdict, and denial on every error branch. Testable headless.
2. Registration and the `polkitd` interface, including re-registration after a
   `polkitd` restart and cancellation.
3. The prompt surface and its grab, including the refusal to prompt when the
   grab cannot be taken.
4. The concurrency and identity-selection cases.

## Implementation exit

An offscreen regression drives the helper conversation for success, wrong
response, cancellation and an unavailable helper, and proves the response
appears on no stream this project writes. An interface regression proves
registration, re-registration after a restart, cancellation and two concurrent
requests. A surface regression proves the prompt refuses rather than degrades
when it cannot hold the keyboard. Real `pkexec` against a real password is
`VAL-R8` and does not block this exit.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| R8-P-A | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-A.numstat.tsv) | 12 files, +1065/-7 | verification is delegated whole, and every error denies | [helper conversation](../../evidence/2026-08-14-polkit-helper-conversation.md) | `VAL-R8` |
| R8-P-B | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-B.numstat.tsv) | 10 files, +967/-4 | the session has an agent that survives a `polkitd` restart | [agent registration](../../evidence/2026-08-14-polkit-agent-registration.md) | `VAL-R8` |
| R8-P-C | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-C.numstat.tsv) | 16 files, +1072/-13 | the prompt names the real action and refuses to show without a grab | [prompt surface](../../evidence/2026-08-14-polkit-prompt-surface.md) | `VAL-R8` |
| R8-P-D | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-D.numstat.tsv) | 11 files, +273/-177 | the production build verifies again, so the departure can be deployed at all | [production contract breaches](../../evidence/2026-08-15-production-contract-breaches.md) | `VAL-R8` |
| R8-P-E | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-E.numstat.tsv) | 12 files, +176/-28 | the deployed bundle carries the lock and the polkit child, and the handover knows the agent exists | [bundle carries the lock and the agent](../../evidence/2026-08-15-bundle-carries-the-lock-and-the-agent.md) | `VAL-R8` |
| R8-P-F | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-F.numstat.tsv) | 9 files, +105/-9 | the first live prompt's two defects: the card cut to its header, and a stray click spending the request | [first live prompt](../../evidence/2026-08-15-first-live-prompt.md) | `VAL-R8` |
| R8-P-G | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-G.numstat.tsv) | 12 files, +165/-12 | the card uses the overlays' real anatomy, and the prompt opens on the focused output | [reproduced with grim](../../evidence/2026-08-15-prompt-reproduced-with-grim.md) | `VAL-R8` |
| R8-P-H | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-H.numstat.tsv) | 15 files, +205/-16 | the hover/press race, the keybind route's imaginary cursor, and the prompt's missing material — the first live day's findings | [first live session findings](../../evidence/2026-08-15-first-live-session-findings.md) | `VAL-R8` |
| R8-P-I | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-I.numstat.tsv) | 8 files, +104/-6 | resting dense-glass companions stop saturating the whole session | [resting companions record](../../evidence/2026-08-15-resting-companions-saturated-the-session.md) | `VAL-R8` |
| R8-P-J | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-J.numstat.tsv) | 8 files, +98/-3 | a wallpaper change keeps the old image until the new one is ready | [wallpaper switch record](../../evidence/2026-08-15-wallpaper-switch-black-frame.md) | `VAL-R8` |
| R8-P-K | `celestina:` | done | [inventory](../../inventories/2026-08-14-polkit-authentication-agent/R8-P-K.numstat.tsv) | 19 files, +403/-15 | surfaces form and leave as one block, on presented frames, with one shared departure | [one block record](../../evidence/2026-08-15-one-block-forming-and-leaving.md) | `VAL-R8` |

## What R8-P-A found about the helper

The plan said "spawning `polkit-agent-helper-1`", and on this machine that is
not possible: the binary is not setuid and refuses to run that way. It is
reached over `/run/polkit/agent-helper.socket`, socket-activated by systemd,
and `libpolkit-agent-1` is where polkit keeps the choice between the two
transports. The conversation child therefore links that library instead of
writing a copy of a decision this project does not own. The delegation ADR
0005 requires is unchanged in every respect that matters: the helper still
runs PAM as root, still tells polkitd itself, and nothing in this repository
can produce an authorization it did not grant.

## Why R8-P-B does not register the live shell

The agent is built and exercised, and the shell does not start it. A
registered agent receives real requests, and until `R8-P-C` there is nothing
to show them on: a `pkexec` that hangs waiting for a prompt nobody can see is
worse than this machine's current behaviour, which is to fail immediately
because no graphical agent exists. Registration therefore lands with the
surface, in one commit where the capability is whole.

## What R8-P-C found about this machine

The plan's tangible outcome says that with the agent stopped an action fails
"exactly as it does on this machine today, which has no graphical agent at
all". That is not true: Noctalia runs a `polkit-agent` plugin and holds this
session's single agent slot, so Celestina's registration is refused — by
design, since polkitd accepts one agent per session and this shell does not
fight for it.

The consequence is that `VAL-R8` has one shape rather than two. A real
`pkexec` against a real password cannot be tried while Noctalia is running, so
the prompt's first real use and the departure it was built for are the same
validation.

## Why R8-P-D is in this plan

R8 owns the departure, and the departure cannot happen without a production
build. `verify-production.sh` refused on two contract breaches that had been
sitting in the tree since the surfaces they belong to were written: five
animations naming `Easing.OutCubic` directly instead of the `easeStandard`
token, and `qml/LockScreen.qml` living in the shell module's own directory
while belonging to the lock's separate executable — a file that directory's
rule can never accept, because every file in it must be registered in the
shell's module and this one cannot be.

Neither is a polkit defect. Both are what stood between this checkpoint and a
deployable shell, which is why they are closed here rather than filed for a
plan that would have to open first.
