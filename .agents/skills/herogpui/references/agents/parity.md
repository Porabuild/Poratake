# HeroUI v3 parity and audits

Read this guide when changing a public builder, documented behavior, styling,
tokens, motion, component anatomy, demos, reference metadata, or an audit.

## Contract and exclusions

This repository ports HeroUI v3.2.7. Use tagged HeroUI source for component
anatomy and styles, and the exact dependency versions HeroUI pins for inherited
behavior: React Aria 3.53.1, React Stately 3.51.0, and React Aria Components
1.22.1. GPUI framework claims must be valid for the `gpui-pre` version in
`Cargo.lock`, read from its unpacked registry sources.

Do not reintroduce v2 concepts:

- `content1` through `content4`, numbered color scales, or primary/secondary as
  color roles; v3 uses surfaces, semantic roles, and accent.
- The removed `radius` prop or the v3 field `size` props. HeroGPUI-only
  additive size enums (`SliderSize` is the precedent; `CheckboxSize`,
  `RadioSize`, `TabsSize`) are allowed only under this narrow exception: every
  step is defined in the component's own docs, `Md` is byte-identical to the
  pinned default, the builder is recorded in `extra_audit.py` as a repository
  extension, and it never appears in `reference_metadata` or presents itself
  as a v3 prop. The same exception covers per-component `radius` builders that
  replace one owning `util::*_radius` helper: the default stays the helper,
  an `sx` corner still wins per corner over the instance radius, and the
  builder is recorded the same way.
- V2-only spellings such as `Divider`, `DateInput`, `Progress`,
  `CircularProgress`, `NumberInput`, or `isLoading`.
- Removed props including `isStriped`, `isBordered`, `isPressable`,
  `isHoverable`, `isBlurred`, `isLoaded`, `isExternal`, `underline`,
  `showOutline`, `isInvisible`, `strokeWidth`, and `hideSeparator`.

`extra_audit.py` is the mechanical guard against v2 leftovers and invented API.

## Choose the audit that owns the claim

| Command                              | What it proves                                                                                                                   |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| `python .shots/api_audit.py`         | Every documented v3 prop/part row maps to the correct Rust owner and builder/constructor                                         |
| `python .shots/extra_audit.py`       | Exported builders are documented by v3 or have a narrow allowed repository reason                                                |
| `python .shots/reason_audit.py`      | Recorded omissions still match real documented rows and their reasons remain honest                                              |
| `python .shots/write_only.py`        | Builder fields are read rather than stored as no-ops; shared field names still need manual review                                |
| `python .shots/design_audit.py`      | Resting metrics match tagged v3 CSS and token scales                                                                             |
| `python .shots/token_audit.py`       | Theme variables, light/dark values, layout tokens, delays, and shadows are exposed accurately                                    |
| `python .shots/state_audit.py`       | CSS and prose interactive states map to rendering code                                                                           |
| `python .shots/anim_audit.py`        | Motion symbols exist and per-overlay enter/exit timing, easing, and scale match                                                  |
| `python .shots/behaviour_audit.py`   | Documented keyboard/pointer behavior claims map to implementation evidence                                                       |
| `python .shots/anatomy_audit.py`     | Foreign component slots and documented composition parts exist in the right component                                            |
| `python .shots/part_audit.py`        | Every component stylesheet part is implemented, translated, or narrowly excluded                                                 |
| `python .shots/example_audit.py`     | Gallery section names cover v3's documented examples                                                                             |
| `python .shots/demo_audit.py`        | Gallery code exercises the props used by v3's examples                                                                           |
| `python .shots/inert_audit.py`       | Gallery controlled examples are driven and keyed state is instance-scoped                                                        |
| `python .shots/reference_audit.py`   | Checked-in reference metadata resolves to real routes, owners, methods, and pinned source links                                  |
| `python .shots/package_audit.py`     | Crate packaging and gallery CLI metadata are coherent                                                                            |
| `python .shots/a11y_audit.py`        | Every component whose React Aria hook reports a role states one, with the components a later wave will reach recorded as pending |
| `python .shots/theme_serde_audit.py` | `ThemeDocument` JSON keys match `ThemeBuilder` methods, and `serde` stays a non-default feature                                  |

### Runtime accessibility tree (macOS)

`a11y_audit.py` reads roles declared in the source. What a screen reader
actually gets is checked at runtime by `.shots/ax_tree.py`. GPUI's macOS
platform exposes the window through AccessKit, which answers the
NSAccessibility attributes VoiceOver reads (role, name, value, enabled,
focus, `AXPress`). The tree is built only once an assistive client
queries the app, and the script's Swift helper (`.shots/ax_dump.swift`,
compiled on demand into `target/ax/`) is such a client. For each component
page the script launches the native gallery unfocused, dumps the tree and
checks:

- **R1:** every operable control has a name;
- **R2:** checkbox and range values are well formed;
- **R3:** every interactive page exposes at least one control;
- **R4, R5 (report-only):** setting focus through AX, and pressing through
  `AXPress`.

```sh
python3 .shots/ax_tree.py --record target/ax-run.json --dump-dir target/ax-dumps
python3 .shots/ax_tree.py --page Checkbox --page "Combo Box"
python3 .shots/test_ax_tree.py
```

It needs macOS and the Accessibility (TCC) permission for the process that
runs it, so it is not a CI job: it exits 2 without either. It reads what
VoiceOver reads but does not verify speech output, reading order or rotor
navigation. A name the audit accepts may still be unhelpful, so read the
names it reports. `EXPECTED_UNNAMED` holds accepted exceptions, each with
its reason, and an entry that no longer matches fails.

No individual audit proves full parity. In particular, prop coverage does not
prove behavior, design metrics do not prove anatomy, screenshots do not prove
interaction, and static evidence mappings do not replace focused behavior tests.
Use `python .shots/design_audit.py --coverage` when changing the design reader;
an all-green mapped subset is not proof that every upstream metric is covered.

## Running the audit set

The documentation, CSS, and ComponentPreview inputs are checked in.
`.shots/heroui-bundle.txt.gz` is the docs bundle the prop and prose audits read;
`.shots/heroui-css-v3.2.7.tar.gz` contains the component stylesheets the design,
motion and anatomy audits read; `.shots/heroui-demos-v3.2.7.tar.gz` holds the
preview sources `demo_audit.py` falls back to when the rendered bundle omits
them. All three unpack themselves on first use. Cache keys include the
normalized tagged URL. `--fetch` is the only network path and deliberately
refreshes the cache; `--pack` rewrites the checked-in archive afterwards. TLS
certificate verification stays enabled.

For a strict aggregate verdict and a fresh evidence snapshot, run:

```sh
python .shots/parity_report.py --output docs/parity/audit-results.json
```

This runner checks each reader's declared counters and diagnostics as well as
its exit code. Missing input, unreadable summaries and reported gaps fail the
report even when a legacy reader exits zero. It prewarms demo fallbacks, hashes
the effective inputs and rejects changes during the run. Bundle override
environment variables are rejected for this pinned report. A passing report
still proves only mapped static coverage, not visual or interaction parity.

The individual-reader loop remains useful for inspecting full diagnostic output:

```powershell
Get-ChildItem .shots/*audit.py | ForEach-Object {
    python $_.FullName
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
python .shots/write_only.py
```

CI runs the strict aggregate, checks inventory freshness and exercises the
reader regressions below. It preserves a generated `parity-report` artifact,
including failed reader output. A failure before report generation remains a
failed job even though there is no report artifact to upload.

### Inventory freshness and reader regressions

After synchronizing source, generated data and the WASM artifact, refresh and
check the existing work queue:

```sh
python .shots/interaction_inventory.py --refresh
python .shots/interaction_inventory.py --check
python .shots/coverage_report.py --refresh
python .shots/coverage_report.py --check
python .shots/test_interaction_inventory.py
python .shots/test_parity_report.py
python .shots/test_coverage_report.py
python .shots/test_inert.py
```

To refresh upstream source deltas and demos too, supply `--upstream` and
`--baseline` together, pointing at verified extractions of the inventory's
recorded target and baseline commits. This command does not change the release
pin or fetch a moving branch.

The inventory preserves unchanged reviews and retired rows, invalidates stale
specimen verdicts, and writes atomically. A `verified` specimen needs existing,
hashed upstream/native/WASM/test evidence tied to the current input fingerprint.
An empty specimen list or a current gallery-section inventory is not completed
state coverage. See [the implementation plan](../parity/ui-design-plan.md) for
the required expansion and [execution progress](../parity/ui-design-progress.md)
for the current frontier.

Scripted runs keep automated results as checked files in `docs/parity/runs/`,
one file per kind. Re-recording overwrites the file, and git keeps the
history:

| Run                      | Recorder                                                                       | Granularity     |
| ------------------------ | ------------------------------------------------------------------------------ | --------------- |
| `interaction-sweep.json` | `python3 .shots/record_sweep.py` (the headless hover/press sweep)              | gallery section |
| `wasm-smoke.json`        | `node web/scripts/gallery-smoke.mjs --record docs/parity/runs/wasm-smoke.json` | gallery section |
| `native-a11y.json`       | `python3 .shots/ax_tree.py --record docs/parity/runs/native-a11y.json`         | page            |

The inventory indexes each run under `scripted_runs` with its hash, kind,
pass/fail ids and whether it is current. A run is current while the
`source_sha256` it stamped matches the source snapshot, or, for the web
smoke, while its artifact key matches `web/src/data/wasm-parity.json`. An
unknown section id, a tampered file and an unlisted file all fail `--check`.
The index is outside `verification_fingerprint`, so re-recording a run never
invalidates a review. A run never makes a specimen `verified`: it is
regression evidence for the inputs it recorded, not an upstream comparison.
Record runs after the last source edit, then refresh the inventory and the
coverage report.

The coverage generator writes `docs/parity/coverage-report.json` and its
maintainer-readable Markdown companion from the inventory. It reports every
status, evidence surface and unresolved id, including measured gaps, accepted
deviations and platform limits. Its `--check` mode is the CI freshness gate;
the report must not be edited by hand or interpreted as a parity verdict.

Refreshing either pin is deliberate, never a side effect of a run. The bundle
audits refuse any copy whose latest release is not `PINNED_RELEASE` in
`.shots/bundle.py`; point `HEROUI_BUNDLE` at another file and set
`HEROUI_BUNDLE_UNPINNED=1` to read a different release on purpose. To move the
pin, refresh the archive and `PINNED_RELEASE` together and re-run the set:

```powershell
curl -sL https://heroui.com/react/llms-full.txt | gzip -9 > .shots/heroui-bundle.txt.gz
python .shots/design_audit.py --fetch   # then re-pack heroui-css-v3.2.7.tar.gz
python .shots/demo_audit.py --fetch --pack
```

Run focused scripts while iterating. Run the set when a broad parity claim,
audit parser, shared metadata table, or release surface changes.

## API ownership and omission rules

- Read the entire `## API Reference` section, not a fixed character window or
  only a heading named after the page. V3 uses root tables, part tables,
  composition tables, hooks, functions, and layout tables.
- Attribute builders per `impl <Struct>`, never per file. Several modules define
  multiple components.
- Preserve the owner on folded part rows. A parent builder cannot satisfy a
  child part's same-named prop, and a child cannot satisfy the parent.
- Every name in companion/part/fold tables must resolve to a real `impl`.
- A table is a prop table only when its headers say so. Value/key tables are not
  API rows. Handle tables where a `Component` column precedes `Prop`.
- A builder-name match is insufficient when the documented type is wider. Map
  scalar/array unions and callback shapes to the builder that proves the full
  signature.
- An alias is only a spelling difference. Never map `defaultValue` to `value` or
  otherwise use an adjacent prop to hide a missing state mode.
- Constructor arguments are implemented props when their semantics match.
- Render-prop arguments are normally portable by inversion: accept a closure
  and pass it the state the component already computes.
- Shared interactive render-prop state uses `util::InteractiveState` plus
  `util::interaction`/`util::track_interaction`. Attach the tracking handlers
  only when a render closure is configured because hover/press state costs a
  frame and another event binding.
- A recorded omission must name the real limitation: no accessibility tree, no
  CLDR locale data, browser-only hints, HTTP form transport, a single-valued
  enum, or a genuinely missing mode. "GPUI cannot" requires checking the pinned GPUI
  source first.
- Remove no-op builders. Do not keep a public promise just to make an audit row
  appear implemented.

Use `python .shots/gap_context.py "Table=allowsSorting,isRowHeader"` to print
source descriptions for suspicious gaps, and `reason_audit.py` to re-evaluate
existing exclusions.

Keep audit configuration narrow and named: `WONT_PORT` records real omissions,
`ALIAS` spelling differences, `EXTRA_OK` allowed repository-only builders,
`COMPANIONS`/`PART_STRUCTS`/`FOLD_STRUCTS` ownership, `ABSENT_IS_ZERO` legitimate
zero metrics, and `COVERED_ELSEWHERE` metrics proved by another row. Demo/state
exceptions belong in their specific tables (`WONT_DEMO`, `NEEDS_FEATURE`,
`NOT_STATE`), not in a broad prop omission.

## Audit-reader integrity

An audit must fail loudly when it cannot locate an expected page, section,
table, source block, owner, or symbol. Empty input is not a zero-gap result.

Logical module names such as `color_picker.rs`, `date_picker.rs`, and
`reference_metadata.rs` still key the tables after those modules became
directories. Read them through `.shots/component_source.py` (or
`web/scripts/lib/gallery-source.mjs`); do not `open()` the historical
filename as a single file.

When editing an audit:

- Anchor parsers on structural boundaries such as headings, `impl` blocks, or
  declarations, not arbitrary character windows.
- Scope CSS reads to the owning selector and apply later breakpoint/utility
  overrides in cascade order.
- Resolve variables and arbitrary values before comparing widths or colors.
- Distinguish "absent means zero" from "pattern unreadable" explicitly.
- Report coverage in addition to pass counts. An unmapped source claim is an
  error or a reasoned exclusion, never a silent skip.
- Test a known-negative case so the changed reader demonstrates that it can
  fail. A green result alone may mean it read nothing.
- Avoid putting escape-heavy regexes through a shell heredoc; hidden control
  characters can make a readable-looking pattern match nothing.

## Design, state, and anatomy

- Use the largest applicable responsive breakpoint for this desktop port.
- Respect CSS cascade conflicts such as `size-*` after `h-*`/`w-*` and custom
  property overrides of generic border utilities.
- Focus allowances such as matching positive padding and negative margin are
  not content padding.
- V3 has component-specific radii and motion. Do not collapse them into one
  generic control radius or one generic overlay animation.
- V3 floating panels use shadow separation; dark-mode hairlines are not proof
  that light mode should have a border.
- State evidence must check meaningful arguments. A function call with a
  literal `false` focus flag does not prove focus styling is wired.
- Anatomy comes from containment and composition, not visual plausibility.
  Select, Autocomplete, and ComboBox have different trigger/query structures.
- A composition part with only `children`/`className` still matters even if it
  contributes no prop row.
- Recorded platform limitation: AvatarGroup's default `overlap="clip"` is a
  CSS radial-gradient `mask-image` crescent. Pinned `gpui-pre` has only
  rectangular `ContentMask` clipping and no alpha masks, so the port paints the
  2px seam in `--background` instead of cutting it. It matches on a solid
  background and is a solid seam over images or gradients; do not claim
  transparent-seam parity. See `avatar_group.rs` and `part_audit.py`.
- Render order is part of anatomy. Labels, descriptions, options, and field
  errors must appear in the order upstream composes them.

## Demos are executable evidence

Section-name coverage and code coverage are separate. A correctly named demo
can still ignore the feature it claims to show.

- Use uncontrolled `default_*` props for an interactive specimen unless the
  demo stores and feeds back the controlled value.
- Wire every controlled gallery prop to its matching callback.
- Give every repeated instance a unique id.
- Exercise a new builder/render closure in the gallery when the public API is
  meant to be discoverable.
- Compare the page's code, not just its headings, and drive behavior that cannot
  be inferred from a screenshot.

When parity work changes interaction, follow the component test guide and the
gallery guide as well as the static audits.
