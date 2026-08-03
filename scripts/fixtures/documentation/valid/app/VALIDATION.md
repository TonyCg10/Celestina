# App author validation

## VAL-APP-1 — Observed hardware failure

- **Status:** failed
- **Related implementation:** APP-1
- **Requires:** fixture hardware
- **Procedure:** exercise the hardware path
- **Pass condition:** the hardware reports one confirmed state
- **Result:** the fixture reported the expected simulated failure
- **Evidence:** [Fixture evidence](docs/evidence/2026-08-03-fixture.md)
- **Remediation:** `APP-1B` in [the active app plan](docs/plans/active/2026-08-03-app.md)
