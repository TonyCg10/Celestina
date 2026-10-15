# ADR 0006: Own the night-light gamma transition

- **Date:** 2026-08-17
- **Status:** accepted

## Context

ADR 0001 retained a bounded `wlsunset` process at 2700 K until gamma failed to
release cleanly or the lived result became unsuitable. The process does release
gamma, but that lifecycle is itself the reproduced defect: startup applies the
warm lookup table in one compositor commit, and killing the process restores
identity in one commit. The resulting whole-output color step is perceptible as
a brief warm flash even when an ordinary screen recording barely captures it.

The subprocess boundary also cannot confirm the effect it reports. A successful
spawn was published as `active` before `wlsunset` had acquired each output's
gamma control. If the compositor refused that control, the UI could claim the
state for up to the session-hold polling interval before the child exit was
observed.

## Decision

Celestina owns the fixed night-light transition through
`wlr-gamma-control-unstable-v1` inside its existing aggregate Rust provider.
It does not add a daemon, scheduler, location policy or second provider process.

- The final warm white point remains the established 2700 K value.
- A bounded monotonic transition moves every available output between identity
  and that white point. One Wayland-owning worker creates, updates and releases
  every gamma-control object; the command thread never touches those proxies.
- Disabling reaches and commits identity before releasing the controls, so the
  protocol's automatic reset has no visible distance left to travel.
- `active` is provider-confirmed state: it is published and persisted only
  after the final warm commit succeeds. Missing protocol support, an invalid
  ramp size or a compositor `failed` event is a refusal, never a claimed state.
- The control-centre switch paints only the provider's confirmed value. An
  activation requests the transition without first moving the thumb locally.
- Caffeine remains the external `systemd-inhibit` hold. Similar process shape
  does not justify sharing ownership once night light has a stateful Wayland
  protocol and a temporal transition.

## Consequences

- This record supersedes only the night-light row of ADR 0001. Its other
  execution defaults and falsifiers remain accepted.
- The dependency on the installed `wlsunset` executable ends for night light;
  the provider gains the narrow wlroots gamma protocol binding instead.
- A compositor backend that does not advertise gamma control, including a
  nested Niri winit backend, truthfully exposes no night-light provider. The
  development nest can validate refusal and UI stability but not the physical
  color ramp.
- A crash still lets the compositor restore identity immediately. Avoiding that
  failure step would require a separate long-lived authority and is not worth a
  new daemon for this interaction.
- Gamma control is exclusive per output. A competing client is reported as a
  failure rather than displaced or hidden.

## Revisit when

Revisit the direct protocol only if Niri removes wlroots gamma control, a stable
standard protocol replaces it, or measured hardware behavior still contains a
visible step between consecutive lookup tables. Scheduling, per-output color
temperature and ambient-light policy remain separate product decisions.
