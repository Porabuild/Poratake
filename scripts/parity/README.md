# Native visual and desktop tests

## Windows visual regressions

Run `bun run test:visual` to build the GPUI harness and check the visual baseline matrix. Use `--theme=light`, `--width=950`, or `--scale=1,1.25` to select profiles. The matrix covers light and dark themes, 950 and 1280 logical pixels, and simulated scales of 100%, 125%, 150%, and 200%.

Baselines live under `baselines/win32/<theme>-<width>-<scale>/<renderer>/`. The harness checks geometry and pixels independently. It prints the evidence directory, which contains captured images, diffs, and `renderer.json` with the GPU and driver. The renderer must have its own matching baseline.

To record a profile, run `bun run test:visual --update-baselines` with the desired profile filters. Review the captured states before committing baseline changes. The Checks workflow can run the full matrix or record and upload baseline shards.

## Windows desktop tests

The Checks workflow also runs `windows-scroll-capture-test.ps1` and `windows-recording-tooltip-test.ps1` on a real Windows desktop. The first checks scroll capture stitching and the second checks recording tooltips, capture exclusions, and recording control behavior. Both scripts build the required native targets and write evidence under the runner's temporary directory.

## macOS native capture smoke

Build the GPUI app with the `parity` feature and launch it with `--native-smoke-recording`. Set absolute paths for `PORATAKE_CONFIG_DIR`, `PORATAKE_NATIVE_SMOKE_OUTPUT`, and `PORATAKE_DAEMON_PATH`. Set `PORATAKE_NATIVE_SMOKE_RECT` to the test rectangle in x,y,width,height points. Use an authorized daemon and synthetic content in the test capture area.

The process takes capture commands on stdin, including `capture area`, `capture window`, and `capture timer`. The `pointer down x y`, `pointer move x y`, and `pointer up x y` commands use coordinates local to the first overlay window. The `snapshot` command captures the configured rectangle with the visible selector excluded and saves `active-selector.png` to the output directory. Set `PORATAKE_NATIVE_SMOKE_TARGET` to `screen` or `window` to change the recording target from the default area. For a window target, also set `PORATAKE_NATIVE_SMOKE_WINDOW`.
