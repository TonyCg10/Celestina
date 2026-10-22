# ADR 0019: No secret passes through the shell

- **Date:** 2026-10-17
- **Status:** proposed

## Context

A saved Wi-Fi network can need a passphrase that NetworkManager does not
hold, and a Wi-Fi network that is not saved needs one to join. The shell is a
desktop surface; a text field for a password in it would make the shell a
credential store, and an agent would make it a second secret agent beside the
keyring and `nm-applet`.

The status stays `proposed` until the author sets it: this directory's
convention names no case where a session accepts a record, and the session
that wrote it did not ask the author to accept it.

## Decision

- The shell calls `connect(id, None)` only. The action type has no field that
  could carry a secret, and a unit test pins that the network worker never
  forwards `Some`.
- The shell registers no secret agent. NetworkManager asks the user's agent
  (`nm-applet`, polkit, the keyring) as it does today.
- A saved network that needs a secret NetworkManager does not hold fails with
  `NeedsSecret`; the row says so and offers `Editar…`, which opens
  `nm-connection-editor`.
- The shell never reads or logs connection settings. The connectivity code's
  journal lines carry fixed tokens only: an error kind (`error_kind`,
  `PowerError::kind`), a step kind (`step_kind`), a refusal key, a typed
  failure or a typed request error. They never carry a network or device name, a row id or address, or
  text the bus or a daemon returned. Two lines outside that rule hold no
  service data: the spawn error of a worker thread (an OS error) and the
  editor's start error (the program name and an OS error).

## Consequences

- The shell cannot join a protected network that is not saved; the person
  does it in `nm-applet`.
- The unit's tests and the author's rows check that the shell never asks for a
  password (VAL-CEL2-NETWORK).

## Revisit when

The author wants passwords entered in the shell. That is a new decision with
its own agent and storage design, and it is not a change of this unit.
