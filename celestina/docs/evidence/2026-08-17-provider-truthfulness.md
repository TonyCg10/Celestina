# Two providers that misread the machine

- **Date:** 2026-08-17
- **Scope:** Celestina unit `LIVE-1-B`
- **Artifact:** `celestina-provider-adapter`'s night light and
  `celestina-shell-core::network::parse_devices`
- **Environment:** read against the author's real three-output session and its
  own `nmcli` output; verified by `cargo test` and `cargo clippy` on
  `celestina-shell-core`
- **Plan:** [live session repairs](../plans/archive/2026-08-17-live-session-repairs.md)
- **Validation:** `VAL-R8`

## Procedure

The author reported small warm flickers while night light was on, and a
Bluetooth adapter that the shell had no business describing as a network
device. Both were traced by reading the code against the machine's real
command output rather than against an assumed shape.

## Result

### The night light rebuilt the controller it had just built

Destroying a `zwlr_gamma_control_v1` restores the compositor's own tables
instantly, by protocol. A teardown-and-rebuild is therefore a visible snap to
neutral followed by a fresh warm sweep — and the provider was doing it in a
loop.

`needs_reconciliation` counted an output as broken when its controller had no
`gamma_size` yet, which is the ordinary state of a controller that was just
created and has not been answered. The poll loop runs every 25 ms, so a
controller could be torn down and rebuilt before it ever reported its size,
and `reconcile_active_outputs` then re-ran a full nineteen-frame transition
across every output.

The more outputs, the likelier some output sits in that window at any poll,
which is why a single-output nest never showed it and three monitors did.

Two changes. A young controller is no longer mistaken for a broken one: only a
controller that reports `Failed`, or that has been invalidated, counts. And an
output whose controller genuinely fails now serves an exponential backoff —
one second, doubling to a thirty-second ceiling — which `needs_reconciliation`
honours, so a monitor that cannot hold a controller stops costing the session
anything visible. A controller that answers clears its own backoff, so a
transient hotplug failure keeps a quick first retry.

### An escaped separator was read as a field boundary

`parse_devices` split `nmcli -t` output on the raw `:` byte, while the same
file documents that nmcli escapes a colon inside a field as `\:` and provides
`split_terse` for exactly that. It is not a rare case in this listing: the
DEVICE column carries the Bluetooth adapter's MAC address.

The author's own row, `5C\:DC\:49\:0D\:D1\:62:bt:disconnected:`, parsed as a
device named `5C\` of kind `DC\` in state `49\`. Two tests carry that listing
verbatim. The first asserts the real link is still found — it is, because
`wlan0` lives on its own line, which is why this never hid the network
indicator and is recorded here rather than claimed as the cause of anything.
The second asserts the adapter survives as one device named
`5C:DC:49:0D:D1:62`.

The connection field is now joined back from everything after the third
separator, so a connection name containing a colon keeps it.

## Limits

The flicker repair was never reproduced deliberately: it is reasoned from the
protocol's semantics and the loop's own conditions, and covered by the
provider's existing tests rather than by a test that drives a failing output.
Whether the author still sees warm flickers is unmeasured, and is the check
this record most wants.

Neither repair says anything about night light's temperature, which is a
setting (`night_light_kelvin`) with no control surface yet — a gap, not a
defect, and not addressed here.
