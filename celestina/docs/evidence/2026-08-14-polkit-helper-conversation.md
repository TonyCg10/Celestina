# The conversation that asks, and never decides

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R8-P-A`
- **Artifact:** Celestina 0.27.0, `celestina-polkit-converse` and
  `PolkitConversation`
- **Environment:** the repository's automated suite on the author's machine,
  plus one run of the real child against this machine's real polkit helper. No
  password was typed and no action was authorized.
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

The rule this unit exists to hold is one sentence: nothing in this repository
can produce an authorization polkitd did not grant. Every case below drives
the conversation to a point where a careless implementation would report
success anyway, and asserts a denial instead.

## Procedure

`PolkitConversation` was driven against stand-in children whose behaviour is
dictated per case — one that authenticates, one that denies, one that dies,
one that exits with a code this shell does not define, one that is not there,
and one that records what reached it. The real child was then run once against
this machine's real helper with a cookie polkitd never issued.

## Result

Eleven regressions, all passing, and the suite at 21/21.

- **Only exit code 0 authorizes.** A denial, a crash, an undefined exit code
  and a missing child all answer something other than `Authenticated`.
- **A cancelled prompt answers nothing at all.** No verdict is emitted, so
  nothing downstream can mistake an abandoned attempt for a denial the person
  saw or an authorization they were given.
- **The response reaches only the child's input.** It arrives on stdin, and
  the child's command line — which every process on this machine can read out
  of `/proc` — contains neither the response nor the cookie.
- **PAM's own words arrive unaltered**, including a message with a newline in
  it, which is why every frame on the wire is percent-encoded: a prompt must
  not be able to forge a second event, and the shell must not paraphrase what
  the stack asked.
- **A second conversation while one is in flight is denied**, not queued.

### The real helper, without a password

    $ printf 'not-a-real-cookie\\n' | celestina-polkit-converse --user toni
    secret Password%3A%20
    exit=1

The child reached the real helper, PAM asked for a real password, and closing
the pipe without answering ended as a denial. That is the whole transport
proven end to end with nothing secret involved.

### What the plan said, and what the machine said

The plan said "spawning `polkit-agent-helper-1`". On this machine that cannot
work: the binary is not setuid — it says so and exits — because polkit now
reaches it over `/run/polkit/agent-helper.socket`, socket-activated by
systemd. Which of the two transports a machine offers is polkit's business and
changes between releases, and `libpolkit-agent-1` is where polkit keeps that
knowledge, so the child links the library rather than hand-writing a decision
this project does not own. The delegation ADR 0005 requires is unchanged: the
helper still runs PAM as root and still answers polkitd itself.

### Two defects the suite surfaced, neither in this unit's code

- **The shell-service regression had not built since `R6-D`.** Wiring the
  session verbs to `LockController` left that test target linking against a
  symbol it never compiled. The shell itself linked, so nothing said the test
  was gone — it simply stopped being run, which is the failure mode of a
  broken build that only one target notices.
- **`Qt::UniqueConnection` with a lambda aborted the surface-manager
  regression.** It is documented not to deduplicate functors and asserts in a
  debug build instead. The dense-glass aggregator now connects the destroyed
  hook once, when a source is first seen, which is what the flag was standing
  in for.

Both are corrected here rather than filed, because a suite with a target that
does not build and a test that aborts is one nobody can read a new unit's
result out of.

## Limits

No real authorization happened. Nothing here says `pkexec` prompts, that the
prompt is readable, or that the right person is asked — there is no agent
registered yet, which is `R8-P-B`, and a real action against a real password
is `VAL-R8`.

The real-helper run above proves the transport on this machine only. A machine
with a setuid helper and no socket takes the library's other path, which is
exercised nowhere in this repository.
