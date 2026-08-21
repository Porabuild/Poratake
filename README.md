<p align="center">
  <img src="https://raw.githubusercontent.com/Porabuild/Poratake/main/build/icon.png" width="128" height="128" alt="Poratake" />
</p>

<h1 align="center">Poratake</h1>

<p align="center">
  <strong>Capture anything. Keep the moment.</strong><br />
  A native screenshot and screen recording studio for macOS and Windows. Capture, annotate, record, edit, understand, and share without breaking your flow.
</p>

<p align="center">
  <a href="https://porabuild.com/poratake">Website</a> · <a href="https://github.com/Porabuild/Poratake/releases">Releases</a> · <a href="https://github.com/Porabuild/Poratake/issues">Report a Bug</a> · <a href="https://github.com/Porabuild/Poratake/issues">Request a Feature</a>
</p>

<p align="center">
  <em>Free and open source under the GNU AGPL v3.0</em>
</p>

---

<p align="center">
  <img src="https://porabuild.com/poratake/opengraph-image" alt="Poratake capture studio" width="960" />
</p>

## Why Poratake?

Capturing something should not turn into a chain of disconnected tools. Poratake keeps screenshots, recordings, annotation, OCR, transcription, editing, and export in one focused workflow.

### Capture Exactly What You Need

Capture a full display, a window, or a precise area. Freeze the desktop when the moment cannot move, set a timer, capture a scrolling page, or use the all-in-one panel when you want every tool close at hand.

### A Complete Screenshot Editor

Draw with pen and highlighter tools, add shapes, arrows, text, and numbered steps, redact sensitive details, crop the result, and present it against a polished wallpaper or window frame.

### Recording That Stays on Target

Record a display, area, or individual window with microphone, system audio, camera, and cursor tracking. A selected window remains the recording target when it moves, resizes, or passes behind another window.

### A Video Editor Built In

Trim recordings and shape the final story with cursor controls, Auto Zoom, drawings, camera, audio, wallpaper, keyboard events, subtitles, and a custom first frame. Preview and export follow the same composition.

### Turn Pixels Into Information

Extract text with OCR, scan QR codes, and transcribe recordings without leaving the capture workflow.

### History, Pinning, and Sharing

Return to earlier captures, pin an image above other windows, copy or save the result, print it, or upload it through your own S3 or REST provider.

### Built for Real Desktops

Poratake uses native capture daemons on macOS and Windows. Windows capture is HDR-aware, preserving natural SDR screenshots and recordings instead of producing washed-out results on HDR displays.

## What Ships Today

| Workflow          | Capabilities                                                                                      |
| ----------------- | ------------------------------------------------------------------------------------------------- |
| Capture           | Full screen, area, window, freeze screen, timer, scroll capture, selectors, all-in-one, and print |
| Understand        | OCR, QR scanning, and transcription                                                               |
| Record            | Screen, area, window, camera, microphone, system audio, cursor tracking, and live controls        |
| Screenshot editor | Pen, highlight, shapes, arrows, text, numbering, redaction, crop, wallpaper, and window frames    |
| Video editor      | Cursor, Auto Zoom, drawing, camera, audio, wallpaper, keyboard, subtitles, first frame, MP4, GIF  |
| Desktop and share | History, pin, desktop icons and wallpaper, cloud upload, copy, and save                           |

## Platforms

| Platform | Support                                                                             |
| -------- | ----------------------------------------------------------------------------------- |
| macOS    | macOS 15 or later on Apple silicon and Intel; full feature set                      |
| Windows  | Windows 10/11 (x64 and arm64); full feature set except the macOS-only capture sound |

## Install

Published installers are distributed through the [GitHub releases page](https://github.com/Porabuild/Poratake/releases), with corresponding source published alongside every release. To build Poratake locally, follow the setup and native build instructions in [CONTRIBUTING.md](CONTRIBUTING.md).

## Contributing

Contributions are welcome. Please open an [issue](https://github.com/Porabuild/Poratake/issues) before starting a substantial change so the approach and scope can be discussed first.

## License and Attribution

Poratake is a modified version of [Capty](https://github.com/capty-app/capty). Poratake modifications were made in 2026.

Copyright (C) 2026 Capty.

Copyright (C) 2026 Serhii Vecherenko for Poratake modifications. Poratake is a Porabuild project. Copyright in other contributions remains with their respective contributors.

Poratake is licensed as a whole under the [GNU AGPL v3.0](LICENSE), without warranty of any kind. You may redistribute it under the same license. Corresponding source for Poratake releases is available from this repository; release downloads and their source are published together on the [releases page](https://github.com/Porabuild/Poratake/releases).

Poratake bundles components under compatible licenses, including FFmpeg under the LGPL v2.1, whisper.cpp under the MIT License, and Geist under the SIL Open Font License 1.1. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for copyright notices, exact versions, source details, and license terms.
