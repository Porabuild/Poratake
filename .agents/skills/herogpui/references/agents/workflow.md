# Workflow and architecture

Read this guide for every implementation task. It defines where code belongs,
which sources are authoritative, and how much verification a change needs.

## Start from the current checkout

```powershell
git status --short
git branch --show-current
git diff -- <paths-you-will-touch>
```

This repository commonly contains unfinished component, gallery, audit, and
screenshot work. Treat every pre-existing modification as user-owned. Do not
format, revert, regenerate, or include unrelated paths. A read-only review does
not authorize edits, builds, formatting, or test runs unless the request says
otherwise.

Read before writing:

1. The complete target file and the relevant `impl` block.
2. Callers and sibling components that establish the local pattern.
3. Focused tests in `crates/herogpui-components/tests/`.
4. The exact pinned upstream source when the change claims parity.
5. The current diff again after editing.

Do not turn a focused fix into a refactor. Delete unsupported or obsolete code
instead of leaving compatibility aliases, no-op builders, or speculative flags.

## Repository map

- `crates/herogpui-core` owns shared v3 vocabularies and color math. It contains
  no component code.
- `crates/herogpui-theme` owns semantic and layout tokens. `ThemeProvider` is a
  GPUI global; consumers read it with `ActiveTheme` (`cx.colors()`,
  `cx.role(..)`, `cx.layout()`).
- `crates/herogpui-components` owns the component implementations and headless
  behavior tests. Components are builder structs implementing `RenderOnce`.
- `crates/herogpui` is the facade a consumer depends on, and the only crate a
  consumer names: it re-exports all of `gpui` at its root, `gpui_platform` as
  `platform` (and `application`), and each HeroGPUI layer as a feature-gated
  named module (`core`, `theme`, `components`). It also owns the crate-level
  `init` and `prelude`, and its own copy of GPUI's `actions!` macro — the
  upstream macro expands `gpui::Action` as an absolute path, which does not
  resolve for a consumer that reaches GPUI only through this crate. Its
  doctests are the only place the published installation contract is compiled,
  so `cargo test --doc -p herogpui` is part of verifying a change to it. The
  layering below it stays as it is; the facade is a layer on top, not a merge.
- `gallery` is the documentation app. Page routing and categories live in
  `gallery/src/pages/mod.rs`; component demos live primarily in
  `gallery/src/pages/components/`; checked-in v3.2.7 API metadata lives in
  `gallery/src/pages/reference_metadata/`.
- `.shots` contains parity audits, headless gallery drivers, reference images,
  and the real lint gate. There is no setup step and no generated source
  tree: GPUI comes from the published registry family, so `cargo` works in a
  fresh clone with no command before it.
- `llms.txt` is the public API reference intended for LLM consumers.

## Source hierarchy

Use the narrowest source that actually owns the contract:

1. This checkout's tests and code for its current behavior and supported API.
2. HeroUI v3.2.7 component code and styles for port parity.
3. HeroUI's exact pinned dependencies for inherited interaction semantics:
   React Aria 3.53.1, React Stately 3.51.0, and React Aria Components 1.22.1.
4. The `gpui-pre` 0.3.11 sources pinned in `Cargo.lock` for framework
   behavior and available APIs. Cargo unpacks them to
   `~/.cargo/registry/src/index.crates.io-*/gpui-pre-0.3.11/`; read that tree,
   not a Zed git checkout, not the older `gpui-unofficial` republish, and not
   the unrelated crates.io `gpui` 0.2.2 crate.
5. Other Zed revisions, GPUI republish versions, or GPUI projects only as
   precedent, never as proof that the pinned version supports an API.

The live `https://heroui.com/react/llms-full.txt` bundle is an input to several
audits, but tagged source and the checked-in reference metadata establish the
repository's v3.2.7 contract. A green audit is only evidence for what that audit
actually reads.

## Project invariants

- HeroUI v3 only. Do not reintroduce v2 tokens, components, or props.
- Keep upstream OKLCH values and component metrics exact; do not aesthetically
  "improve" a parity value.
- Builder names follow v3 prop names in Rust spelling. This is why the workspace
  allows Clippy's `wrong_self_convention` lint.
- Callback fields cloned into GPUI closures use `Arc<dyn Fn ...>` rather than
  `Box<dyn Fn ...>`.
- Interactive elements need unique ids per page and per component instance.
- Every workspace crate must inherit `[workspace.lints]` with
  `[lints] workspace = true`; `bash .shots/lint.sh` enforces this.
- Do not copy totals from audit output into instructions. Route, prop, example,
  and coverage counts change; let the scripts print the current values.

## Verification by change type

| Change                                | Iterate with                                                                                                                                        | Before broad handoff                                                         |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Rust logic in one component           | Focused `cargo test -p herogpui-components --test suite_<area> <module>::` (see the [test index](../../crates/herogpui-components/tests/README.md)) | Component suite, format, lint, relevant audits                               |
| Public builder/API                    | Focused tests plus `api_audit.py`, `extra_audit.py`, `write_only.py`                                                                                | All audits and package checks when release-facing                            |
| Tokens or component metrics           | `token_audit.py`, `design_audit.py`, focused screenshots                                                                                            | Format, lint, affected behavior tests, visual check                          |
| Interaction, focus, overlay, or state | Focused behavior binary                                                                                                                             | Gallery drive for the real path, the interaction sweep, then component suite |
| Gallery demo or reference metadata    | Gallery tests and relevant demo/reference audit                                                                                                     | Rebuild, route smoke, focused capture                                        |
| Audit parser or mapping               | Run the changed audit against known-positive and known-negative input                                                                               | Run every `*audit.py` as CI does                                             |
| Documentation only                    | Check links, commands, and current file names                                                                                                       | No Rust build unless the documentation changes code generation               |

Normal tests are one-shot. `bacon` is the opt-in persistent check loop; do not
replace ordinary test commands with watch mode.

## CI-shaped verification

`.github/workflows/ci.yml` is authoritative. Every job that runs cargo does so
on a plain checkout with no setup step; locally the same holds, so the first
command in a fresh clone is just `cargo`. Its jobs cover:

- Formatting, generated gallery/WASM freshness, and website typecheck, lint
  and production build.
- Strict parity reports, inventory freshness, reader regressions and web font
  subset checks. The aggregate runs every `*audit.py` plus `write_only.py` and
  checks reported gaps as well as exit codes; see [parity](parity.md).
- Workspace lint inheritance, Clippy and cargo-deny (`bash .shots/lint.sh`,
  which runs on any host with bash), unused dependencies, each-feature
  compilation, and each library crate with `--no-default-features`.
- Workspace tests through `.shots/run-tests.sh` on Linux, macOS and Windows,
  plus the theme's optional serde feature. Optimized (`Cargo.toml` profile)
  tests run on pushes and manual dispatches, and on a pull request labelled
  `ci:opt-levels` or touching build configuration. The Windows job also
  exercises the gallery control-file helpers with `.shots/test-control.ps1`.
- On pull requests, `cargo semver-checks` for the four library crates against
  their latest crates.io release; a deliberate break carries the
  `semver:breaking` label. The release repeats the check before publishing.
- Rustdoc, gallery CLI installation from source, and a wasm32 build on the
  pinned toolchain followed by the lockfile-matched wasm-bindgen pass.

Do not claim the full gate passed after running only a focused test or one audit.
For documentation-only changes, verify the documentation directly rather than
spending minutes compiling unrelated Rust.

## Finish with evidence

Re-run `git status --short` and inspect the exact diff. Report which checks ran,
which did not, and why. A screenshot proves pixels, a headless behavior test
proves the exercised event path, and an audit proves only its mapped surface;
none substitutes for the others.
