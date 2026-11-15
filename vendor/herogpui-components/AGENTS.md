# Component crate instructions

These rules apply under `crates/herogpui-components` in addition to the root
guide.

Before editing, read [component implementation](../../docs/agents/components.md)
and the focused test binary that owns the behavior. For HeroUI API, anatomy,
design, state, or behavior claims, also read
[parity and audits](../../docs/agents/parity.md).

- Keep component fixes in the owning module and reuse existing `util` helpers.
  Add a shared helper only when multiple real call sites require the same
  contract.
- Verify GPUI APIs against the exact Zed git revision in `Cargo.lock`.
- Preserve controlled/uncontrolled semantics, per-instance keyed state, unique
  ids, disabled tab behavior, and topmost-overlay arbitration.
- Add or update a focused headless test for logic changes. An audit mapping or
  screenshot is not a substitute for exercised interaction.
- Run the relevant parity audits when a builder, part, token, metric, state,
  motion, or behavior surface changes.

Iterate with `cargo test -p herogpui-components --test <binary>` and finish with
the broader gates justified by the change. After a visual or composition change,
follow the root gallery rebuild/drive workflow.
