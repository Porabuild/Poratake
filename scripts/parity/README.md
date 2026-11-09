# UI comparison

Run the Electron renderer with its fixture preload bridge:

```sh
bunx vite --config scripts/parity/vite.renderer.mts
```

Use `?window=screenshot&platform=darwin&image=/@fs/<absolute PNG path>` or
`?window=video-editor&platform=darwin&file=<HTTP video URL>`. Fixture files must
be under the repository or the system temporary directory allowed by Vite.

Capture GPUI with its real platform renderer and fonts:

```sh
PORATAKE_PARITY_OUTPUT=/tmp/poratake-parity \
PORATAKE_PARITY_IMAGE=/absolute/path/capture.png \
cargo run --locked --manifest-path src/main/Cargo.toml \
  -p poratake-gpui --features parity -- --parity
```

The optional `parity` feature is excluded from normal builds. The command uses
a fresh isolated profile and exits before normal app startup. It renders at
1280×738 logical pixels and advances GPUI's clock for hover and press animations.
Captures include the editor header, held buttons, selected tools, open dropdowns,
the color picker, and video header/sidebar states. The runner asserts selected
tools and menu state so a failed interaction cannot silently produce mislabeled
evidence.

Match viewport, scale, theme, fixture and interaction state before comparing
images. Compare the changed surface separately from video playback, OS window
chrome and automation cursor markers.
