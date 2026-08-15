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
| R8-P-B | `celestina:` | planned | agent registration and the `polkitd` interface | — | the session has an agent that survives a `polkitd` restart | interface regression incl. re-registration, cancellation, concurrency | `VAL-R8` |
| R8-P-C | `celestina:` | planned | the prompt surface and its keyboard grab | — | the prompt names the real action and refuses to show without a grab | offscreen surface regression | `VAL-R8` |

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
