# Suite decisions

Accepted decisions explain durable architecture and workflow. They do not grant
authority or replace operational rules.

| ADR | Status | Decision |
|---|---|---|
| [0001](0001-vendor-neutral-agent-contract.md) | accepted | One vendor-neutral root agent contract |
| [0002](0002-independent-delivery-lanes.md) | accepted | Separate implementation and author-validation lanes |
| [0003](0003-reusable-production-artifacts.md) | accepted | Build once, verify, deploy, and compare the same bytes |
| [0004](0004-monorepo-change-ledger.md) | accepted | Durable exact change ledger for one monorepo history |
| [0005](0005-bounded-qt-bridge-crates.md) | accepted | Keep Qt bridge exceptions narrow and explicit |
| [0006](0006-source-first-library-navigation.md) | accepted | Navigate the standalone media library by configured source |
| [0007](0007-spanish-product-copy.md) | accepted | Product copy is Spanish; development truth stays English |

An accepted decision is superseded by another ADR, never rewritten to hide its
historical verdict. Every ADR retains at least Context, Decision, Consequences,
and Revisit conditions. Link to discussions and plans instead of duplicating
their detail.

Project-local decisions stay in their owner's `docs/decisions/`; this index does
not duplicate them. Each local index links every record exactly once and keeps
its lifecycle current.
