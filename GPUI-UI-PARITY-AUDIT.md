# GPUI versus Electron verification audit

Refreshed 2026-11-15 for Poratake 0.9.6.
Electron remains the product reference. This is a verification record and current
backlog; the earlier implementation plans are historical findings, not an open-bug
list. A source comparison or passing unit test does not establish full manual parity.
The intended destination is a GPUI-only app after the required workflows and
platform behavior have been verified.

## Current critical scope

This pass covers major clipping, sizing, missing controls, stale selections,
capture corruption and preview/export mismatches in the core Windows and macOS
capture and recording workflows. Electron remains the reference where the same
workflow exists. The detailed backlog below remains useful, but is not a list of
required work for this pass. Minor antialiasing differences and Linux-only
enhancements are deferred unless they expose a critical shared defect or have a
small verified fix. Exhaustive states for every control remain deferred.

Each gate counts once only when its stated evidence exists. As of 2026-11-15,
seven of twelve gates are complete: 58% of the gates, not an estimate of elapsed or
remaining effort. Partial work does not count as a completed gate.

The full Windows visual matrix is opt-in through the Checks workflow's manual
`run_windows_visual_regressions` input. Run it against the intended revision for
release verification; routine PR checks do not run the matrix.

| Gate                                 | Status | Completion evidence                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HeroGPUI dependency                  | Done   | #43 released as 0.15.1; Poratake pins that release and its matching GPUI version.                                                                                                                                                                                                                                                                                                           |
| Reviewed visual coverage             | Done   | Windows CI at `5f50562a` passed 16 profiles and 2,912 reviewed baselines on Microsoft Basic Render Driver.                                                                                                                                                                                                                                                                                  |
| Automated screenshot workflow        | Done   | Capture, annotate, save and reopen run in the visual workflow fixtures.                                                                                                                                                                                                                                                                                                                     |
| Automated recording workflow         | Done   | Recording, editing and export fixtures compare tested preview/output cases.                                                                                                                                                                                                                                                                                                                 |
| Native Windows tooltip               | Done   | Hosted checks verified recording and capture-preview tooltips, including screenshot exclusion; recording frames were decoded.                                                                                                                                                                                                                                                               |
| Native Windows preview handoff       | Done   | A real screen capture showed a bottom-right preview and opened its saved image in the editor.                                                                                                                                                                                                                                                                                               |
| Hosted code-and-baseline CI          | Open   | Routine checks passed at `f54b4c25`; run the optional visual matrix against the intended release revision before completion.                                                                                                                                                                                                                                                                |
| Windows capture and save             | Done   | Hosted Windows captured an area, annotated, saved and reopened a PNG with pixel and editor-size checks. A later run created a smaller selection inside the previous region.                                                                                                                                                                                                                 |
| Windows recording and export         | Open   | Initial and after-seek preview samples match a real trimmed export. Verify broader native output and system audio when available without prompting.                                                                                                                                                                                                                                         |
| Windows major visual and DPI checks  | Open   | Check tray, size submenu, preview and toolbar hover/focus/open states at 100%, 125% and 150% OS scaling, including a narrow window and connected monitors.                                                                                                                                                                                                                                  |
| macOS critical capture and recording | Open   | Area, window, and timer screenshots passed on Retina. Screen, area, and window recordings were edited and compared with decoded exports. System audio and microphone passed. Unpressed toolbar and preview hover passed on the built-in display with WindowServer logs. Full desktop encode, other pointer paths, live camera, additional displays, and ambiguous scroll seams remain open. |
| Final review and audit               | Open   | Review the final changes, verify CI on the actual PR head, resolve critical regressions and update this checklist with exact evidence.                                                                                                                                                                                                                                                      |

### Local macOS verification attempt at `c34624e1`

On 2026-11-15, this checkout was clean at `c34624e1` on an Apple M5 Pro Mac
running macOS 27.2. System Information reported one online built-in Retina display
(3456×2234). `bun run dev:gpui` exited before launch because Bacon 3 was not
installed. This checkout had no built GPUI executable, development daemon or
development FFmpeg. No dependencies were installed. The separately installed
Electron app was already running and was left intact. The computer-use helper
reported Screen Recording permission, but that does not establish permission for
the GPUI executable or its daemon.

The macOS `SCROLL_CAPTURE_TEST` fixture was compiled from this checkout with the
same sources and flags as the Checks workflow and ran successfully without the
`--screen-capture-exclusion` option. Its synthetic scroll preview/save checks,
recording stop coordination and capture-filter coordination passed. The fixture
explicitly skipped the ScreenCaptureKit exclusion check. This is not evidence of
native captured pixels or an interactive GPUI workflow.

Area, window, timer and real scroll captures; selector cleanup; preview and
tooltip placement; annotation, save and reopen; screen, area and window recording,
target movement and closure; decoded export comparisons; audio and camera; light
and dark appearance; and narrow-window behavior remain unverified on this Mac.
No new defect was established by this attempt. The macOS gate stays open, as does
the fixed-header scroll-stitch issue described below.

The follow-up built the Swift daemon and GPUI shell from this checkout using
locally cached dependencies. The isolated GPUI parity build initially failed on
macOS because a shared workflow referenced a Windows-only helper. The helper is
now available to parity builds on macOS; fixture samples and control indices also
account for the prompt shadow, the iPhone/iPad control, macOS point coordinates,
and the actual display bounds. These are fixture changes, not evidence from a real
capture. The normal app was not launched: startup prewarms screen capture, and the
signed Electron app's grant does not establish access for the development daemon.

With the simulated daemon and an isolated profile, the 1280px, 2× parity run passed
in both light and dark themes, as did the 950px, 2× narrow-layout run in both
themes. At 1280px and 1× in each theme, simulated window, area, display, timer and
scroll capture, selector cleanup, annotation, save and reopen checks passed before
the run stopped in an editor preview assertion. In two dark runs and one light run
at 1.5× editor zoom, drawing a draft changed two sampled pixels outside its expected
bounds from `[34, 103, 155, 255]` to `[40, 120, 180, 255]`.
The before and after images from the second run are under
`/tmp/poratake-parity-c34624e1-dark-scale1-proof/` in its `profile-*` directory.
The observed line disappeared near the draft. A follow-up comparison with the
reopened exported image showed that the unannotated preview edge had sampled a
neighboring atlas tile. The pinned Apple renderer now clamps nonempty image tiles
to their texel centers, as the Windows renderer already does. The full dark 1×
and light 1× simulated runs subsequently passed, including three opaque edge
comparisons with the reopened export and the original draft assertion; no tolerance
was changed.

The isolated macOS parity workflow also passed at 950 logical pixels and simulated
125% scaling in both light and dark themes. Each run rendered 193 GPUI states and
checked narrow editor navigation, selector states, recording controls, preview
placement and history tooltips. Representative recording controls are retained in
[light](tests/native/evidence/macos-ui/record-control-light-950-125-20261116.png)
and [dark](tests/native/evidence/macos-ui/record-control-dark-950-125-20261116.png),
along with the scrolled editor toolbar and bottom-right preview states for each
theme. The fake daemon deliberately injects a camera protection failure in one
workflow; the [logged failure](tests/native/evidence/macos-ui/parity-narrow-125-20261116.txt)
is expected. These are headless renderer and simulated
interaction checks, not native window-compositor, live capture or device evidence.
The representative recording controls show no extra wrapper in the renderer;
native frame verification and physical interaction remain separate checks.
GPUI could also round a cropped one-pixel dimension to a zero-sized atlas tile.
In an isolated Metal probe, a neighboring white tile changed an expected
`[240, 80, 64, 255]` pixel to `[247, 167, 159, 255]`. The pinned GPUI core now
retains one-pixel source dimensions when cropping and ignores empty images; the
probe passes for both centered and far-edge crops and checks that an empty image
paints nothing. A narrow two-pixel crop can
still round one dimension to zero, so the Metal shader now clamps the other
dimension independently and keeps premultiplied-alpha interpolation. The probe
checks the opaque blend near both edges and a translucent blend; before this
shader change an edge pixel was `[35, 0, 35, 255]` instead of approximately
`[128, 0, 128, 255]`. A later Metal probe exposed a remaining source-edge leak:
a narrow crop produced `[0, 0, 128, 255]` where the image edge was
`[0, 0, 255, 255]`. Apple sprites now retain the original allocation bounds and
fractional source rectangle; the shader interpolates within that rectangle and
clamps to the original image. The edge probe and horizontal and vertical
fractional gradient checks pass with differences of at most two channel levels.
The full dark 1× and 2× and light 950px 2× parity runs pass after the change,
and the x86_64 macOS shell compiles. Probe images are under
`/tmp/poratake-metal-crop-before/` and `/tmp/poratake-metal-crop-2d/`.
The Windows shader's zero-width clamp remains unverified. These are headless
checks, not native UI proof.

A wider diagnostic comparison found 476 differing pixels around translucent
content at 1.5× editor zoom after the edge fix. Metal was interpolating straight
RGB and alpha separately before compositing. Interpolating premultiplied values
removed that difference in the focused fixture; the macOS redaction preview's
software sampler now uses the same interpolation. The full dark 1× simulated
run passed both that comparison and the existing transparent pixelation checks
at 0.5×, 0.73×, 1.37× and 1.5× editor zoom. The saved reference images from before
the fix are under `/tmp/poratake-draft-reference-dark-3/`. Native editor preview
and export parity remain open.

An isolated 3840×2160 headless editor probe on this Apple M5 Pro compared 42
post-warmup GPU frames for each sampler, across three alternating runs. Median
GPU time was 2.70 ms before and 2.60 ms after; individual frames varied, so this
does not establish a speedup or characterize Intel Macs, other GPUs or live
window presentation. The new sampler reads four texels for each nonempty image
tile instead of one hardware-filtered sample.

A separate Swift process launched by this session returned true from
`CGPreflightScreenCaptureAccess()` without requesting access. That result belongs
to the probe process and does not establish the GPUI daemon's authorization; live
GPUI capture has not been attempted. The probe reported microphone access granted
and camera access undetermined for its own identity. Neither was exercised.

After installing Bacon 3.25.0 at the user's request, the native Swift fixture was
run with `--screen-capture-exclusion`. Its own authorization preflight passed, and
it found its hidden control window in ScreenCaptureKit content. Starting the
filtered stream then terminated the fixture with SIGTRAP on macOS 27.2. The
diagnostic report identified `Must only be used from the main thread` on a
background ReplayKit `setWindowRotationLocked` path into AppKit's
`NSWindow.setStyleMask`. This reproduces the earlier hosted fixture trap with a
symbolized native stack; it does not establish that the GPUI recorder fails. No
captured frame was produced, so exclusion and restart remain unverified. The
existing Electron process remained running.

The user subsequently authorized the checkout's FFmpeg build. The pinned 9.0.1
source passed its checksum, produced an arm64/x86_64 universal binary and passed
the script's LGPL check. A generated one-second H.264 VideoToolbox MP4 encoded and
decoded successfully. Configure warned that PNG encoding and decoding were disabled
because the deflate and inflate wrappers were unavailable, despite the script
requesting both codecs; neither appears in the resulting codec lists. Any effect on
Poratake workflows remains unverified. The dev launcher now recognizes the stamped
FFmpeg build, but the live GPUI and recording checks above remain open.

The development FFmpeg script now explicitly enables macOS's system zlib while
autodetection is disabled. A rebuilt universal 9.0.1 binary lists both PNG
encoding and decoding; on the Apple Silicon host, a generated 2×2 RGBA image
round-tripped through both codecs with all 16 bytes identical, including alpha.
Both architectures link the system `/usr/lib/libz.1.dylib`; the Intel slice was
built but not executed. The build's LGPL configuration check passed. This checks
the codec directly, not a live capture or the complete video editor workflow. A
generated four-frame H.264 VideoToolbox MP4 also encoded and decoded after the
rebuild. Against that rebuilt binary, the ignored macOS export tests for
first-frame intro and delayed audio, synthetic camera ranges with zoom across a
cut, and synthetic microphone/system-audio sidecar mix and mute all passed. The
tests compare generated preview and decoded export data without accessing devices.

The subsequent GPUI launch exposed a Bacon configuration error: the Unix kill
override replaced the complete `gpui` job with an empty command. The launcher now
applies the platform kill command through Bacon's `all_jobs` configuration, leaving
the job command intact. A focused test runs Bacon's job listing when Bacon is
installed, and the corrected `bun run dev:gpui` launched with an isolated profile.

For a permission-safe native probe, a temporary copy of the Swift daemon added a
screen-access preflight before its startup capture. Its own preflight returned true
without a request. A separate instance of that instrumented daemon captured a
400×300-point live screen area into an 800×600 PNG, and captured a GPUI preview
window into a nonblank PNG. These are direct daemon calls, not completed GPUI
selection workflows; neither proves control or overlay exclusion. An earlier
standalone prewarm attempt timed out, but a further instrumented run completed, so
the timeout is not a confirmed defect. The GPUI preview overlay existed in macOS's
window list but was not exposed as a target by the background-control tool. Native
selector interaction, timer and scroll capture, and recording/export remain open.

After the user authorized temporary foreground control, the isolated GPUI app was
launched with `--intent capture-area`. macOS reported a visible 1728×1117 capture
overlay at window layer 101, and the instrumented daemon again reported screen
access granted. The computer-use service did not return that window or resolve it
by its macOS window ID, so no selection or save was performed. The test process was
stopped and its windows disappeared. A separate `--intent record-screen` launch
showed a 332×52 pre-recording control window at layer 101; the service could not
target that window either, so recording was not started. Both runs used the same
temporary daemon preflight and isolated profile described above; the original
Electron process remained running.

Although the isolated profile had system audio disabled, the Swift recorder sets
`SCStreamConfiguration.capturesAudio` to true on macOS 13 and later to support
in-session toggling. The daemon's system-audio authorization was not established,
so no direct recording probe was started under the no-permission-dialog constraint.
For a later isolated recording probe, a temporary daemon copy checked its own
screen-capture authorization before starting and disabled ScreenCaptureKit audio,
microphone, camera, keyboard and cursor tracking. A separate synthetic window
process supplied the target, so the probe did not capture the existing editor.
The window moved during recording and was then closed. Initially ScreenCaptureKit
reported `SCStreamErrorDomain` code -3815; the daemon emitted `CAPTURE_ERROR` and
stopped itself, which caused GPUI's error path to discard the project. The daemon
now identifies that error for window recordings as `TARGET_CLOSED` and waits for
the client to stop and finalize the take. Other errors and display recordings keep
their existing error path.

The same probe found that ScreenCaptureKit sends a stopped-status sample with
timestamp zero, resetting the reported duration even though the movie contained
video. Ignoring that control sample retained the duration: after moving and closing
the target, explicit stop succeeded with 2.326 seconds reported and `ffprobe`
measured 2.327 seconds and 12 frames. The probe's logs and synthetic movie are in
`/var/folders/_r/p8gqgcd93zb1w708g0h__ztw0000gn/T/poratake-native-smoke-5yrih1e3/window-stream-fixture/`.
Decoded frames from a second take show the synthetic target's updated content
both before and after movement. Native Swift tests cover the error classification
and stopped sample status. This verifies the
screen-only daemon path with a synthetic window, not GPUI's recording controls,
audio, camera, editor or export.

The macOS recorder previously let a system-audio callback establish the initial
recording timestamp before the first screen frame arrived. Screen and system-audio
callbacks now use one serial queue, and only a screen sample with an image can
start the timeline. After a pause, either stream can resume it: the macOS SDK
defines idle screen frames as having no new image, so requiring a new image could
discard system audio for as long as a static screen remains unchanged. The permission-free native fixture
checks screen, audio, empty and stopped sample eligibility. A second native fixture
feeds synthetic samples through the recorder's actual callback logic and verifies
audio arriving before the first video frame and after a paused, unchanged screen.
The full daemon also typechecks. The commands and results are recorded in
[native evidence](tests/native/evidence/macos-ui/recording-timeline-20261116.txt).
This does not verify physical audio, microphone synchronization or
an actual paused recording, which remain open under the access constraint.

The same fixture exposed a separate audio corruption bug in both muting and
timestamp adjustment: a 480-sample, 48 kHz PCM buffer lasting 10 ms became
4.8 seconds because the total buffer duration was assigned to every sample.
Both paths now retain the original sample timing entries, shifting timestamps
only when retiming. The native fixture checks the actual recorder methods with
both a single shared timing entry and individual timing entries, including
duration, an interior timestamp, zeroed output and an unchanged source buffer.
This is synthetic PCM evidence, not a live microphone or system-audio recording.
Microphone re-enable padding also started at the previous buffer's start and
could extend a full silent buffer past the resumed audio. A native synthetic
fixture reproduced both overlaps in an AAC writer. It now starts padding at the
last successfully written buffer's end and trims the final silent buffer to
whole samples that fit. The writer received adjacent intervals at 0–10 ms,
10–20 ms, 20–26 ms and 26–36 ms; a decoded check of the
[synthetic AAC](tests/native/evidence/macos-ui/microphone-padding-synthetic-20261116.m4a)
found the expected silent middle. The fixture also checks sub-sample boundaries
and individual sample timing entries. An eight-second synthetic gap also passed
with the writer configured for real-time input. A five-minute gap failed on this
Mac: the writer stopped accepting buffers after approximately 11 seconds and the
resumed buffer was dropped. A separate writer probe that omitted silence entirely
collapsed an eight-second timestamp gap to a 64 ms file, so simply skipping the
padding is not a fix. The retained [gap probe](tests/native/evidence/macos-ui/aac-gap-collapsed-synthetic-20261116.m4a)
contains generated audio only. This reproduced the original backpressure failure.

A separate readiness-driven writer probe supplied the same five-minute gap and
final generated audio buffer through `requestMediaDataWhenReady`. It accepted all
30,001 buffers over 134 callbacks. Decoding the retained
[synthetic AAC](tests/native/evidence/macos-ui/aac-readiness-synthetic-20261116.m4a)
produced 300.01 seconds of PCM, with silence before 300 seconds and the generated
signal in the last 10 ms. This validates a possible writer mechanism only: it does
not queue real microphone callbacks, handle stop or failure, or fix the recorder.
Another synthetic probe used readiness observation, added three generated audio
buffers while the gap was being drained, and requested finish before draining
completed. It wrote all 30,003 buffers in 151 drains; the
[decoded output](tests/native/evidence/macos-ui/aac-queued-synthetic-20261116.m4a)
contained the resumed 30 ms. A separate writer-cancellation check received a
status observation. Neither probe is connected to the recorder or establishes
the behavior of a live microphone or a failed writer.

The recorder now feeds microphone audio through a bounded queue that waits for
writer readiness, keeps each silence interval as a compact range, and drains the
queue before finishing its input. With generated samples passed through the actual
recorder methods, the five-minute case now wrote 30,003 contiguous buffers,
including three resumed buffers; a cancelled writer and an injected queue limit
each returned an error. The
[decoded output](tests/native/evidence/macos-ui/recorder-queue-synthetic-20261116.m4a)
contains 300.03 seconds of PCM with the final 30 ms of generated signal. Queue
overflow reports a recording error rather than silently dropping audio. Real
microphone toggles, device changes and stop/failure timing remain unverified.

System audio had the same writer-backpressure failure: it discarded each buffer
when the writer was not ready, without reporting an error. A native generated-PCM
probe submitted 30,000 adjacent 10 ms buffers rapidly through the recorder; the
[old AAC](tests/native/evidence/macos-ui/system-audio-backpressure-before-20261116.m4a)
contained only 10.54 seconds. System audio now uses the bounded readiness-driven
queue as well, drains it before finishing and reports a writer or queue failure.
The same probe produced [300 seconds of decoded PCM](tests/native/evidence/macos-ui/system-audio-backpressure-after-20261116.m4a);
the macOS checks now run the duration assertion. The fast synthetic submission
deliberately creates backpressure and does not establish how often it occurs in a
live recording. Real system audio, toggles and device timing remain unverified.

System audio could also lose synchronization when its first sample arrived after
the first screen frame or later samples had a timestamp gap. A generated recorder
fixture sent audio at 2, 2.01 and 8 seconds; the old AAC decoded to 30 ms. The
recorder now queues silence up to each later timestamp. The [new generated AAC](tests/native/evidence/macos-ui/system-audio-gap-after-20261116.m4a)
decodes to 8.01 seconds, with signal at 2 and 8 seconds and silence in between;
the [old output](tests/native/evidence/macos-ui/system-audio-gap-before-20261116.m4a)
is retained for comparison. The native fixture decodes and checks those intervals
and runs in macOS checks.

Repeated gaps shorter than one sample previously accumulated drift: 1,000 generated
buffers with 10-microsecond gaps yielded 10 seconds instead of 10.00999 seconds.
The recorder now carries the fractional remainder and writes whole silence samples
when enough time accumulates. The native fixture finishes within one 48 kHz sample
of the expected duration, including an intervening overlap; the [generated output](tests/native/evidence/macos-ui/system-audio-fractional-after-20261116.m4a)
and [old output](tests/native/evidence/macos-ui/system-audio-fractional-before-20261116.m4a)
are retained. A remainder shorter than one sample can remain at the end. Real
ScreenCaptureKit timing, physical audio and callback performance remain unverified.

Overlapping system-audio buffers also extended the encoded timeline: three
generated 10 ms buffers covering 20 ms produced 30 ms before the correction. The
recorder now discards already-accepted samples and queues the remaining suffix.
The native fixture covers partially overlapping buffers with shared and per-sample
timing, plus a fully repeated buffer; the decoded 20 ms output retains the expected
three signal levels in order. The [generated output](tests/native/evidence/macos-ui/system-audio-overlap-after-20261116.m4a)
and [prior output](tests/native/evidence/macos-ui/system-audio-overlap-before-20261116.m4a)
are retained. Actual callback overlap and device timing remain unverified.

If an audio callback arrives after the first video callback with a timestamp
before the video start, the recorder previously clamped that timestamp to zero,
so generated pre-video samples displaced the actual opening audio. It now drops
whole pre-video buffers and trims a buffer that crosses the video start to its
first remaining sample. A fixture drives the recorder's stream callback with
distinct generated signals: before the change the first 8 ms contained pre-video
audio; afterward the decoded signal comes from the crossing buffer and the next
buffer at the expected times. The [before](tests/native/evidence/macos-ui/system-audio-initial-before-20261116.m4a)
and [after](tests/native/evidence/macos-ui/system-audio-initial-after-20261116.m4a)
outputs and a macOS check are retained. This reproduces a synthetic callback order;
the timing of live ScreenCaptureKit callbacks remains unverified.

An audio queue failure during stop previously prevented the primary writer from
finishing, and the GPUI error handler deleted the project while the daemon was
stopping. The daemon now attempts to finish each writer before returning the
failure; GPUI keeps the partial project and displays its path. A native fixture
injects a cancelled system-audio writer while a generated H.264 primary writer is
active: before the change the primary writer remained unfinished; after it, the
error is still returned and the retained [one-frame movie](tests/native/evidence/macos-ui/recording-audio-failure-recovery-20261116.mov)
decodes successfully. Fake-daemon GPUI workflows inject an audio error during the
recording and from the stop response; both check that the control closes and the
project file remains byte-identical. Other stop failures still leave the controls
available for retry. This does not
prove recovery of a real capture or of a video writer that itself has failed.
Electron also deleted the project as soon as it received a terminal recording
error, potentially while the daemon was still finalizing the video. It now keeps
the project, requests recorder finalization, waits for that request before a new
recording starts, and reports the possible partial-recording path in its error
dialog. Focused tests cover finalization failure, the retry ordering and the
permission-error message; they use mocked daemons and dialogs. A failure returned
directly from a requested stop now also reports the possible partial-recording
path after the controls close. The toolbar and tray use this same stop action;
focused tests cover the action and both entry points. The full Electron suite
passed 2,331 tests with three skipped on this Mac. Recovery after a real recording
failure remains unverified.

The microphone re-enable anchor and its upper timestamp limit previously used
the last encoded video frame even if later screen callbacks had advanced the
recording timeline. In a generated native fixture, a 20-second screen timeline
with a 1-second last encoded frame placed microphone audio at 1 second. Both
calculations now use the latest screen presentation time, adjusted for pauses,
without moving behind the last encoded frame. The fixture now places generated
audio at 20 seconds and continues it through 21 seconds; the
[decoded AAC](tests/native/evidence/macos-ui/microphone-screen-clock-synthetic-20261116.m4a)
contains silence at 1 and 19.99 seconds and signal from 20 seconds. This checks
synthetic callbacks, not how long a real static screen can go without any callback
or whether physical microphone and screen clocks stay synchronized.

Microphone re-enable also installed its start timestamp after starting the
capture session, allowing an early callback to use the previous timeline. A
generated callback delivered during setup reproduced a 0.01-second buffer end
where 20.01 seconds was expected. The recorder now installs the pending start
on the microphone callback queue before setup begins and clears it if setup fails.
The native fixture exercises both paths without opening a microphone; real device
delivery and synchronization remain unverified.

The GPUI scroll handoff previously removed its selector windows without passing
their IDs to the daemon. The earlier screenshot probes found stale selector pixels
immediately after a window was hidden or removed. The handoff now keeps the hidden
selectors owned by the scroll session, passes their stable IDs with the three
control IDs, and releases them when that session ends or fails. The daemon asks
ScreenCaptureKit for off-screen windows and rejects a frame if any requested
exclusion cannot be resolved. An off-screen [native window probe](tests/native/evidence/macos-ui/scroll-selector-lease-20261116.txt)
checks that an ordered, non-key window becomes hidden while retaining its ID,
that its actual ID joins the three actual control-window IDs, and that cleanup
leaves a newer selector intact; simulated GPUI workflows check
the exclusion list and session lifecycle. Actual scroll pixels and the behavior
of the off-screen ScreenCaptureKit filter remain unverified on this Mac.

The daemon also queued an auto-scroll stop after handling its request. A synthetic
native lifecycle test showed that this delayed stop could reach a replacement
session and stop it. Since daemon requests are already routed on the main thread,
stop now takes effect in request order. The [native fixture](tests/native/evidence/macos-ui/scroll-stale-stop-20261116.txt)
checks both a replaced session and a rapid start followed by stop; no real screen
or input was used.

If the first scroll frame fails, both shells now offer retry instead of claiming
that a captured portion can be saved. Existing partial-capture messages remain
unchanged. Focused GPUI and Electron event tests cover zero and nonzero frame
counts; native notification presentation remains unverified.

The macOS window screenshot, recording and recording-outline handlers previously
converted signed window IDs to `CGWindowID` without consistently validating the
range. A negative or oversized ID could trap the daemon; non-integer inputs could
also reach an unsafe numeric conversion. All three handlers now reject null-window,
negative, oversized and non-integer IDs before using capture APIs. The
[native validation fixture](tests/native/window-id-validation.swift) round-trips
invalid values and valid numeric boundaries through the JSON types and all three
handlers. Its test build stops immediately after validation, so neither success nor
failure cases start capture, open windows or access devices.

Screenshot-area and recording-start requests also accepted the null window ID in
their exclusion lists. The screenshot path could then treat that unresolved ID as
an absent control and proceed without excluding it. Both handlers now reject null
IDs before starting capture. The screenshot handler also validates a supplied
display ID as a representable integer before conversion; an out-of-range floating
point value previously reached a trapping integer conversion. The native validation
fixture checks invalid, mixed and valid exclusion lists and display IDs without
calling capture APIs. This validates request handling, not live exclusion pixels.

A separate direct-daemon probe covered the only connected display with an
isolated test pattern and placed a magenta stand-in control over it. It recorded
the full display and a 300×200-point area, first including the control as a
positive check and then excluding its window ID. On two fresh daemon processes,
the decoded 3456×2234 display and 600×400 area videos contained the control in
the positive checks and the underlying pattern in the exclusion checks. In the
second run, all eight decoded frames of each excluded recording had zero magenta
samples; every frame of each positive recording contained magenta. Logs and
synthetic videos are `daemon-display-probe*.log` and `display-*.mov` in the same
`window-stream-fixture/` directory. The same screen-only daemon restrictions
apply. This verifies exclusion of a supplied native window ID, not that GPUI
registers every real control and overlay or that its editor/export matches.

On 2026-11-16 the temporary `window-stream-fixture/` directory was no longer
present, so the logs and videos cited from that directory below cannot currently
be inspected. The measurements remain a record of the earlier runs; the
separately linked images under `tests/native/evidence/` remain in the worktree.
Repeat those native probes and retain their outputs in persistent storage before
using the missing videos or logs as release evidence.

An isolated native GPUI pre-recording toolbar was then placed above a small
synthetic backdrop. A screen-only Swift daemon recorded that area with the actual
toolbar window ID supplied for exclusion. A positive recording without the ID
contained 3,024 changed sampled pixels from the toolbar. The excluded recording
matched the backdrop, including after the daemon showed its actual full-display
recording overlay during the stream: zero sampled pixels changed by more than
20 levels in the tested region. The toolbar's GPUI process used a fake daemon;
the real screen-only daemon was driven directly, so this does not verify the
GPUI start button or its transfer of the ID. Evidence is
`gpui-control-probe-separate-daemon.log` and `gpui-control-*.mov` in the same
fixture directory. Before using the fake UI daemon, two attempts with two copies
of the instrumented screen-only daemon timed out starting the test recording;
one logged ScreenCaptureKit connection interruption `-3805`. This does not
establish a failure of the app's normal single-daemon recording flow.

An opt-in native GPUI harness subsequently invoked the real area recording
control's start and stop handlers with one instrumented daemon, using an isolated
profile and synthetic backdrop. The daemon checked its own existing screen grant
and disabled system audio, microphone, camera, keyboard and cursor tracking for
the probe. GPUI showed the actual recording overlay, started and finalized a
1456×320, 18-frame project, and opened its 208×148 preview 20 points from the
bottom-right work-area edges. All 18 decoded frames were at least 99.93% the
expected blue backdrop in sampled pixels, with a small animated marker accounting
for the remainder; no toolbar or overlay was detected. The run and project are
under `window-stream-fixture/gpui-integrated-1794755586882217000/`, and the log
is `gpui-integrated-probe-bottom-right.log` beside it. This drives GPUI's handlers
programmatically rather than exercising the selector or physical pointer input,
and it does not verify audio or camera.

The same one-daemon GPUI harness recorded the full 1728×1117-point display over
a synthetic backdrop. All 18 decoded 3456×2234 frames were at least 99.99% the
expected backdrop in sampled pixels; the toolbar and recording overlay were not
detected, and the preview was again 20 points from the bottom-right work-area
edges. Its project is under
`window-stream-fixture/gpui-integrated-screen-1794755697139936000/` and its log
is `gpui-integrated-screen-probe.log`.

For a window target, the harness selected an isolated 400×240-point window and
started recording through GPUI. The native outline remained the same window ID
and followed the target's 180-point horizontal and 100-point vertical movement.
Closing the target caused GPUI to stop normally, remove the outline, retain the
800×480, 14-frame take and open the bottom-right preview. All decoded frames
contained the expected synthetic pattern in sampled pixels; changed pattern
content was present after movement. The project is under
`window-stream-fixture/gpui-integrated-window-1794755853621575000/`, and the log
is `gpui-integrated-window-probe-final.log`. These harness runs use the same
permission-safe daemon and programmatic handlers described above; selector,
physical input, audio and camera remain unverified.

The same isolated native harness drove GPUI's screenshot selector with internal
pointer events on the one connected 1728×1117-point display. With freeze disabled,
an area drag over a solid synthetic blue window saved a 600×400 PNG containing
the selector's dimming and handles. The selector itself was gone when the preview
appeared. After allowing the drag to redraw before release, the visible selection
was offset 33 points below the saved crop: Core Graphics reported the full-height
selector at y=33 instead of the display's y=0. A fast drag captured an entirely
dimmed image. The five-second timer capture also saved a 66-pixel-high dimmed
strip at the top of the 2× PNG. The original failures are preserved as
synthetic evidence: [area selector captured](tests/native/evidence/macos-selector/area-selector-captured.png)
and [timer dimming captured](tests/native/evidence/macos-selector/timer-dim-captured.png).
Calling `orderOut:` before capture did not reliably remove the selector from the
captured frame, and setting the native frame during and just after window creation
did not correct its reported position; neither attempt was retained. This probe
does not establish whether normal pointer timing changes the frequency of the
capture race. It used a test-only, screen-authorized daemon with audio and input
tracking disabled, and did not modify privacy settings.

The selector was a titled AppKit panel, which AppKit constrained below the menu
bar. Removing its native frame and applying the display bounds after opening
placed it at y=0 on this display. Live area captures now pass the selector's
native window IDs to the daemon; ScreenCaptureKit excludes them explicitly,
and frozen captures continue to use the retained frame. In isolated fresh app
launches, a slow drag, a fast drag and a five-second timer each saved a 600×400
PNG with no pixel more than six RGB levels from the synthetic blue backdrop.
The test examined every pixel, including the edges where the handles had appeared.
An additional capture taken while the selector was still visible also excluded
it and had the expected 2× dimensions. The native window list was empty of the
selector after each saved capture and the preview opened at the bottom right.
The resulting [area](tests/native/evidence/macos-selector/area-clean.png) and
[timer](tests/native/evidence/macos-selector/timer-clean.png) images are retained
alongside the earlier failures.
The daemon used the production screenshot algorithm with a temporary preflight
wrapper and screen-only recorder. After those runs the same geometry calculation
was extracted into a separately tested helper. The test did not exercise physical
mouse input or a second display. Geometry tests cover a negative display origin
and 1×/2× dimensions; real mixed-display and color-profile comparisons remain
open. The before images above remain evidence of the original regression.

An offline geometry check found that independently rounding a half-point origin
and width could extend a selection by one point past the display edge, causing
the new bounds validation to reject it. Selected capture rectangles now round
both edges and derive their width and height from those rounded edges, while
the existing conversion for other window layouts is unchanged. Tests cover the
right and bottom edges at positive and negative display origins; this case has
not yet been rechecked with a native pointer.

After the selector changes, the fake-daemon GPUI visual workflow completed at
1280 points in dark mode at 1× and at 950 points in light mode at 2×. The dark
run covered the simulated capture, timer, OCR/QR, scroll, annotation, save and
reopen workflows; the narrow light run covered the visual layouts, preview and
history. They produced 199 and 195 PNGs respectively under
`/tmp/poratake-macos-smoke-offline-dark-final/` and
`/tmp/poratake-macos-smoke-offline-light-retry/`. The first attempted run exposed
a headless-only failure because its simulated selector has no native window
handle; the parity fixture now uses simulated positioning and no native window
IDs. These results do not verify native capture or physical input.

The macOS fake-daemon workflow now supplies two synthetic selector IDs and
asserts their exact presence in area, display and timer capture requests. A
subsequent full dark 1280-point, 1× run completed with 35 matching area requests:
13 cached and 22 live. Both completed timer captures retained the IDs after the
countdown. The log and request record are under
`/tmp/poratake-macos-exclusion-handoff/`. This tests the handoff and wire format;
it does not test ScreenCaptureKit or replace the paused native rerun.

During a later repeat probe, a macOS screen-access dialog for Poracode Nightly
appeared even though the test daemon had reported an existing screen grant.
The probe stopped; the dialog and privacy settings were left untouched. Its
cause was not established, and the partial test image containing the dialog
was deleted. Further live capture requires a way to avoid triggering or
interacting with a permission dialog.
The later [read-only attribution check](tests/native/evidence/macos-ui/screen-permission-attribution-20261117.txt)
found ScreenCapture entries for the signed Poratake and Poracode bundles, while
the development GPUI executable has a separate ad-hoc identity. A retained
input-monitoring preflight was attributed to Poracode, but that is not a
screen-capture result. Available historical logs did not establish the cause of
the earlier dialog, so they do not justify another live attempt under the
no-dialog constraint.

The window screenshot selector independently saved a clean 800×480 image of a
synthetic target, dismissed the selector and opened the bottom-right preview;
the [saved window image](tests/native/evidence/macos-selector/window-clean.png)
contains the target pattern without selector pixels. The selection was driven by
an internal pointer event; physical mouse interaction and another display remain
unverified.

Each of those three GPUI-created projects was then passed through the isolated
video-editor drawing, save/reopen and H.264 export test. The first and later
decoded export frames matched the corresponding editor previews within the
test's compression tolerance. This is an editor-harness comparison rather
than a native file-dialog or pointer-driven export.

The Mac video-editor harness exposed a critical decode failure with the pinned
FFmpeg 9.0.1: its decoder passed the removed `-vsync` option, so FFmpeg exited
without frames. The editor could show a black source preview and export a black
source image with annotations on top. The decoder now uses
`-fps_mode passthrough`, preserving the intended frame timestamps. With the
pinned FFmpeg, 37 Mac video-editor tests passed, including a drawing gesture,
save/reopen and decoded first-frame comparison, timeline normalization and export
cancellation. The drawing workflow also passed when supplied each of the three
synthetic native screen, area and moved/closed-window recordings above; its
changed-pixel RGB mean errors versus the edited previews were 25.20, 4.75 and
18.06 respectively. A second comparison after seeking also passed for each
recording; at 1.5 seconds the window recording's changed content was present in
the preview. These are lossy H.264 comparisons, not pixel-identical results or
interactive native-editor evidence. The Mac 2× dark
parity run also loaded the video and exercised all sidebar tabs in the headless
GPUI renderer; its images are under `/tmp/poratake-macos-video-parity/`. A light
950px parity run passed too, although its loaded video editor used a 1200px
minimum fixture width at the time.

The Mac fixture now also opens the loaded video at the requested width, reflecting
the editor's reduced minimum on a small display. At 720 points the timeline bar
clipped Scrub Audio and keyboard help. The bar now scrolls horizontally on macOS;
the Metal headless workflow scrolls to Scrub Audio, toggles it on and off, then
scrolls back to Play in both light and dark appearance at 2×. A 950-point dark
run covers the wider loaded editor. The
[scrolled synthetic editor](tests/native/evidence/macos-ui/video-preview-visible-controls-scrolled.png)
shows the controls. Physical trackpad and keyboard navigation remain unverified.

The loaded-video workflow now also accepts a reduced height. At 720×520 and
simulated 1× in both appearances, it checks the rendered red annotation and blue
video, all sidebar tabs, timeline scrolling and keyboard focus, and the export
button and canceling status within the viewport. The [dark editor](tests/native/evidence/macos-ui/video-small-workarea-dark.png),
[dark export panel](tests/native/evidence/macos-ui/video-small-workarea-dark-export.png),
[light editor](tests/native/evidence/macos-ui/video-small-workarea-light.png) and
[light export panel](tests/native/evidence/macos-ui/video-small-workarea-light-export.png)
are retained. This uses generated media and synthetic input; it does not verify
native small-display placement, physical input or the save dialog.

At the same 720×520 simulated viewport, an isolated UI test selected the
rectangle tool, drew on a generated video, saved and reopened the project, then
completed an MP4 export to a supplied path. The test checked the viewport on
both editor instances and compared decoded frames at the start and at 1.4 seconds
with their respective previews. Whole-frame mean channel differences were 1.03
and 1.08; the first annotation region averaged 5.03. On the 1,196 pixels changed
by the drawing, the decoded export's RGB mean difference was 20.77 from the
edited preview versus 118.72 from the original preview. The [preview](tests/native/evidence/macos-ui/narrow-video-preview.png),
[decoded frame](tests/native/evidence/macos-ui/narrow-video-export-frame.png),
[later preview](tests/native/evidence/macos-ui/narrow-video-later-preview.png),
[later decoded frame](tests/native/evidence/macos-ui/narrow-video-later-export-frame.png),
[synthetic MP4](tests/native/evidence/macos-ui/narrow-video-export.mp4) and
[measurements](tests/native/evidence/macos-ui/narrow-video-export-metrics.json)
are retained. The test bypasses the save dialog and does not exercise a live
recording or native pointer delivery.

On 2026-11-16, fresh isolated 720×520, 2× simulated light and dark runs both
completed on the current worktree. Their loaded-video screenshots are retained in
[light](tests/native/evidence/macos-ui/simulated-small-video-20261116-light.png)
and [dark](tests/native/evidence/macos-ui/simulated-small-video-20261116-dark.png),
with [decoded pixel measurements](tests/native/evidence/macos-ui/simulated-small-video-20261116-pixels.json).
The image viewer displayed the dark preview as black, but decoding the PNG found
the expected red annotation and blue video pixels in both images. System
Information reported one online 3456×2234 built-in Retina display and the
system appearance was dark; no additional display was available to check. The
synthetic runs do not verify native placement, input or captured pixels.
The same local run passed the Swift fixtures for recorded-window closure
classification, stopped-frame handling, display-coordinate conversions and
screenshot capture geometry. Those fixtures do not start ScreenCaptureKit.
Further isolated 720×738 and 720×500 runs passed at simulated 2× in both
themes. The 720×500 runs rendered 173 states each, including a loaded video
editor and horizontal toolbar and timeline-control scrolling. The generated
screenshot editor is retained in [light](tests/native/evidence/macos-ui/compact-editor-720x500-light-20261116.png)
and [dark](tests/native/evidence/macos-ui/compact-editor-720x500-dark-20261116.png),
and the video editor in [light](tests/native/evidence/macos-ui/compact-video-720x500-light-20261116.png)
and [dark](tests/native/evidence/macos-ui/compact-video-720x500-dark-20261116.png).
The harness now checks the saved video screenshot itself for the generated red
annotation and blue source; FFmpeg decoding also found both colors in each image.
An image viewer again displayed the light preview black even though the decoded
PNG contained those pixels. These headless runs use a fake daemon and are not a
live capture, native-window or Electron visual comparison.

The fixture also checks the rendered screenshot for the synthetic video's red
annotation and blue checkerboard, in addition to comparing the in-memory preview
against the composed frame. An image viewer initially displayed some of these
screenshots with a black preview, but direct PNG pixel inspection confirmed that
the expected content was present. This check does not establish that the preview
is always visible in a real window.

The three existing synthetic native recordings were also edited offline with a
cut and 1.5× zoom, saved and reopened, then exported through VideoToolbox. Four
decoded frames from each export matched the edited previews with mean channel
errors below 0.87, and changed-pixel checks confirmed that zoom affected the
preview and appeared in the export. Separate generated-video tests passed for
annotations and redactions, and for a synthetic camera track with visibility
ranges across a cut and zoom. These checks did not access a physical camera or
exercise interactive editing. The macOS export tests requiring development
FFmpeg are opt-in because the clean macOS CI job does not provision that binary;
they were run locally with the checkout's pinned FFmpeg 9.0.1.

The edited GIF workflow also ran on macOS using generated media at all six
offered frame rates. After saving and reopening the edit, the test checked
annotation and source colors, opaque output, frame counts and playback timing.
Each two-second GIF lasted 2,000 ms, with at most 5 ms accumulated timing error.
The macOS FFmpeg path scales this fixture's 184×114 preview to an 854×528 GIF,
as Electron's fixed-width conversion does; Windows' direct GIF path keeps the
184×114 source size. After resizing the decoded GIF back to preview dimensions,
the sampled frames had mean BGRA errors of 1.098–1.908. Between 148 and 219 of
83,904 channels per sampled frame differed by more than 32 levels. At one
translucent padding pixel with source alpha 1, the preview was white and the
resized GIF was approximately `[209, 213, 211]`. The test retains the Windows
per-pixel padding assertion; at the time the macOS result was recorded without
that assertion. The [measurements](tests/native/evidence/macos-ui/gif-preview-export-20261116.txt)
are generated-media evidence, not an interactive recording or physical display
comparison.

A follow-up stage probe rendered the same 50 fps fixture at the MP4 export's
776×480 size, compared that unencoded composition, a separately encoded and
decoded MP4, and the decoded GIF after resizing each to the 184×114 preview.
At the alpha-1 sample, their RGB values were `[211, 217, 216]`, `[210, 215,
213]`, and `[209, 213, 211]`, versus `[255, 255, 255]` in the preview. Their
whole-frame mean BGRA differences were 0.667, 1.707 and 1.788 respectively;
131, 149 and 149 of 83,904 channels differed by more than 32. The difference
therefore already appears in this size-dependent composition comparison before
video encoding. The [measurements](tests/native/evidence/macos-ui/gif-preview-export-stages-20261116.txt),
[preview](tests/native/evidence/macos-ui/gif-stage-preview-20261116.png),
[unencoded composition](tests/native/evidence/macos-ui/gif-stage-composition-20261116.png),
[MP4](tests/native/evidence/macos-ui/gif-stage-mp4-20261116.png) and
[GIF](tests/native/evidence/macos-ui/gif-stage-gif-20261116.png) are retained.
The opt-in fixture can regenerate its synthetic stage images with
`PORATAKE_GIF_DIAGNOSTIC_DIR`.

An isolated Electron canvas and WebCodecs probe then exercised the same gradient
using the checkout's `renderGradientToCanvas`. At a half-transparent pixel,
its canvas over black was `[64, 32, 16]` and its decoded H.264 frame was
`[64, 31, 16]`; the alpha-1 decoded sample was black. The
[probe](tests/native/electron-video-alpha.cjs) and [output](tests/native/evidence/macos-ui/electron-video-alpha-20261116.json)
use generated content. GPUI had instead unpremultiplied translucent RGB and
forced alpha to opaque, making the half-transparent pixel `[128, 64, 32]` and
amplifying tiny values near alpha zero. The video preview, MP4 input and direct
GIF output now composite their premultiplied pixels over black. A unit test
covers alpha zero, one and 128, and the existing preview/export test asserts the
half-transparent pixel against the Electron result. After the change, the same
alpha-1 preview, unencoded composition and decoded GIF samples were near black;
the channels differing by more than 32 in that stage comparison fell from 131,
149 and 149 to 5, 5 and 5 respectively. The [new measurements](tests/native/evidence/macos-ui/gif-preview-export-stages-black-20261116.txt),
[preview](tests/native/evidence/macos-ui/gif-stage-preview-black-20261116.png),
[composition](tests/native/evidence/macos-ui/gif-stage-composition-black-20261116.png)
and [GIF](tests/native/evidence/macos-ui/gif-stage-gif-black-20261116.png) are
retained. The GIF workflow now applies its existing padding-pixel assertion on
macOS as well as Windows, and the edited MP4 and synthetic camera tests pass.
An isolated dark 1280-point GPUI parity run also completed after the change;
its [workflow results](tests/native/evidence/macos-ui/opaque-video-parity-20261116.txt)
include generated screen, area and window preview/export comparisons, screenshot
annotation and save/reopen, and scroll workflow fixtures. It used a fake daemon.
These checks do not establish parity for every translucent image, display or
interactive recording.

The shared GPUI code and its test targets also passed an offline
[Windows cross-check](tests/native/evidence/macos-ui/opaque-video-windows-crosscheck-20261116.txt)
using the cached SDK. This compiles the Windows GIF path but does not execute it.
On 2026-11-16 a fresh dark-theme 1280-point, 1× simulated workflow completed on
the current worktree. Its generated area, window and screen projects were edited,
saved, reopened and exported; decoded frames at 0.5, 1 and 1.5 seconds differed
from their corresponding in-memory previews by mean BGRA channel values of 0.653,
0.713 and 0.712 respectively for each fixture. The saved H.264 videos, 1-second
decoded frames and editor screenshots are retained for [area export](tests/native/evidence/macos-ui/simulated-recording-20261116-area.mp4),
[area decoded frame](tests/native/evidence/macos-ui/simulated-recording-20261116-area-decoded.png),
[area editor](tests/native/evidence/macos-ui/simulated-recording-20261116-area-preview.png),
[window export](tests/native/evidence/macos-ui/simulated-recording-20261116-window.mp4),
[window decoded frame](tests/native/evidence/macos-ui/simulated-recording-20261116-window-decoded.png),
[window editor](tests/native/evidence/macos-ui/simulated-recording-20261116-window-preview.png),
[screen export](tests/native/evidence/macos-ui/simulated-recording-20261116-screen.mp4),
[screen decoded frame](tests/native/evidence/macos-ui/simulated-recording-20261116-screen-decoded.png)
and [screen editor](tests/native/evidence/macos-ui/simulated-recording-20261116-screen-preview.png).
[Stream metadata and hashes](tests/native/evidence/macos-ui/simulated-recording-20261116-streams.json)
confirm three 320×180, two-second, 60-frame H.264 files. The source media and
daemon were simulated. An independent [comparison of the rendered editor
screenshots with the decoded 1-second frames](tests/native/evidence/macos-ui/simulated-recording-20261116-comparison.json)
averaged each 2×2 screenshot block to the export size: the mean RGB channel
difference was 1.570 for each case, with 4,780 of 172,800 channels differing by
more than 12 levels. All three generated targets produced the same decoded frame,
so they are not independent source-content tests. This does not establish native
target movement, closure, capture controls exclusion or hardware access.

The generated handoff workflow now accepts three distinct screen, area and
window sources and checks that each project retains its assigned source frame.
In light and dark appearance, each was edited with a zoom, rectangle and a
quarter-second video cut, saved, reopened and exported. The three decoded
outputs differ, and their in-memory previews at 0.5, 1.0 and 1.5 seconds have
mean channel differences below 1.56. At one second, an independent comparison
of the rendered editor screenshot with the decoded frame has mean RGB
differences of 1.892 for area, 1.636 for window and 1.650 for screen; 8,137,
5,886 and 6,339 of 172,800 channels respectively differ by more than 12 levels.
The fixture now checks the rendered preview as well as the composition buffer.
The [measurements](tests/native/evidence/macos-ui/distinct-recording-preview-export-20261117.txt)
list the retained generated inputs, editor screenshots, decoded frames and MP4s.
These use a fixture daemon, not real capture or devices, and do not prove
physical target movement, closure or capture-control exclusion.

A follow-up [stage comparison](tests/native/evidence/macos-ui/video-render-export-stages-20261117.txt)
separates editor rendering from export on those three generated recordings.
At simulated 100% display scale, every displayed video-preview pixel matches the unencoded
composition. At 125% and 200%, the display-size previews match a separately
scaled composition within one channel level. Resizing the displayed images
back down adds differences around high-contrast edges, so that normalized
comparison is not a direct measurement of the original composition.

At 100% scale, independent RGB→YUV420→RGB conversions reproduce nearly all of
the decoded exports' errors above 32 levels: 1,563 of 1,570 for area, 1,997 of
1,998 for window, and 801 of 802 for screen. The decoded frame's mean RGB
difference from the conversion-only result is at most 0.136. The retained
[conversion results](tests/native/evidence/macos-ui/video-chroma-yuv-20261117.json)
and an isolated [Electron WebCodecs probe](tests/native/evidence/macos-ui/video-chroma-electron-20261117.json)
use generated high-contrast content. The Electron probe encodes a single frame
and does not exercise its complete editor or exporter. These measurements do
not establish equivalence for other content, live capture or physical displays.
The generated-audio editor test also passed on macOS: it compared preview and
decoded export across a cut, two playback speeds, volume changes and synthetic
keyboard clicks. The Mac sample's mean click contribution was below the
Windows fixture's absolute cutoff, so the Mac assertion checks that the click
changes the mix and remains distinguishable from export error. This is a
file-only comparison, not a microphone, system-audio or listening test.
Another file-only test generated separate system and microphone M4A sidecars,
with distinct tones that change from 440/880 Hz to 660/1100 Hz across the
source. The expected tones appeared in the preview mix and decoded exports on
both sides of a cut, and muting each track separately removed its tone from
both preview and export. The sources were synthesized;
this does not verify device access, live recording or the timing of live toggles.

A generated first-frame intro exposed two more Mac export defects. At exact
video frame boundaries, rounded FFmpeg `pts_time` values and floating-point
subtraction could leave the preceding frame visible. The Unix decoder now uses
integer presentation timestamps with FFmpeg's time base and allows one
representable floating-point step at the requested boundary. The intro's audio
delay was staged as 15,999 zero bytes at 12 fps, splitting a 16-bit stereo frame
and corrupting the following audio. The Mac encoder now rounds the delay to
whole audio frames before writing silence. The native VideoToolbox regression
checks the intro and two changed source frames against decoded export, verifies
silence during the intro and a recognizable tone afterward, and failed on the
old padding. Unit checks cover the timestamp precision and byte alignment. The
audio and camera fixture exports above also passed after these changes; no live
devices were used.
The normal Unix decoder tests now also drive the complete frame-selection path
with synthetic FFmpeg output at and just before fractional frame boundaries,
without requiring FFmpeg or screen access. The macOS audio-padding unit test
checks whole stereo frames at 12, 15, 24, 25, 30 and 60 fps.

Code review also found that rebuilding preview audio removed the old output
before reading whether it was playing. The replacement therefore paused until
the next playback tick even when playback was active. The handoff now retains
that state before taking the output. A software-mixer test now replaces stems
while playing and paused, checks the player state and seek anchor, and uses no
hardware. This handoff has not been exercised against a real output device.

An additional offline macOS run passed 13 recording-control, 35 capture-preview,
27 selector and 14 scroll-control GPUI tests. These exercise simulated device
menus, preview placement and dismissal, selector geometry and scroll controls;
they do not access real devices or establish native interaction. The Swift
scroll fixture also passed its synthetic preview/stitch and filter lifecycle
checks with its ScreenCaptureKit exclusion check left disabled.

The user's screenshot showed a second rounded outline around the recording toolbar.
The native GPUI popup was a titled AppKit panel even though the toolbar draws its
own border. Removing the titled frame for the recording toolbar and screenshot
preview eliminated the extra outline in isolated native previews on macOS. The
[recording toolbar](tests/native/evidence/macos-ui/recording-toolbar.png) is
preserved; the one-card `capture-preview-final.png` is under
`/var/folders/_r/p8gqgcd93zb1w708g0h__ztw0000gn/T/poratake-native-smoke-5yrih1e3/`.
The screenshot preview was placed at the bottom right of the sole available
display. These checks verify visible frames and placement, not interaction,
tooltips, capture handoff or recording.

The macOS scroll preview, cursor prompt and control bar also used GPUI's titled
popup configuration while drawing their own content. They now remove the native
frame and shadow using the same helper as the recording toolbar. A hidden native
GPUI popup with the scroll-control options had a titled frame and shadow before
the helper and neither afterwards; its AppKit frame stayed unchanged and the
window could still be made click-through. The [probe result](tests/native/evidence/macos-ui/scroll-popup-native-frame.txt)
also checks the three actual scroll windows hidden on the primary display with
a full-display selection: each is borderless and positioned as requested; only
the preview and prompt are click-through. It does not show the windows or verify
their pixels in a capture. Real scroll capture and overlay exclusion remain open.

The [hidden native popup probe](tests/native/evidence/macos-ui/native-tooltip-bridge-20261116.txt)
also sets, replaces and clears the AppKit view tooltip through the shared GPUI
helper, including a Unicode label. The native view returned the expected text and
then no tooltip, without changing the popup frame. This checks the bridge used by
recording controls and capture previews; it does not establish actual hover
delivery, visible tooltip placement or exclusion from recorded pixels.

The native tray menu also showed a rounded frame extending well below its content,
as captured in the [before image](tests/native/evidence/macos-ui/tray-before.png). The menu is now created
hidden, made borderless, and activated after its view is ready. Disabling the native
window shadow removes a remaining curved outline; the menu's window is then resized
to its measured content height. In isolated dark and light native previews it
stayed visible and its bounds shrank from 304×632 to 304×548 points, matching its
rendered menu; the [dark](tests/native/evidence/macos-ui/tray-dark.png) and
[light](tests/native/evidence/macos-ui/tray-light.png) images record the result.
The existing fitted-menu test now also runs on macOS at 1×, 1.25×, 1.5×
and 2×. Live tray interaction and dismissal remain unverified because the
computer-use service does not expose the window.

The native screenshot preview previously used a fixed 208×604-point window for a
single thumbnail. AppKit's window hit test confirmed that a point in the empty
space above the card targeted the preview instead of the window underneath. The
macOS preview window now fits its stack: a native one-card probe measured 208×148
points, and the same empty point reached the underlying window while a point on
the thumbnail reached the preview. A top-left one-card probe also measured 208×148
points and returned the underlying window below the preview. A separate instrumented native probe opened
three cards in a 208×452-point window and showed all three cards in the
[preview stack](tests/native/evidence/macos-ui/preview-stack.png). Automated
checks cover stack growth and shrinkage, all four corner anchors, and a simulated
multi-display menu inside a one-card window. The narrow gaps between cards and live
preview interaction remain unverified. After the change, isolated 2× parity runs
passed at 1280px in light mode and 950px in dark mode, including all four preview
corners, a four-card stack, tooltips, display menus and editor handoff. The native
window remains rectangular, so transparent corners and the gap between multiple
cards can still intercept clicks. Physical mixed-DPI placement remains unverified.

An isolated native GPUI probe opened the recording control and a generated-image
preview in both light and dark mode without starting a daemon. AppKit reported both
visible, borderless and without native shadows. On the available display, the
208×148-point preview had 20-point bottom and right margins relative to the AppKit
work area. The [probe log](tests/native/evidence/macos-ui/native-ui-frame-20261116.txt)
records the measured frames. The computer-use helper did not list either floating
window as targetable, so this check does not verify hover, tooltips, screenshot
pixels or interaction.

Lowering only the isolated probe windows to the normal AppKit window level made
them targetable by the already-authorized computer-use helper. Native window
images show one toolbar border in [dark](tests/native/evidence/macos-ui/native-control-normal-level-dark-20261116.png)
and [light](tests/native/evidence/macos-ui/native-control-normal-level-light-20261116.png),
and the generated preview in [dark](tests/native/evidence/macos-ui/native-preview-normal-level-dark-20261116.png)
and [light](tests/native/evidence/macos-ui/native-preview-normal-level-light-20261116.png).
The helper captured those windows through its own ScreenCaptureKit access; no
Poratake capture or recording was made. The production floating level, physical
hover, native tooltip presentation and capture exclusion remain unverified.
A later isolated probe checked both themes at the actual AppKit popup level 101.
AppKit reported borderless windows without native shadows; after layout the
recording control measured 276×44 points, with no transparent side margin, and
an outside hit test reached another window. The generated GPUI scenes contain a
single toolbar border in [dark](tests/native/evidence/macos-ui/recording-control-floating-scene-dark-20261116.png)
and [light](tests/native/evidence/macos-ui/recording-control-floating-scene-light-20261116.png).
These images omit the macOS compositor, so the outer wrapper in the user's
screenshot remains unresolved. The helper still could not target the floating
windows. The [native measurements](tests/native/evidence/macos-ui/recording-control-floating-native-20261116.txt)
also record a normal-level control check and timer edge calculations on the one
available display. No real capture, physical hover or tooltip presentation was
tested in this probe.
The macOS parity-feature test build now excludes standalone native-probe helpers
from unit-test builds. Strict clippy passes for all parity targets, and the
parity-enabled recording-control, capture-preview and selector-overlay suites
pass 15, 39 and 30 tests respectively. These suites use simulated interaction.
A background click on the generated preview was delivered to the lowered probe
window, but the helper observed no change and the hover controls did not appear.
The physical pointer did not enter the window, so this does not establish whether
hover works for a user.
In a later isolated normal-level probe, moving the generated preview under the
stationary pointer and dragging into its Close control made the real pointer
land over the control. The preview reported card and pointer hover, and its
AppKit tooltip text was “Close preview”; the helper's accessibility tree also
exposed the hovered controls. A window-list query did not identify a separate
tooltip window near the pointer after the drag. Since the pointer entered while
the mouse button was held and the query cannot establish all tooltip presentation,
this does not prove ordinary hover display or a product defect. The isolated
probe was closed and focus returned to Poracode. The [native record](tests/native/evidence/macos-ui/preview-tooltip-drag-20261116.txt)
contains the observations; ordinary physical hover and recorded-output exclusion
remain open.
An isolated native probe then dispatched a GPUI mouse move while the preview was
unfocused. The card's hover state became true, but GPUI's macOS window-hover value
remained false because it reflects window focus. The controls consequently stayed
hidden, as shown in the [before image](tests/native/evidence/macos-ui/native-preview-hover-before-20261116.png).
The preview now tracks mouse entry and window exit separately from focus on macOS;
it also clears and suppresses native tooltips after exit. In the same native probe,
the controls appeared on entry, disappeared on exit and reappeared on re-entry while
the window stayed unfocused; the [after image](tests/native/evidence/macos-ui/native-preview-hover-after-20261116.png)
shows the controls. A GPUI regression checks those transitions and the associated
auto-dismiss gate; all 36 local capture-preview tests passed. Input was simulated,
and native physical hover and tooltip presentation still require verification.

The macOS recording toolbar also reserved a transparent side margin for its
device menu. An AppKit hit test found that a point 14 points inside that margin
targeted the recording window. The closed toolbar now fits its measured content;
it expands for a menu, countdown or keyboard-focus tooltip and returns to the
fitted size afterward. In isolated native light and dark probes, the measured
toolbar and window were both 276×44 points, remained centered, and an outside
point reached another window. A fake microphone menu expanded the window to
332×300 and returned it to 276×44 on dismissal without querying a real device.
The [native results](tests/native/evidence/macos-ui/recording-control-window-fit-20261116.txt),
[light fitted toolbar](tests/native/evidence/macos-ui/recording-control-fitted-light-20261116.png),
[dark fitted toolbar](tests/native/evidence/macos-ui/recording-control-fitted-dark-20261116.png),
[fixture menu](tests/native/evidence/macos-ui/recording-control-fitted-menu-light-20261116.png)
and [keyboard tooltip](tests/native/evidence/macos-ui/recording-control-fitted-focus-dark-20261116.png)
are retained. The image checks use the headless renderer, and native input was
simulated. While a menu, countdown or keyboard tooltip is open, the expanded
transparent area can still intercept clicks; the fitted frame also clips the
toolbar's outer shadow. Native keyboard behavior, real capture exclusion,
multi-display positioning and a visual comparison of that shadow remain open.

The isolated dark native toolbar was also exercised with a fixture daemon for
the [camera menu](tests/native/evidence/macos-ui/recording-camera-menu-dark-native-20261116.png)
and an empty [iOS device menu](tests/native/evidence/macos-ui/recording-ios-menu-full-dark-native-20261116.png).
The long generated camera name was truncated visually but retained in the native
accessibility tree. Selecting “None” in the iOS menu through accessibility closed
it and restored the fitted toolbar; outside hit testing reached the underlying
window. No real device was queried. The helper also returned [one partial image](tests/native/evidence/macos-ui/recording-ios-menu-dark-native-20261116.png)
without the toolbar and one blank image; a later capture after simulated pointer
exit and entry showed the complete surface. The [probe record](tests/native/evidence/macos-ui/recording-device-menus-native-20261116.txt)
does not distinguish a helper capture artifact from an actual redraw problem.
The windows were lowered for inspection, so production floating-level appearance
and physical input remain open.

A follow-up reopened the isolated iOS menu and captured its unchanged window
eight consecutive times, including immediately after reopening; all eight
contained the toolbar. In another run, the same open menu was captured from
the [GPUI Metal scene](tests/native/evidence/macos-ui/recording-ios-menu-gpui-scene-20261116.png)
and the [helper window capture](tests/native/evidence/macos-ui/recording-ios-menu-helper-stable-20261116.png).
Both show the toolbar and menu. Against black, the 664×600 images had a mean
RGB channel difference of 0.24 levels; 58,211 of 1,195,200 channels differed
by more than two, with a maximum of 21.82. These captures do not establish why
the earlier helper images were partial or whether the visible compositor output
was affected. The comparison used an isolated lowered window and no capture or
media permissions from Poratake.

An isolated AppKit probe toggled the native recording-window shadow on and off.
The authorized computer-use helper captured the same 552×88-pixel window image
with either setting; its window capture ends at the frame and therefore cannot
show an external shadow. The [probe record](tests/native/evidence/macos-ui/recording-control-native-shadow-20261116.txt)
and [captured toolbar](tests/native/evidence/macos-ui/recording-control-native-shadow-20261116.png)
are retained. This does not establish the appearance or capture exclusion of a
native shadow, so the production shadow behavior is unchanged.

An isolated AppKit probe moved the generated preview under the stationary pointer.
The preview's card became hovered but its window pointer flag stayed false, and a
four-second auto-dismiss closed it. The macOS GPUI backend now forwards AppKit's
mouse-enter event as a pointer movement. With that change the unfocused preview
stayed open beyond the timer; after moving the window away, it closed. The move also
exposed stale window pointer state after card hover cleared, so the preview now
clears that state when the pointer is outside its viewport. The
[native probe record](tests/native/evidence/macos-ui/stationary-preview-hover-20261116.txt)
and AppKit event-conversion test cover this sequence without screen or device
capture. Physical pointer movement and popup overlap with other apps remain open.

The display and window pickers also used GPUI's macOS window-focus value as a
hover signal. Selector windows on other displays can be unfocused, leaving the
display dimmed or a hovered window unhighlighted. On macOS the selector now tracks
pointer entry and exit independently, initializes its pointer after positioning,
and re-evaluates the target when an asynchronous window list arrives. GPUI visual
tests with simulated input verify the dimmed and highlighted states on entry,
exit and re-entry, including a window list arriving under a stationary pointer.
An isolated AppKit picker also reported pointer states true, false and true after
simulated entry, exit and re-entry while its window remained unfocused. It used a
small test window and no capture. Actual multi-display pointer behavior, native
selector cleanup and capture output remain unverified by these tests.

An isolated AppKit preview was also moved so that its Close control lay under
the stationary system pointer. The preview became the top window at that point,
reported the card and pointer inside, and registered `Close preview` on its
native view while the window remained unfocused. Moving it away cleared both
hover states and the tooltip; moving it back repeated the result. The
[native probe record](tests/native/evidence/macos-ui/preview-native-close-hover-20261116.txt)
uses a generated preview and real AppKit entry and exit delivery, without moving
the pointer or starting a capture. It does not establish that AppKit visibly
presents the tooltip during physical pointer movement.

The generated preview was then lowered to normal window level so the authorized
computer-use helper could target it. Foreground pointer drags from the preview
interior to Close and Delete revealed its controls and registered the respective
native tooltip text; moving to the preview's margin cleared the card hover and
tooltip. The helper reported input delivery as unverified, and the probe's own
pointer and tooltip state supplied the subsequent observation. The [record](tests/native/evidence/macos-ui/preview-physical-hover-20261116.txt)
and [window image](tests/native/evidence/macos-ui/preview-physical-hover-20261116.png)
are retained. A drag beyond the window was refused, and the window-only screenshot
cannot show a system tooltip outside the frame. Physical window exit, visible
tooltip presentation and production floating-window input remain open.

In a separate normal-level preview run, a foreground pointer click on Edit
closed the generated preview and opened its checkerboard in an isolated GPUI
editor. The helper reported delivery as unverified; the new editor window and
its generated image were observed afterward. Although its accessibility tree
reported focus, macOS identified another application as frontmost, so no
physical drawing or keyboard input was sent. The [handoff record](tests/native/evidence/macos-ui/preview-physical-edit-20261116.txt)
does not establish physical annotation, save or reopen behavior.

An isolated editor attempt then used the computer-use helper's background drag
after dispatching the Rectangle tool action. The helper reported the drag as
unchanged and the exported image had zero changed pixels. A subsequent synthetic
GPUI drag changed 1,790 pixels and produced the [annotated editor image](tests/native/evidence/macos-ui/editor-native-annotation-20261116.png).
The native save sheet opened in the isolated temporary directory, but its Save
element returned stale-snapshot refusals and a background click left the sheet
unchanged. It was canceled; no output was saved or reopened. The
[attempt record](tests/native/evidence/macos-ui/editor-background-save-20261116.txt)
does not establish whether the background-input failures are in the app or the
helper, and does not replace physical annotation and save verification.

An AppKit probe also confirmed that calling `NSSavePanel.ok:` directly throws
an unimplemented-method exception for this sheet. That was a probe-only failure,
not evidence that a user click fails; the direct call was removed. A later
isolated run canceled the sheet, wrote the annotated image to a new temporary PNG
through the same screenshot save backend, and opened that file in a second
native editor. The 640×360 [saved PNG](tests/native/evidence/macos-ui/native-save-annotated-20261116.png)
differs from its [generated source](tests/native/evidence/macos-ui/native-save-source-20261116.png)
at 1,790 pixels and matches both editors' exported pixels exactly. The
[probe result](tests/native/evidence/macos-ui/native-save-reopen-20261116.txt)
and script are retained. The drawing and save were dispatched by the probe;
the native sheet was canceled, so a user's Save click remains unverified.

In a subsequent isolated native run, simulated movement over Close registered
“Close preview” on the actual AppKit view; moving to Delete replaced it with
“Delete screenshot”. A simulated window exit cleared the AppKit tooltip, and
re-entering over Close registered “Close preview” again. The preview remained
unfocused. This checks native tooltip registration and clearing, not whether
AppKit displays the tooltip during physical hover. The GPUI regression also
checks replacement and clearing of the test tooltip after these transitions.
With a four-second auto-dismiss setting in the isolated native probe, the
unfocused preview stayed open beyond the timer while simulated hover was active,
then closed after simulated exit. Its generated 640×360 source PNG still existed
afterward. A GPUI timer regression checks the same hold, exit and source-preservation
behavior; all 37 local capture-preview tests passed. No physical pointer or live
capture was involved.
After the native accessibility tree initialized, the lowered recording control
exposed its Start, camera, microphone, system-sound, iOS-device and Close buttons.
The helper invoked Close through macOS accessibility and confirmed that only the
isolated control window closed. No device menu or capture was opened. Production
floating-window accessibility and physical input still need verification. A
separate isolated run invoked the pre-recording system-sound button twice through
macOS accessibility; its label changed from “Turn system sounds on” to “Turn
system sounds off” and back. This changed only the temporary probe configuration
and did not start or test system-audio capture.
A separate native probe found that the unfocused recording control registered
“Start recording” on hover but retained it after a simulated window exit. GPUI
re-established its stale hover during redraw. The macOS control now clears the
tooltip on exit and restores it on subsequent pointer movement over the control.
In the isolated AppKit probe, the registered tooltip changed from “Start recording”
to none on exit and back on re-entry, while the window stayed unfocused. A GPUI
regression also checks Start and Close; all 14 local recording-control tests
passed. The [toolbar image after the fix](tests/native/evidence/macos-ui/native-control-tooltip-fix-dark-20261116.png)
is byte-identical to the earlier dark normal-level image. The probe used simulated
input and did not start recording or test native tooltip presentation with a
physical pointer.
After that change, a native GPUI event probe clicked the pre-recording system-sound
control twice. Its isolated preference changed from off to on and back to off;
the regression also checks both transitions. No recorder or audio device was
started, and this does not verify physical pointer delivery.
One background double-click on the generated preview was delivered but did not
open an editor or change the preview. This does not establish how a physical
double-click behaves; the native preview-to-editor handoff remains unverified.
In a separate isolated run, a simulated GPUI double-click on the generated
preview opened a native screenshot editor. Selecting Rectangle through AppKit
accessibility and dispatching a simulated GPUI drag drew the rectangle shown in
the [native editor](tests/native/evidence/macos-ui/native-editor-annotated-20261116.png).
The editor's Save button opened a macOS save sheet in the temporary probe directory.
The background helper refused the sheet's Save element as stale, and a background
Return did not dismiss the sheet; no annotated file was saved or reopened in this
run. The test process was stopped, leaving the user's editor untouched. Physical
input, the completed native save/reopen flow and real capture pixels remain open.
Another isolated run selected Rectangle through macOS accessibility and drew it
with a simulated GPUI drag. Accessibility then invoked Undo and Redo on the
native editor. Comparing the captured central image region, Undo changed 16,445
pixels by more than two RGB levels; after Redo, zero pixels differed by more than
two levels from the annotated state. The [undo](tests/native/evidence/macos-ui/native-editor-undo-20261116.png)
and [redo](tests/native/evidence/macos-ui/native-editor-redo-20261116.png) window
images are retained. This checks those accessibility actions on a generated image,
not physical pointer delivery or live capture.
After the renderer change, `bun run dev:gpui` launched an isolated native preview
with the instrumented daemon; macOS reported its 208×148-point window. The test
process was stopped and the existing Electron process remained open.

The normal GPUI Settings window was discoverable by the computer-use service.
Using an isolated profile, a native interaction switched Appearance from light to
dark and opened the Screenshot, Recording, Shortcuts and About pages. The Devices
page was not opened because it can request microphone or camera authorization.
PNG captures of the dark Appearance, Screenshot, Recording and About pages are
in the evidence directory above as `settings-appearance-dark.png`,
`settings-screenshot.png`, `settings-recording.png` and `settings-about.png`.
An attempted corner drag left the window at 880×700 points; it did not verify a
narrow native layout. Floating capture controls remain undiscoverable to the
service, so these checks do not verify interactive capture or recording.

At the screenshot editor's 950-point minimum width, the rectangle tool adds a
third option control and pushes the Pin action beyond the right edge. The macOS
toolbar previously clipped that action without allowing horizontal scrolling.
It now uses the same scrollable cluster as Windows. In isolated macOS headless
tests at 2×, both horizontal and vertical scroll input brought the action into
view, the Copy action responded after scrolling, and the options could be
reached again. Both light and dark 950-point runs passed, along with a forced
720-point dark run; 720 is below the normal editor minimum. The synthetic
[before](tests/native/evidence/macos-ui/editor-toolbar-before.png) and
[scrolled](tests/native/evidence/macos-ui/editor-toolbar-scrolled.png) images
show the 950-point case. The normal 1280-point dark workflow also completed,
including the fake-daemon capture handoffs and annotation save/reopen checks;
the Intel macOS parity build passed `cargo check`. Native mouse and trackpad
delivery remains unverified.

The screenshot editor previously requested its 950×650 minimum even when the
primary display's usable area was smaller. Its initial size now uses the work
area and reduces the native minimum when necessary. Geometry tests cover an
800×600 work area with absent, larger and smaller saved sizes, plus a degenerate
area; ordinary minimum-size checks remain unchanged. At 720×520, the available
size after the existing margins on an 800×600 work area, isolated light and dark
headless runs exercise the editor toolbar's scrolling, Copy action and keyboard
focus reveal. The [light](tests/native/evidence/macos-ui/editor-small-workarea-light.png)
and [dark](tests/native/evidence/macos-ui/editor-small-workarea-dark.png)
full-window images and [light](tests/native/evidence/macos-ui/editor-small-workarea-light-focused.png)
and [dark](tests/native/evidence/macos-ui/editor-small-workarea-dark-focused.png)
focused Pin images are retained. The normal-width dark synthetic capture,
recording and editor workflow also passed after the sizing change. These simulate
the smaller work area; native placement, pointer input and physical small displays
remain unverified.

Keyboard traversal exposed a separate issue in the same 950-point layout: Tab
could focus Pin while the entire control remained to the right of the viewport.
The macOS toolbar now reveals a focused control when focus changes, while manual
scrolling remains available. The isolated Metal workflow checks that Tab reveals
Pin and Shift-Tab reveals the first option after each starts clipped, with room
for the focus outline; both light and dark runs passed. The
[focused Pin screenshot](tests/native/evidence/macos-ui/editor-toolbar-keyboard-pin.png)
is from the dark run. This is synthetic input and does not verify a physical keyboard.
The video editor had a separate focus defect on all platforms: its root mouse
handler took focus after every child click. The root now relies on GPUI's focus
handling, so child controls retain focus and a click on the editor background
still returns focus to the editor. In the Mac 720-point fixture, Play and Scrub
Audio now keep distinct focus; Tab reveals the clipped audio switch and
Shift-Tab reveals Play again in light and dark appearance. In both fixtures,
Space also toggles the focused switch in both directions, and the C shortcut
changes the Cut control after a background click. The
[focused switch](tests/native/evidence/macos-ui/video-keyboard-scrub.png) is from
the dark run. These checks use synthetic input.

After Play received focus, pressing C left the GPUI Cut tool inactive. A
state-based synthetic check confirmed this; the earlier pixel-only probe was
inconclusive because button animation also changed pixels. Electron's shortcut
handler accepts C from a button but ignores input, textarea, select and editable
targets. GPUI now accepts editor shortcuts from HeroGPUI button key contexts while
keeping Space and Enter for button activation. The synthetic workflow checks C
and Command-S from Play, Space on Cut and Scrub Audio, and the zoom slider's arrow
key; C does not activate Cut from the switch or slider, and Command-S does not
open Export from the switch. A separate text-input
test checks that typing C, Ctrl-S and Command-S do not activate editor shortcuts. Native
keyboard delivery and other control types remain unverified.

In an isolated native editor, a foreground drag added a red rectangle to the
800×740 test image. Saving produced an 800×740 PNG with 3,140 changed pixels;
opening that PNG in a second GPUI editor showed the rectangle while the first
editor stayed open. The save panel briefly displayed the selected temporary
directory but later displayed Applications and saved there; the test file was
moved back to the evidence directory. The cause of that directory change has not
been established. The `rfd` 0.15.4 macOS backend appends the default filename to
the starting directory passed to `NSSavePanel`; a noninteractive AppKit probe
confirmed that the panel initially holds that non-directory URL. Whether this
explains the observed directory change is unverified. A follow-up isolated AppKit
probe observed the same filename-appended URL after the save panel became visible;
the directory-only control retained the expected directory. Programmatically
invoking the panel's OK action did not complete either case, so this does not
establish the selected destination or the cause of the earlier change.
The vendored `rfd` backend now passes the directory itself to AppKit and sets the
suggested filename separately for both save and open panels. In an isolated GPUI
editor using a generated image, the visible save sheet reported its temporary
directory and a separate suggested PNG filename; the repeatable
`tests/native/macos-save-panel-probe.py` check passed and canceled the sheet. The
[native record](tests/native/evidence/macos-ui/save-panel-directory-20261116.txt)
describes the check. It now selects Rectangle and verifies that a simulated drag
changes the generated image before opening the save sheet. In a separate native
run the rectangle was visible and the intended temporary directory appeared in
the sheet, but background accessibility returned stale IDs for its controls and
background Return and click left it open. The isolated sheet was canceled. The
subsequent authorized foreground attempt also left the sheet open; an AppKit
frontmost-application check reported Docker Desktop after the activation attempt,
so the test stopped without sending more desktop input. A separate app-local
synthetic Return event also left the sheet open. Neither attempt produced a PNG.
The GPUI crate also type-checked for Intel macOS; the
dialog itself was exercised on Apple Silicon. An attempted test-only call to
`NSSavePanel.ok:` terminated the isolated probe because AppKit reports that method
as unimplemented; the unsafe probe action was removed. The directory correction
does not establish the destination of a completed save or explain the earlier
Applications result; physical save, reopen and video export remain open.
Full-resolution captures
of the two editor views differ at
10,607 image-region pixels by more than two channel levels, concentrated around
the annotation edges (maximum 63). Evidence is `native-editor-annotated.png`,
`native-editor-full-original.png` and `native-editor-full-reopened.png` in the
same directory. Rendering screenshot annotation previews on the export sampling
grid corrected the same native test: `native-editor-full-after.png` differed from
the reopened image by at most two channel levels across the tested image region.
An isolated fixture also checks the draft-to-committed transition at 4× device
scale; it changed 4,709 pixels before aligning drafts and now matches. These are
image-editor checks; they are not evidence about recording or video export.

For a 1,000×500 source-pixel rectangle on an opaque 1,920×1,080 image at 1.6×
device scale, three uncached developer-build CPU timings on this Mac were
12.8–13.3 ms for the old draft rasterization and 18.2–19.3 ms for the initial
aligned path. Skipping unnecessary alpha conversion on opaque patches reduced
the aligned timings to 14.0–14.7 ms in a second three-run probe; the old path in
that probe took 12.2–14.7 ms. These are not interactive frame-rate measurements;
translucent images, large or numerous annotations and slower machines still need
profiling.

A direct native timer-control probe opened a one-second countdown, received its
completion event, and hid the panel successfully. It did not exercise the GPUI
timer capture workflow or verify the saved output.

A direct native scroll-capture probe used an isolated colored-row scroll window
and the daemon's native controls. Three preview frames were emitted and a
800×1770 PNG was saved from a 400×370-point selection. The output visibly
contains the test rows without the control panel or boundary frame. After matching
the reference screenshot's Display P3 profile to sRGB, 582,400 of 592,000 pixels
in the first viewport matched exactly; the larger differences were confined to
one-pixel horizontal row edges. The daemon also reported a no-overlap stop while
finishing; this single synthetic probe does not establish whether that decision
was correct. Evidence is in
`/var/folders/_r/p8gqgcd93zb1w708g0h__ztw0000gn/T/poratake-native-smoke-5yrih1e3/scroll-output.png`.
That temporary PNG was no longer present when checked on 2026-11-16, so the
measurements above are a historical record rather than retained release evidence.
It is not an interactive GPUI scroll workflow or a fixed-header test.

[Hosted CI at `71ec03ef`](https://github.com/Porabuild/Poratake/actions/runs/43662801738)
passed all active jobs, including the native workflows and Windows visual matrix;
Windows packaging was skipped.

[Hosted Windows CI at `badb3d5b`](https://github.com/Porabuild/Poratake/actions/runs/43402764907)
passed native recording with programmatic cuts and annotation, and compared three
preview/export frames. It did not exercise editing in the UI or capture audio.

[Hosted Windows CI at `1d6a4d0a`](https://github.com/Porabuild/Poratake/actions/runs/43419762563)
captured a real area, reopened the selector, and drew a rectangle in the screenshot
editor. Saving then crashed while opening the native dialog, so that run did not
verify the saved image or reopen.
[The next Windows run](https://github.com/Porabuild/Poratake/actions/runs/43425397059)
passed real capture and reopened the selector but did not reach the editor: its Edit
click did not wait for the hover controls to render. The harness now checks their
readiness and the preview HWND hit test before clicking.
[The Windows native workflow at `19e8db81`](https://github.com/Porabuild/Poratake/actions/runs/43429066842)
passed capture, rectangle annotation, the owned Save dialog and reopen, including
sampled output pixels and matching editor dimensions. The selector reopened and
cancelled without producing another capture, but capture protection prevented
checking its selection pixels. All active CI jobs passed; Windows packaging was
skipped.
[The Windows workflow at `cd9c5d0e`](https://github.com/Porabuild/Poratake/actions/runs/43435257885)
created a smaller capture inside the previous selection, confirming the selector
was fresh. It later failed a screenshot color check because the second preview
overlapped the editor and the resulting annotation crossed a sample point. The
revised harness closes that preview and samples away from the annotation.
[The Windows workflow at `16f9df47`](https://github.com/Porabuild/Poratake/actions/runs/43438907936)
passed the corrected capture, selector reset, annotation, save and reopen checks.
It then exposed a video editor sizing defect: the 1200×750 minimum clipped the
Export control on the 1024×768 runner before the export click.
[The Windows workflow at `215d9070`](https://github.com/Porabuild/Poratake/actions/runs/43445368824)
verified the resized editor and Export control within the work area, opened the
owned Save dialog and produced a stable file. The output pixel and duration check
failed before decoding: a later capture-preview tooltip overwrote the recording
tooltip's paint-proof metadata.
[The native Windows workflow at `258af434`](https://github.com/Porabuild/Poratake/actions/runs/43449868862)
passed with isolated proof files. It exercised the owned video Save dialog and
verified the decoded export's duration, dimensions, marker pixels and tooltip
exclusion. All active jobs passed, including 16 visual profiles and 2,912 baseline
comparisons on Microsoft Basic Render Driver; Windows packaging was skipped.
A GPUI test added at `e8d1b9fe` clicks Rectangle, draws on a recording fixture,
reopens the saved project and compares its preview with a decoded export, including
the changed annotation pixels. It passed in the Windows Rust suite at `e3907912`;
it is not a native UI recording workflow. The same full-width tooltip pattern also
prevented screenshot-editor Reset Zoom clicks. A pointer test verifies both edges
of its 56px control and passed in that Windows suite.
[Windows CI at `baa80bc0`](https://github.com/Porabuild/Poratake/actions/runs/43458601974)
passed the native Rust suite but stopped before the picker and visual matrix: the
runner terminal covered the reopened screenshot editor during its pixel capture.
The harness now waits for the editor to be foregrounded, hit-tested and painted.
[Windows CI at `bae84b8f`](https://github.com/Porabuild/Poratake/actions/runs/43463210217)
passed screenshot reopen and verified that the recording selection hole is cleared
when the same native overlay window is parked and reused. A UI trim was saved, but
the after-image check missed the selected clip's changed color, so export and picker
checks did not run. GitHub cancelled the macOS job before it started because no
hosted runner was available.
[Windows CI at `e76c8e8e`](https://github.com/Porabuild/Poratake/actions/runs/43467897146)
passed screenshot reopen and the native recording overlay reuse check, trimmed a
recorded clip through the video editor and verified the decoded export's duration,
marker pixels and tooltip exclusion. The cursor picker opened, cancelled and
reopened; its acceptance check stopped because UI Automation exposed nine edit
controls without the expected name. Native file acceptance still needs verification.
The macOS build and test job passed in that run; real macOS screen-capture exclusion
remains unverified.
[Windows CI at `847ab547`](https://github.com/Porabuild/Poratake/actions/runs/43473164699)
passed screenshot reopen and overlay reuse, but did not reach the picker: the
Export tab was not active when its button was queried. The harness now waits for
and clicks the actual sidebar tab before checking Export Video. The macOS CI
build and test job passed on that run.
[Windows CI at `e3907912`](https://github.com/Porabuild/Poratake/actions/runs/43477996629)
passed the tab, trim and decoded export checks. Picker acceptance again stopped;
the captured UI Automation inventory showed file-list properties and a search box,
but no filename Edit. The dialog did expose the isolated recording directory's
file items, so acceptance must use a verified item rather than an arbitrary edit.
[Windows CI at `02d33a22`](https://github.com/Porabuild/Poratake/actions/runs/43483480175)
found the unique fixture and state-file items, but the helper waited for an enabled
Open button before selecting the fixture and stopped. The captured evidence did not
establish whether the button was absent or disabled. GitHub cancelled the macOS job
before starting because a hosted runner was unavailable.
[Windows CI at `0a748514`](https://github.com/Porabuild/Poratake/actions/runs/43488090394)
selected the unique fixture item and verified the selection, but the dialog exposed
no Open Button or SplitButton to UI Automation. The macOS CI build and test job passed.
[Windows CI at `ea0604e7`](https://github.com/Porabuild/Poratake/actions/runs/43492688395)
passed the native workflow, including cursor-picker cancel, reopen and acceptance
of the selected fixture with an exact sidecar hash. Its visual matrix failed all
16 profiles on 292 images changed by the Reset Zoom and video drawing button fixes.
Those Microsoft Basic captures were reviewed and copied to the matching baselines;
all 2,912 images match those references offline. A hosted rerun is still needed,
and the affected NVIDIA references have not been regenerated or verified.
[Windows CI at `d3e7ad25`](https://github.com/Porabuild/Poratake/actions/runs/43504165738)
found no healthy audio render endpoints. It created the area recording project
in its isolated directory, captured the fiducial and trimmed the recorded clip,
but stopped before the preview comparison and visual matrix: 591 of 6,912 exported
fiducial pixels exceeded the original 48-level tolerance. Of those, 589 were
within one pixel of a tile edge. The revised check retains a whole-region mean
limit and compares tile interiors, with shifted and rotated negative controls.
System-audio capture remains unverified.
[Windows CI at `aef5a9f3`](https://github.com/Porabuild/Poratake/actions/runs/43508581901)
passed the native workflow, including the sampled initial-frame comparison
between the editor preview and decoded export, and all 16 visual profiles with
2,912 exact baseline comparisons. All active jobs passed; Windows packaging was
skipped. Its sampled region is static and does not prove preview updates after
seeking.
[Windows CI at `1e1c1b30`](https://github.com/Porabuild/Poratake/actions/runs/43516523003)
passed that later native check: the editor sought past the moving marker and its
preview marker samples matched the decoded export while rejecting the initial
frame. This checks a sampled region. Its visual matrix reported 32 expected
export-label differences across 16 profiles; those Microsoft Basic captures
were reviewed and all 2,912 images matched their updated references offline.
[Hosted CI at `e3e5775f`](https://github.com/Porabuild/Poratake/actions/runs/43521864369)
then passed all active jobs, including the native seek workflow and all 16
Windows visual profiles. Windows packaging was skipped. The affected NVIDIA
references are stale and have not been regenerated or verified.

The later [Windows run at `2ecfc778`](https://github.com/Porabuild/Poratake/actions/runs/43536628201)
passed the native Rust tests and saved the annotated capture, but timed out
before capturing reopened-editor readiness pixels. The initial foreground and
size checks passed; the retained evidence cannot distinguish a later foreground
change, failed center-point hit testing, or an exception before capture. The
harness now records these details on timeout.
The [next Windows run at `f29632a1`](https://github.com/Porabuild/Poratake/actions/runs/43539748747)
showed that another top-level window had foreground during the readiness timeout;
its identity was not recorded and no reopened-editor pixels were captured. The
harness now retries activation within the existing deadline and records the
competing window's identity, while retaining the hit-test and pixel checks.
The following [run at `b31ecf11`](https://github.com/Porabuild/Poratake/actions/runs/43543409414)
failed Linux and macOS GPUI lint because the new preview reveal helper and entity
import were compiled where unused. They are now gated to the platforms that use
them. Its Windows native capture, reopen, recording and audio-sidecar checks passed,
but all 16 visual cases stopped at the preview's initial animation position:
the headless window had no HWND and was removed by the new native reveal guard.
The parity workflow now treats that expected headless case separately; the strict
geometry assertion and native failure handling remain.
The [next run at `442a94f4`](https://github.com/Porabuild/Poratake/actions/runs/43548284322)
passed Linux, macOS and the native Windows workflows, but all visual cases stopped
at auto-dismiss: the headless workflow advanced the clock without delivering the
reveal frame that now starts the timer. It now settles the preview before timing;
a focused test also checks that the full dismissal delay follows reveal.
The [run at `9e284e21`](https://github.com/Porabuild/Poratake/actions/runs/43574423088)
passed 14 of 16 visual profiles; two framed-attachment cases tried to drag the
empty center of an unfilled rectangle. The fixture now drags the visible edge and
retains its exact movement assertions. The native Windows workflow passed.
The [run at `1ddf5017`](https://github.com/Porabuild/Poratake/actions/runs/43553564358)
passed Linux, macOS and the native Windows workflows. The visual cases passed the
preview timing check but later closed previews in other headless capture workflows,
which were outside the parity-only reveal guard. The guard now covers the whole
headless parity context; native windows retain their reveal checks.
[Hosted CI at `05bdabab`](https://github.com/Porabuild/Poratake/actions/runs/43556661556)
passed all active jobs, including native Windows capture and recording workflows,
the audio-only sidecar proof, and 16 visual profiles with 2,912 baseline
comparisons. Windows packaging was skipped. Verify the final PR head separately.
The documentation-only [follow-up at `d354d0b1`](https://github.com/Porabuild/Poratake/actions/runs/43559683718)
passed Linux, macOS and the other non-Windows jobs, but the Windows workflow lost
foreground during the initial reopened-editor wait, before its later recovery
loop or pixel checks. The initial wait now retries activation within its existing
deadline and records the competing window on failure.
Video export, tray file selection, screenshot wallpaper and video media imports
use asynchronous dialogs to avoid the same nested event-loop pattern. The other
native selection and cancellation flows still need verification.

Estimate with an isolated Windows desktop and an already-authorized macOS test
environment: roughly 2–5 working days, assuming no new major defects. Windows
workflow and DPI checks are expected to take 4–8 focused hours; macOS capture and
exclusion work may take 1–3 days; review and follow-up checks may take 1–2 hours.
The hosted Windows gate previously took about 50 minutes and can run in parallel.
These are planning ranges, not guarantees. Without macOS access or the necessary
existing Screen Recording authorization, the macOS gate and overall finish date
remain unknown. No test in this pass should request that authorization.

## Versions and dependency evidence

| Component          | Current source                                                                                                                                                                                          |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HeroGPUI           | Exactly 0.15.1 in `src/main/Cargo.toml`; components use the source patch below and the other three crates retain registry checksums; source tag `v0.15.1` at `09ec52be169bf25a6351a14088e34e22fe89fe9f` |
| GPUI               | `gpui-pre` and `gpui-pre-platform` exactly 0.3.11, matching HeroGPUI's workspace                                                                                                                        |
| Electron runtime   | 43.4.0 in `bun.lock`; manifest range `^43.4.0`                                                                                                                                                          |
| Electron reference | `@heroui/react` and `@heroui/styles` 3.2.4 in the installed lockfile; manifest range `^3.2.4`                                                                                                           |
| HeroGPUI reference | HeroUI 3.2.7; its component audit is separate from Poratake's app parity                                                                                                                                |
| Bundled guide      | `.agents/skills/herogpui/upstream.json` records revision, versions, source hashes and adapted local hashes                                                                                              |

[HeroGPUI #43](https://github.com/Porabuild/HeroGPUI/pull/43) is merged. The
`v0.15.1` tag points to the release source commit. Release workflow
[42747398534](https://github.com/Porabuild/HeroGPUI/actions/runs/42747398534)
completed successfully, including SemVer, platform tests, gallery artifacts and
registry publication. The public release and 0.15.1 registry packages were checked.
Poratake's published-registry update in `ac384ae` is retained as the base of this change.

The former 0.3.3 GPUI reference, old HeroGPUI revision, hand-rolled control findings
and broad surface-level “Match” claims have been removed from this audit.

The existing GPUI 0.3.11 Windows backend is patched from
`vendor/gpui-pre-windows`, with provenance and the retained Apache-2.0 license in
`THIRD_PARTY_NOTICES.md`. Its image shader, clipboard and pointer-exit handling
differ from the published archive; all dependency versions remain unchanged.

HeroGPUI components 0.15.1 resolve to `vendor/herogpui-components`. The patched
`src/select.rs` preserves focus-ring gutters and handles accessibility expansion;
`src/dropdown.rs` preserves scrollable menu focus gutters within the available
panel padding; `src/slider.rs` handles accessibility value actions; and
`src/util.rs` releases a keyboard press after focus moves into a menu. The archive
checksum, original licenses and NOTICE are retained.
Zero-padding dropdown recipes still have no scroll-end focus gutter; Poratake's
current recipes use 4px or 6px of panel padding.

## Automated visual gate

Run `bun run test:visual` on Windows. The optional `parity` feature uses an isolated
profile, a deterministic 640×360 capture and the real Windows renderer, embedded
Geist fonts and DirectWrite. Hidden native render targets provide GPU readback;
input and frame scheduling use GPUI's headless harness. This is not installed-app
or physical-monitor DPI verification.

At `bca57fb0`, a local NVIDIA dark 1280px/100% profile passed all 193
reviewed visual baselines and the capture, annotation, save/reopen and
preview/output fixture checks. [Hosted CI](https://github.com/Porabuild/Poratake/actions/runs/43338500286)
passed Linux, macOS, typecheck, lint and formatting. Its Windows probe found
the real recording tooltip with the expected text outside its owner and capture
affinity set, but the tooltip did not disappear after pointer exit; the Windows
visual matrix did not run on that commit. The [follow-up Windows probe](https://github.com/Porabuild/Poratake/actions/runs/43343136797)
passed after the pointer-exit fix, including same-point re-entry and an
overlapping window; its visual matrix also passed.

Each profile has a ten-minute watchdog; the first timeout stops the matrix and
preserves that profile's partial evidence. On Windows, process-tree termination
uses `taskkill /T` only while the tracked profile process is still active. If its
leader has exited, the runner avoids targeting a potentially reused PID and
reports that cleanup failed; this is not a Job Object guarantee, so a descendant
that outlives its leader before timeout may remain. If `taskkill` fails, the
fallback stops only the root and reports the failure. POSIX cleanup targets the
detached process group after leader exit. The persistent timeout test verifies
profile-worker cleanup and that an unrelated sibling survives; its POSIX
leader-exit case is skipped on Windows.

[Hosted CI at `515fa5cd`](https://github.com/Porabuild/Poratake/actions/runs/43286352987)
passed the complete 16-case Windows visual gate, native Windows checks and
accessibility actions. Its macOS daemon build, scroll-preview and GPUI shell
tests passed, as did Linux, typecheck, lint and formatting; the remaining Windows
tests passed too. The preceding
[hosted evidence](https://github.com/Porabuild/Poratake/actions/runs/43273775369)
and local NVIDIA runs showed 192 changes confined to the video preview. Those
reviewed images for both renderers are now committed and passed the hosted gate.
The desktop-icon restore no longer blocks the held-response fixture, and the
screenshot save, attachment, pin and editor-reopen workflows reach the baseline gate.

[Hosted CI at `98d84f77`](https://github.com/Porabuild/Poratake/actions/runs/43296056019)
passed the workflow and geometry assertions in all 16 Windows visual profiles,
then failed on the expected changed state list. Its Microsoft Basic renderer
images were compared across every profile and the changed tooltip, pressed and
video-preview states were reviewed. Final local NVIDIA captures also reached the
state-list gate for all 16 profiles. Their changed states were reviewed, and all
2,912 resulting baseline images matched the captures pixel-for-pixel. A strict
rerun against those NVIDIA references passed all 16 profiles. Windows native
and accessibility checks passed; Linux Clippy found platform-specific
tooltip warnings that a follow-up gates. In the next run Linux passed and the
macOS daemon built, but its native fixture hung during the existing scroll checks.
The follow-up keeps those checks on the synchronous main thread and bounds their
response wait. The fixture compiled and passed in
[hosted CI at `5359f58f`](https://github.com/Porabuild/Poratake/actions/runs/43307382869);
the full run passed its active jobs, including all 16 Windows visual profiles.
The [macOS fixture at `83a84d45`](https://github.com/Porabuild/Poratake/actions/runs/43315713985)
also passed the expanded scroll alignment, fixed-footer, accepted-prefix and
cancellation checks. Its Windows visual job was cancelled when the next commit
was pushed.

The GPUI recording error listener now starts before the daemon request and stays
active through a failed stop. A fake-daemon run observed a failed start, retry,
two stop attempts and the expected camera and overlay cleanup, then stopped on a
test-harness redraw race. That race is fixed in the harness; the next local run
stopped at the GPU safety limit before reaching this workflow. A CPU-only daemon
protocol smoke test passed the injected failures and retries. At
[`fe64d934`](https://github.com/Porabuild/Poratake/actions/runs/43321344519),
14 of 16 Windows visual profiles passed. The two remaining profiles reached both
stop attempts and cleanup, then failed a test assertion that searched for the
window outline only after it had already been shown during selection. The test
now checks the complete handoff; [hosted CI at `0da6a9c6`](https://github.com/Porabuild/Poratake/actions/runs/43387203877)
passed all 16 Windows visual profiles.

For translucent video wallpaper, the preview now uses the encoder's opaque
pixel conversion. A Windows MP4 test compares the decoded export with the actual
preview image at four edited times, including drawing and redaction. A GIF test
also reopens every exported frame at several frame rates and checks opaque alpha
and preview colors for transparent and translucent padding.

[Hosted CI at `f9df4d81`](https://github.com/Porabuild/Poratake/actions/runs/43252165030)
passed 4 of 16 visual cases and 706 exact baseline comparisons. Ten cases stopped
in the prewarmed tray geometry check. Both 1280px/100% cases stopped because the
live all-in-one window fixture expected frozen-frame pixels; the saved light and
dark captures contain the fixture's live color in every pixel. That expectation
and its subsequent call-order checks passed in the `904073a1` run.
The preserved evidence contains 2,926 PNGs, 16 renderer profiles and no diff
images. All other required jobs passed. A focused prewarmed-tray test reproduced
the scale reset and passed at 100%, 125%, 150% and 200% after applying the
simulated scale after each show; the revised tray checks passed in the `904073a1` run.

The Windows harness now drains the native foreground queue when each hidden
renderer closes, allowing GPUI's asynchronous window destruction to complete.
Each visual case checks that a closed test window releases its native window.
In a focused local probe, the old harness accumulated 34 windows and reached
8.9 GiB of GPU use; the updated harness stayed at three to four windows and
about 1.2 GiB while exercising the same attachment cases.

The matrix is light/dark × 950/1280 logical pixels × 100/125/150/200% scale, at
738 logical pixels high. Each case captures 163 states; the two 1280px, 100% cases
also capture 28 annotation/selection/timer/scroll/attachment/pin workflow states.
The eight 950px cases each add 27 narrow-history states, for 2,880 images per
complete renderer profile. The 72 new 140px NVIDIA history images were reviewed;
the previous 1,448 images in the 950px cases were pixel-identical. The complete
local NVIDIA gate passed all 16 cases and 2,880 exact images. The 72 hosted
variants were also reviewed; all 2,808 earlier hosted images were pixel-identical.
[Hosted CI at `0b79bd9`](https://github.com/Porabuild/Poratake/actions/runs/42992001485)
passed all required jobs and the complete 2,880-image exact matrix. The 48 selected-OCR
tooltip images and 32 target-menu images were reviewed for NVIDIA; all 2,728
earlier images were pixel-identical.
[Hosted CI at `e76df4cc`](https://github.com/Porabuild/Poratake/actions/runs/43079082274)
passed all required jobs, including 16 visual cases, 2,880 exact baselines and
the native Windows clipboard round trip. This run precedes the zoom changes
described below.
The zoom follow-up changed 148 NVIDIA images; their differences were reviewed
within the screenshot canvas or its one-pixel lower edge. The other 2,732 images
were unchanged, and all eight affected cases passed exact comparison after the
reviewed updates. The [hosted run at `e6454be4`](https://github.com/Porabuild/Poratake/actions/runs/43088770389)
reported 148 changed images across eight cases, but its evidence upload failed
after the artifact API returned HTML instead of JSON. The [run at `8b764bc0`](https://github.com/Porabuild/Poratake/actions/runs/43098740249)
preserved all 16 renderer profiles and the 148 changed Basic Render Driver
images; the other 2,732 images matched exactly. The changed pixels were checked
against the fixture image bounds and representative controls were inspected in
both themes. [Hosted CI at `2d50016b`](https://github.com/Porabuild/Poratake/actions/runs/43107376955)
passed all required jobs and all 16 visual cases with 2,880 exact baselines. Its
evidence includes 16 renderer profiles and 3,373 PNGs, with no diff images.
The [run at `64317044`](https://github.com/Porabuild/Poratake/actions/runs/43123318593)
also passed all required jobs and the 2,880-image visual matrix, before the
wallpaper-redaction change.
The [run at `fb13d389`](https://github.com/Porabuild/Poratake/actions/runs/43128687500)
passed 14 of 16 visual cases. A transparent wallpaper exposed the editor stage's
shadow in its preview, and a fractional-position test compared GPU-rendered pixels
directly with raw PNG pixels. The stage now has the editor's background behind
its wallpaper, and the fractional test compares a reopened PNG at the same bounds.
The follow-up also adds alpha-gradient and transparent-image cases.
The [run at `642d16a7`](https://github.com/Porabuild/Poratake/actions/runs/43132888460)
passed the Windows native, accessibility and clipboard checks and 14 of 16 visual
cases. Both 1280px/100% themes passed the alpha wallpaper checks, then failed the
same zoomed blackout-edge comparison.
Its preserved artifact contains 16 renderer profiles and 3,193 PNGs with no diff
images. After aligning blackout patches, local NVIDIA runs passed the light/dark
1280px/100% workflows and 191 baselines per theme, plus 190 baselines per theme
at 950px/125%. The [run at `9a0eb806`](https://github.com/Porabuild/Poratake/actions/runs/43136845938)
passed the zoom checks but failed two history-reopen baselines: each changed two
green-channel values by one at the same pen endpoint. The same pixels appeared
in the earlier hosted artifact; both baselines were reviewed and updated.
Fourteen of 16 visual cases passed, and the artifact retains 16 renderer profiles,
3,381 PNGs and both diff images. The Windows test step did not run after the
visual failure. Linux, macOS, lint, formatting and typecheck passed.
The [run at `8f6b7169`](https://github.com/Porabuild/Poratake/actions/runs/43141680459)
passed all required jobs, including 16 visual cases and 2,880 exact baselines,
Windows native and accessibility tests, clipboard roundtrip and the project test
suite. Its artifact contains 16 renderer profiles and 3,381 PNGs with no diffs;
the artifact and Windows log are retained under `.scratch-parity/hosted-8f6b7169`.
[Hosted CI at `2d26dab5`](https://github.com/Porabuild/Poratake/actions/runs/43148945257)
also passed all required jobs, including all 16 visual cases and 2,880 exact
baselines. Its artifact contains 3,393 PNGs with no diff files; the evidence is
retained under `.scratch-parity/hosted-2d26dab5`, alongside the Windows log.
[Hosted CI at `1e5d547c`](https://github.com/Porabuild/Poratake/actions/runs/43157671728)
passed all required jobs, including 16 visual cases and 2,880 exact baselines.
Its preserved artifact contains 3,395 PNGs and no diff files; the artifact and
Windows log are retained under `.scratch-parity/hosted-1e5d547c`.
[Hosted CI at `b2d00217`](https://github.com/Porabuild/Poratake/actions/runs/43165588606)
passed all required jobs, including 16 visual cases and 2,880 exact baselines.
Its preserved artifact contains 3,397 PNGs and 16 renderer profiles with no
diff files; the artifact and log are retained under `.scratch-parity/hosted-b2d00217`.
[Hosted CI at `6eb45d57`](https://github.com/Porabuild/Poratake/actions/runs/43182302140)
passed all required jobs, including 16 visual cases and 2,880 exact baselines.
Its preserved artifact contains 3,409 PNGs and 16 renderer profiles with no diff
files. It predates the selected-outline follow-ups below.
[Hosted CI at `27798e87`](https://github.com/Porabuild/Poratake/actions/runs/43189153989)
passed all required jobs and 2,880 exact comparisons across 16 visual cases. Its
preserved artifact contains 3,423 PNGs and 16 renderer profiles with no diff
files. It predates the subsequent redaction and circle-selection checks.
The desktop-icon transition follow-up passed 191 exact baselines in each local
light/dark 1280px/100% NVIDIA run. Its fake-daemon checks cover failed and delayed
icon transitions, stale direct capture errors and attachment recovery; the dark
run also holds a scroll finish for 31 seconds. The full GPUI suite passed 939
tests with one ignored, and the project suite passed 2,281 tests with three skipped.
The frozen-area icon follow-up also passed 191 exact baselines in each local
light/dark 1280px/100% NVIDIA run. Its full GPUI suite passed 938 tests with one
ignored, and the project suite passed 2,281 tests with three skipped.
The multi-display icon follow-up passed 191 exact baselines in each local
light/dark 1280px/100% NVIDIA run, including a simulated two-target workflow.
Its full GPUI suite passed 938 tests with one ignored; the project suite passed
2,281 tests with three skipped.
The scroll-start follow-up passed 191 exact baselines in each local light/dark
1280px/100% NVIDIA run with no diff images. Its GPUI suite passed 939 tests with
one ignored; the project suite passed 2,281 tests with three skipped using four
workers after the default run timed out while inventorying installed licenses.
The earlier frame and aspect-ratio follow-up passed 191 exact baselines in
each local light/dark 1280px/100% NVIDIA run, including the new workflow checks.
The earlier scroll follow-up passed 191 exact baselines in each of the isolated
1280px/100% light and dark NVIDIA cases, with no diff images. The runs peaked at
4,070 MiB and 4,065 MiB of total GPU memory respectively.
An exact rerun after the target-menu interaction fixes passed all 16 cases and
2,808 images. The corresponding 80 hosted images were reviewed, and all 2,728
earlier hosted images were pixel-identical. [Hosted CI at `a9d025f`](https://github.com/Porabuild/Poratake/actions/runs/42980916944)
passed all required jobs, including the 2,808-image Windows visual gate. The 96 earlier NVIDIA all-in-one tooltip images were
reviewed; all 2,632 existing images were unchanged. The corresponding 96 hosted
images were also reviewed, and all 2,632 previous hosted images were pixel-identical. The local
exact-baseline matrix at `80a5700` passed all 16 cases and 2,632 images. After
the color-capture failure fix, the two
1280px/100% workflow cases passed again in light and dark themes. The 96 new
NVIDIA color-picker images were reviewed; all 2,536 existing images were
unchanged. The 96 hosted color-picker images were also reviewed, and the 2,536
previous hosted images were unchanged. The 144 new NVIDIA history images were reviewed. The earlier matrix's 64 corner
states and 160 revised selection images were reviewed; the remaining 2,312 existing
images were unchanged. The DPI follow-up changed 24 NVIDIA history images; all
24 were reviewed and the other 2,512 images were unchanged. The final exact
matrix passed all 16 cases. The latest local Windows native gate passed 1,084 tests with two skipped.
All 144 hosted history images, 64 selection-corner images, 160 revised selection
images and 24 DPI-adjusted history images were also reviewed. The other 2,512
hosted images were unchanged.
At 950 logical pixels and 125%, the synthetic picker backdrop spans 1187.5
physical pixels, leaving a pale final screenshot column. The real color-picker
overlay is transparent; this fixture artifact does not verify physical displays.
Loaded-video windows respect their 1200×750 minimum: the 950px cases use 1200px
for that surface. Recording controls use their measured content width.

| Surface             | Exercised evidence                                                                                                                          | Limits                                                                                            |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Screenshot editor   | Default, representative hover/held press/keyboard focus, selected tools, thickness/color/tool panels and keyboard-operated nested menus     | Not every control's complete interaction matrix                                                   |
| Area overlays       | Screenshot, OCR, QR, timer, scroll, recording and editor-attach default/drag-selection surfaces; standalone window/display picker states    | No OCR/QR service result or stitched-scroll output from these fixtures                            |
| Color picker        | Center, four display corners and 400×300 narrow window; exact sampled loupe pixels and pointer clearance                                    | Physical pointer transitions and native clipboard delivery remain unverified                      |
| All-in-one          | Screenshot/record area, window and display pickers; OCR area, window highlights, pointer leave and normal/narrow tooltips                   | Window ownership is simulated; physical pointer transitions remain unverified                     |
| Recording controls  | Area/window defaults, hover/held press/keyboard focus, long device menus, countdown/cancel, active/paused/muted and locked camera choices   | Fixture RPCs assume granted media permission; hardware and real consent remain native checks      |
| Video editor        | Empty editor states plus a loaded encoded project, drawing, wallpaper and all ten side panels                                               | Playback and every panel's control states are not yet image baselines                             |
| Annotation workflow | Annotated preview, history reopen and flattened PNG reopen                                                                                  | Built-in fixture at 1280px/100%; custom comparison images skip this workflow                      |
| Pin window          | Default, hover, enlarged and narrow windows; image bounds and close control; annotated editor restoration with history enabled and disabled | Native movement and Alt+F4 restoration verified on Windows at 100%; mixed DPI remains unverified  |
| Capture preview     | Single cards and four-card stacks; four corners, eviction, explicit editor handoff, auto-dismiss, hover ownership and a four-display menu   | Native capture handoff verified; native hover delivery and multi-monitor menus need verification  |
| Tray menu           | Window height equals the painted menu at every simulated scale; fitted-menu image baselines                                                 | Native prewarmed client height verified at 100%; physical mixed-DPI transitions remain unverified |

Geometry assertions check 21 toolbar entries, 28/32px control sizes, spacing,
nonoverlapping options, visible caption buttons and the ability to scroll overflow
controls into view. Caption widths follow GPUI's device-pixel rounding. Selection
edges allow at most half a physical pixel of snapping. Menu actions assert selected
tools and open panels before their screenshots are named. Recording-bar children
must fit their viewport without overlapping. The recorder fixture asserts one
start/pause/resume/stop call, enabled inputs at start, disabled live toggles and
unchanged saved preferences. Discard must remove the sole project inside the
isolated recordings directory.
At each simulated scale, an unfrozen screenshot fixture drags from device-pixel
coordinates on a negative-origin display to its right and bottom edges. It checks
the painted selection, exact daemon request, saved image dimensions and four
preview pixels against the saved four-color synthetic image. The fixture does not
capture physical monitor pixels. Temporarily disabling edge snapping made its
125% frame check fail.
Four additional small selections at the display corners check the size label's
painted bounds and clipping mask. Bottom-edge labels must also clear the selection.
Unit coverage checks left, center and right positions, pointer routing through
the label, resizing from the outside outline and simulated 100%, 125%, 150%, 175% and 200%
scaling. Pixel checks scan the dimmed scrim at the corners and across
fractional-position selections at every matrix scale, rejecting both undimmed
gaps and double-dimmed overlaps while
checking that selected pixels remain undimmed. Samples on all four sides verify
the surrounding dim, the undimmed interior and the outside outline.
Picker fixtures place ordered overlapping windows on a negative-origin display
in correctly scaled physical coordinates. Pixel samples independently check the
undimmed candidate and 50% scrim, including targets crossing left, right and bottom edges
and restoration of dimming on leave. The headless platform always reports a
non-hovered window, so the fixture supplies window ownership to the shared
production surface; production rendering still reads the native hover state.
An additional picker scan checks every pixel across the lower viewport. The
recording handoff test checks every physical pixel of its painted scrim and clip
at 100%, 125%, 150%, 175% and 200%, with centered and edge selections. It requires one
dim layer outside the selection and none inside.
The camera, microphone, system-audio and iPhone/iPad icons must each provide
enough pixels with at least 3:1 contrast against the toolbar. Their actual
canvas bounds are measured independently of the dropdown chevrons and checked
against their containing buttons. On macOS, the isolated recording-control
fixture passed in light and dark themes at simulated 100%, 125% and 200% scale.
The 125% renderer captures show the [dark](tests/native/evidence/macos-ui/recording-control-glyphs-dark-125-20261117.png)
and [light](tests/native/evidence/macos-ui/recording-control-glyphs-light-125-20261117.png)
bars. The former light-theme white microphone failed with zero contrasting
pixels. These offscreen images do not verify the floating window's macOS
compositor border or real capture. Media permission
checks return granted only in a parity build running the daemon fixture, before
reading host consent or opening a permission dialog.
Held-button capture advances the mock clock before reading pixels and must
differ from hover inside the measured microphone button bounds.
The font-size submenu samples the keyboard focus ring on all four sides of the
current, first and last options, independently of baselines, and preserves each
as `submenu-text-focus-*.png`. The original virtual viewport failed with zero
focus pixels on the selected option's left edge. The shared Select fix passed
that probe while preserving its 24px rows and 64×276px light-theme panel.
All-in-one Capture text, Pick color and Close tooltips must fit below their
controls, within the viewport and their clipping masks, in both normal and
400×300 overlays and while Capture text is selected. The original top placement
started 5px above the display. An additional 175% tooltip and target-menu probe
passed in both themes; it has no image baseline. The narrow target menu must remain
anchored below its trigger and inside its clipping mask. Synthetic keyboard input
checks target selection, Escape dismissal and reopening from the focused trigger.
Capture-preview hover ownership and keyboard dismissal are checked in all four
corners. Windows and macOS use native hover tooltips, which are outside GPUI's
headless screenshots; Linux keeps the in-window HeroGPUI tooltip. Keyboard focus
labels and Linux preset names wrap inside the card, with long visual labels
ellipsized while the full name remains available to accessibility tools. The
fixture checks that Edit has no tooltip and remains clickable. Video Copy and
Show in Folder hover images use a missing-media fallback, not a decoded recording.

Baseline comparison requires identical image dimensions, state filenames and every
pixel. A changed pixel fails; failed images get a highlighted diff. Updates require
`bun run test:visual --update-baselines`, which records replacements and removes
obsolete `gpui-*.png` states. Review images and geometry before committing updates.
CI runs the visual gate in the Windows native job and preserves PNG evidence
and GPU/driver metadata. Exact images are selected by GPUI's software/hardware
flag and device name, under each matrix case's renderer directory. Missing
profiles fail; no tolerance or fallback accepts another renderer's output.
Loaded-video captures use one pinned synthetic H.264 checker fixture rather than
re-encoding their input on each host. Separate workflow tests still encode/export.

The first [hosted Windows run](https://github.com/Porabuild/Poratake/actions/runs/42779094342/job/129704528224)
at `db35386` passed native checks and every visual case's geometry/workflow
assertions, then failed all 16 exact image comparisons against the local GPU's
images. Many differences were one or two channel levels; some UI edges and video
preview pixels differed more. Renderer-specific reviewed baselines and the fixed
video input address the differing capture environments without weakening the
pixel check. Subsequent hosted capture and comparison status are recorded below.

The next [hosted Windows run](https://github.com/Porabuild/Poratake/actions/runs/42786386596/job/129728542684)
at `2cfb70e` passed 957 native tests with one skip and all 16 cases' geometry and
workflow assertions. Exact comparisons failed because the reported
`hardware-microsoft-basic-render-driver` profile had no reviewed baseline.
Review of all 1,928 captures found two image-edge changes against the preceding
hosted run; the other 1,686 shared non-video states were byte-identical.
Those edge changes exposed actual atlas bleed, so these images were not accepted
as baselines. A new GPU probe reproduced it locally: source `[240, 80, 64]`
became `[245, 142, 132]` near an image edge. Clamping samples to each tile's texel
centers passed the same probe and the capture workflows. Every Windows matrix
case runs this color assertion before baseline comparison and saves
`image-atlas-sampling.png`. After the atlas and Select fixes, 340 changed local
baselines were reviewed and regenerated. A fresh run of the frozen verified
binary passed all 16 cases and 1,928 exact images with updates disabled. The Select
fix changed only the 48 keyboard submenu images; the other 1,880 captures stayed
byte-identical to the atlas-only run.

The next [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42799398288/job/129772749091)
at `dd6b68e` passed 960 native tests with one skip, plus every case's atlas,
focus-ring, geometry and workflow checks. Exact comparison again rejected the
missing hosted profile. All 1,928 captures were reviewed; the 340 changed images
and 47 changed states versus the rejected run match the local shader/Select
change set. The other 1,588 images stayed byte-identical. Those reviewed captures
now establish `hardware-microsoft-basic-render-driver` with driver
`10.0.26100.33648`. Artifact `13758275750` has SHA-256
`3caeda627db64c2dc92184f09f61042cf6ed24ff2ea78e71f0db188d1467eebf`.
The next [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42806652330/job/129797393662)
at `abc940f` passed 961 native tests with one skip and all 16 cases’ geometry,
workflow and atlas checks. It rejected the incomplete state list as intended.
Its 176 new tray/tooltip/Edit-hover images were reviewed, and all 1,928 earlier
hosted images remained byte-identical. That 2,104-image profile was complete at
`460ed6f`. The artifact SHA-256 is
`53a763a3afcd985d4cba1c731526a88896a6f10ca5b480bc86e9793048e1e42a`.

The next [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42813783806)
at `460ed6f` passed all jobs. Windows passed 963 native tests with one skip and
all 16 exact visual cases against the 2,104-image Microsoft Basic Render Driver
profile.

The [next hosted run](https://github.com/Porabuild/Poratake/actions/runs/42819345589)
at `d83494f` passed 965 native tests with one skip and reached the visual
state-list assertion in all 16 cases. The gate rejected the 32 new edge images
because they were not yet in the hosted profile. All 2,104 existing images are
byte-identical to their baselines; the 32 new default and hover images were
reviewed across themes, widths and scales. Artifact `13768140621` has SHA-256
`3365f2021acf721a6425bc179dfbb78e34dde355b84e9e92eb29001446233bdf`.
The next [hosted visual run](https://github.com/Porabuild/Poratake/actions/runs/42823276665)
at `b091e35` passed all 16 cases against the 2,136-image profile without baseline
updates. Artifact `13769796702` has SHA-256
`19cc87f9786816d08d3b7a4c10f1c6238a05b121a11ae4be196eb2e5cd18d98c`.
The [hosted run at `0c5e134`](https://github.com/Porabuild/Poratake/actions/runs/42827223536)
also passed the required jobs, including 971 Windows native tests with one skip and the
same 16 visual cases. Artifact `13770638988` has SHA-256
`dd121943c27275e18143be5a837a289f94ab52791226fc43aee3cc960eeb0d52`.
The [hosted run at `da95f73`](https://github.com/Porabuild/Poratake/actions/runs/42832652057)
passed the required jobs, including 972 Windows native tests with one skip and all
16 visual cases against 2,136 exact images. Its visual artifact `13774605959` has SHA-256
`4ee4a1c8778869a8527cc2784c354fe0b986cf9314a7c2806ae7d59a6b3237ed`. Linux also
passed the unevenly lit QR photograph regression.
The [hosted run at `9407fa6`](https://github.com/Porabuild/Poratake/actions/runs/42845902225)
passed the required Windows, Linux and macOS jobs, including the Windows visual
comparison.
The [hosted run at `9669cde`](https://github.com/Porabuild/Poratake/actions/runs/42851318895)
also passed the required jobs, including the scaled selection and preview fixture.
The [hosted run at `372db36`](https://github.com/Porabuild/Poratake/actions/runs/42862315214)
passed all required jobs, including the Windows visual comparison.
The [hosted run at `a0168f3`](https://github.com/Porabuild/Poratake/actions/runs/42871830573)
also passed all required jobs, including Windows, Linux and macOS tests.
The [run at `831c9de`](https://github.com/Porabuild/Poratake/actions/runs/42876204997)
passed Windows native tests and the visual geometry checks, then rejected the
16 new canceling-state images absent from the hosted profile. All 2,136 existing
hosted images were byte-identical; the 16 new images were reviewed and added.
GitHub reports artifact `13791799183` with SHA-256
`9a5748d75fd3255d023979cd25b9a30b6ec95ca97cd867d439dff8eaba5ff356`.
The [run at `a16a0a7`](https://github.com/Porabuild/Poratake/actions/runs/42879631430)
passed all required jobs, including all 16 Windows visual cases and the normal
2,152-image hosted comparison.
The [run at `72f0f59`](https://github.com/Porabuild/Poratake/actions/runs/42913126458)
also passed all required jobs and the 2,152-image hosted comparison. The next
hosted run captured 80 new tooltip images; they were reviewed and added, while
the previous 2,152 images remained byte-identical. Its Linux X11 test failed on
daemon exit once and passed on retry; the cause of the first exit is unverified.
The [run at `090954e`](https://github.com/Porabuild/Poratake/actions/runs/42968353563)
also failed its scaled GTK X11 test after the print-dialog workflow completed,
while waiting for daemon exit. Its exit status was not reported; the test now
prints it if the failure recurs. An isolated GTK print-dialog probe exited cleanly
300 times, but did not exercise the daemon and did not establish the cause.
The same run's Windows job passed its native tests and all visual geometry and workflow checks,
then rejected the 96 new color-picker images absent from its baseline. All 96
were reviewed; the other 2,536 images were unchanged. Artifact `13834135629` has
SHA-256 `5d8e382bbbc961eb0f2a6d463db48d7ff5e77f496e1c7d6f4afe188f9ebd1a21`.
The Linux job in the [next run](https://github.com/Porabuild/Poratake/actions/runs/42972460141)
passed both X11 tests, including scaled GTK; the earlier exit remains unexplained.
Its Windows native and geometry checks passed, then the visual gate rejected the
96 new all-in-one tooltip states. All were reviewed; the previous 2,632 hosted
images were pixel-identical. Artifact `13835681057` has SHA-256
`69194ad53c1987a87bc5af0710f22a33f3929226c1fe2888a068176adac98d74`.
The [run at `bf4ac143`](https://github.com/Porabuild/Poratake/actions/runs/42977256566)
passed Linux, macOS, Windows native and geometry checks, then rejected 80 new
selected-OCR and target-menu images. All 80 were reviewed; the previous 2,728
hosted images were pixel-identical. Artifact `13837611366` has SHA-256
`08b97e272ba8d5a2c60a905936f643e8847b3185a908bd027d6f87da5bc34b5a`.

The earlier local exact comparison passed all 16 cases. Contact sheets for all cases and
full-resolution narrow menus were reviewed. An isolated baseline with one changed
pixel failed with exit 101 and produced a diff image.

## Workflow and regression evidence

The commands in `bun run checks` passed against the published HeroGPUI 0.15.1
base: 180 Vitest files, 2,224 tests passed and three skipped; 956 native tests
passed and one skipped. Typecheck, lint, format, React Compiler, Rust Clippy and
license/advisory checks passed. After the composition-fit change, typecheck,
lint, format, the native gates and parity-feature Clippy passed again.
Render-unit-test state could lazily start the ambient native daemon, leaving
deferred capture work running after a test. Unit fixtures now inject an empty
daemon path. A controlled fake ambient daemon reproduced the failure before the
fix and was never started afterward. The native suite then passed 957 tests and
one skip without leak warnings; 40 repetitions of the previously flagged tests
also passed without leaks. Parity builds retain their explicit fake RPC client.
The pin close-callback fix subsequently passed the native gate with 959 tests
and one skip. Its regression first found no restored editor; after the fix it
verified the original file, annotations and exact exported PNG with history
disabled. A standalone pin closes without opening an editor. These tests invoke
the platform should-close callback and dispose the test window separately;
native OS event delivery was subsequently verified in an isolated development instance.
Pin windows now have an OS title so native window inspection can identify them;
the title change passed the four pin tests and parity-feature Clippy.
Native foreground input added a rectangle to an owned checker fixture, pinned
it, moved the pin and sent Alt+F4. The pin HWND disappeared and a new editor
retained the rectangle. A real screen capture opened only a bottom-right preview;
double-clicking that preview opened its saved image in the editor. The older
development editor remained open. A separate preview on the earlier inward-tooltip build showed the complete
Close tooltip inside the native card.

A native attachment captured darkened pixels because the editor was still
animating out when the frozen frame was acquired. A distinct red/yellow editor
fixture reproduced its translucent image over the underlying desktop. Windows
now disables that window transition and flushes the compositor after hiding.
With a separate blue/gray fixture underneath, the fixed 200×120 captured layer
matched all 24,000 reference pixels exactly. Its full composition was visible,
and the saved 1240×360 PNG retained the original 640×360 pixels exactly; the
enlarged attachment differed from bilinear reference by at most one channel
level. The isolated test windows were closed and the older editor remained open.
The real native handoff is not yet automated in the hosted harness; the headless
fixture does not test Windows compositor transitions. Evidence is in
`.scratch-parity/native-handoff-green-9dfaa280082844d9bba4b43f417955c0/`.

The tray sizing regression reproduced a 632px viewport around a 564px painted
menu. Measured child height now sizes the window; all four simulated scales fit.
A native test using a synthetic tray rectangle and the original shortcut labels
placed the 564px menu 8px above the taskbar work area. The older development
instance still uses its earlier binary; it was kept open at the user’s request.
Geometry tests also check that changing the measured height preserves the
taskbar anchor and that top/side placement stays inside the work area.
Windows monitor lookup now uses GPUI's selected monitor identity instead of
passing a logical display center to a physical-coordinate API. A regression first
showed an invalid identity selecting a nearby monitor; it now rejects that
identity and matches Windows' monitor and work rectangles for a valid one. The
physical mixed-DPI and monitor-transition checks remain outstanding.
A tooltip regression reproduced a 94x32px label above and left of the preview.
The initial fix used inward HeroGPUI placements; Edit has no tooltip. Hover labels
now use native tooltips on Windows and macOS, while keyboard focus labels and
Linux retain HeroGPUI tooltips.
The tooltip-presence guard also rejects a build with tooltips disabled. A preset
name later produced a 254px Polish tooltip inside a 200px card; it now wraps
within the card and limits very long visual labels to three lines. All 80 new
local renderer images were reviewed, and the previous 2,152 stayed byte-identical.
A normal run then passed all 16 cases and 2,232 images without updating baselines.
The four-display preview menu originally painted 220×158px outside its 200×140px
card. It now fits inside the card and scrolls, stays open after pointer leave,
and closes when its owner is removed or display topology changes. Synthetic input
checks keyboard and mouse selection, short-menu scrolling and auto-dismiss. GPU
samples check all four sides of the first and last focus rings at every simulated
scale and corner. The 96 local menu images were reviewed; the previous 2,232 local
images were byte-identical. A normal run passed all 16 cases and 2,328 images
without updating baselines. The Windows native gate passed 1,006 tests with one
skip, and 74 vendored dropdown tests passed. Physical multi-monitor behavior
remains unverified.
Review of the hosted menu images then exposed a clipped label when keyboard focus
moved to the last display. A shorter menu initially exposed parts of the preview
controls beneath it. The menu now fits three complete rows and hides those controls
while open. Pixel checks reproduced both defects at 100% before the fixes; the
outside-click test also checks clipboard preservation and restored controls. All
16 local matrix cases passed, and the 96 revised menu images were reviewed; the
other 2,232 images remained byte-identical. The [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42935006992)
reached the state-list check in all 16 cases. Its 96 new menu images were reviewed,
and its 2,232 existing images were byte-identical. Artifact `13819691455` has
SHA-256 `9514eb08c98dbb5471a375a8cc2d86473d7c736e505252098e3d74780113503b`.
That run's Linux and macOS Clippy jobs rejected unused test-handle assignments;
those assignments were subsequently corrected. The [later hosted run](https://github.com/Porabuild/Poratake/actions/runs/42938607663)
passed Linux, macOS and Windows, including all 16 cases and 2,328 exact images.
The [framed-transparency run](https://github.com/Porabuild/Poratake/actions/runs/42941558246)
also passed all required jobs and all 16 visual cases, including the new pixel
checks on the hosted renderer. The [history and frame-only run](https://github.com/Porabuild/Poratake/actions/runs/42945831848)
passed all required jobs, 1,015 native tests and all 16 visual cases. The
[later hosted history run](https://github.com/Porabuild/Poratake/actions/runs/42948956984)
passed its history geometry and deletion checks but failed the eight 950px
state-list comparisons because their new images had not yet been added. Its
2,328 existing images remained pixel-identical, and the 80 new images were
reviewed. Artifact `13826065362` has SHA-256
`5a4f1960ff24a1920489f6568c8f81170a27b2b81806ec7fd8ff1219ee8c54fd`.
The [grid-tooltip run](https://github.com/Porabuild/Poratake/actions/runs/42952945962)
passed the same geometry checks and eight 1280px cases; the eight 950px state
lists lacked the 64 new images. All 2,408 existing hosted images were pixel-identical,
and the 64 new images were reviewed. Artifact `13827306848` has SHA-256
`d9cbd127c06a8e3ea96f4c020fdadbc77d6760b8f32f49ece77acbd48214c90c`.
The [selection run](https://github.com/Porabuild/Poratake/actions/runs/42960670267)
passed the geometry and workflow checks but failed the 16 state-list comparisons
because 64 new corner images were absent. Those images and the 160 changed
selection images were reviewed; the other 2,312 existing images were unchanged.
Artifact `13829827867` has SHA-256
`e8ed36fccbec9406e86d1411a6964032d79f9a86785d28c09aad9c2bd1e5598e`.
The [DPI follow-up run](https://github.com/Porabuild/Poratake/actions/runs/42963926798)
passed the geometry and workflow checks and 14 visual cases. The two narrow
125% cases differed in 24 history-grid images, all reviewed; the other 2,512
hosted images were unchanged. Artifact `13832141444` has SHA-256
`d61c324f6de4a9030ecf81bed2977ed43963a693bfa22ad4fb6fa72e349eea6d`.
The [140px history run](https://github.com/Porabuild/Poratake/actions/runs/42986753020)
passed the geometry and workflow checks and eight wider visual cases. The eight
narrow cases rejected their 72 missing states; those images were reviewed and
all 2,808 existing images were pixel-identical. Artifact `13841822779` has SHA-256
`0bdfea91ec8148b6390a7d7aead6dff99b853ce1f8374c88d66a7cd3b99049a6`.
Check the current PR for subsequent changes.
The display-change test checks preview repositioning in a headless window, and
synthetic geometry covers negative origins and mixed scaling. Physical monitor
changes and destination-DPI movement remain unverified.
The pooled-overlay regression reproduced retained selection and drag state, which
are now cleared on park. The editor-size regression found no stored initial
size; successful opening now remembers it before another image can choose a
different size. Reopening small/large images and a resized editor are exercised.
The expanded local matrix recorded all 16 cases and 2,104 images. All 176 new
images were reviewed, and the existing 1,928 local images stayed byte-identical.
A fresh normal matrix passed all 16 cases and 2,104 exact images with baseline
updates disabled.
The attachment-edge expansion adds 32 reviewed local images; the earlier 2,104
local images remain unchanged. A normal run passed all 16 cases and 2,136 exact
images. The four edge indicators are also checked independently of baselines.
The native gate passed 963 tests with one skip after the tray anchor tests and
compositor fix; typecheck, lint, formatting and parity-feature Clippy also passed.
With the attachment-edge interaction and focus tests, it passed 965 tests with
one skip; typecheck, lint, formatting and parity-feature Clippy also passed.
After the recording-preference and toolbar-hit fixes, the local matrix again
passed all 16 cases and the native gate passed 967 tests with one skip.
An additional Windows encode/decode test checks cursor, keyboard shortcuts and
subtitles before and after a cut at 1.5× and 0.5× speeds. It checks which keys
and captions are active, proves each layer changes the preview, and compares
the affected pixels in the export with the preview. The native gate passes 968
tests with one skip after this addition.
An edited GIF workflow now checks saved and reopened state, preview pixels and
every decoded frame boundary at all six offered frame rates. Before the timing
fix, a two-second export played for 1.8 seconds at 30 FPS and 1.92 at 24 FPS;
afterward each frame boundary stays within 5 ms of the edited timeline.
Windows export tests cancel MP4 and GIF at an early frame and at the last frame,
then retry the same path and decode the result. A valid editor project also checks
that retry is blocked until cancellation finishes and succeeds afterward, with no
cancel notification and one success notification. The test passed ten isolated
runs; the native gate passed 992 tests with one skip. The canceling label and its
footer are checked against their actual bounds at every matrix scale. Sixteen new
local canceling-state images were reviewed, and the normal matrix passed all 16
cases and 2,152 exact images without updating baselines. The hosted renderer's new
images were also captured and reviewed. Moving the label outside the footer made
the geometry assertion fail; the restored layout passed. The video editor fixture
uses a 1,200px minimum width even when the matrix case is 950px.
Isolated Linux encoder tests exercise blocked input, cancellation during mux and
staged-file cleanup. A simulated macOS backend on Linux exercises blocked-input
cleanup; neither fixture establishes actual macOS or Linux app behavior.
The audio check compares preview stems and a decoded AAC export across a cut and
speed changes. It verifies 440 Hz and 880 Hz source tones, two short sounds near
their edited positions, two keyboard clicks after raising their volume from zero,
and the full two-second output duration. Signal tests cover stereo tones from
50 Hz to 12 kHz at the non-unit speed presets from 0.5× to 4×, nine short sounds
within 40 ms of their expected positions, and sounds at the end of input and before
silence. A file-only synthesized speech sample was also run through the production
stretch algorithm and Electron's FFmpeg filter at the non-unit speed presets; a
coarse windowed comparison found no silent windows in either output. This was not
a listening test or a full editor/export test. These are synthetic fixtures, not
live speech or music. A scheduler test
covers undoing an edit while its audio rebuild is in flight. The local native gate
passes 984 tests with one skip after the audio, attachment, QR and monitor fixes.
Electron's multi-track export previously normalized the summed inputs, making
them quieter than the independently mixed preview and GPUI export. Its FFmpeg
mix now retains the source levels. A headless 440/880 Hz regression checks overlap,
mute and track-end behavior when FFmpeg is available; a local AAC encode/decode
probe also preserved both tones. Overloaded mixes and live system-audio capture
remain unverified. A separate local headless test generated a genuine two-second
audio-only M4A with the Windows daemon audio processor and encoder, then verified sidecar selection,
both tone frequencies, duration and preview/export audio in GPUI. This does not
exercise a live audio endpoint.
A local Windows test now compares preview and decoded export across a one-frame
intro and checks the first audio sample timestamp within one AAC packet. It does
not exercise live playback or recording.
The normal local visual matrix also passed all 16 cases and 2,136 exact images
after the audio, attachment and monitor fixes, without updating baselines.
Most GPUI surfaces use HeroGPUI tooltips and shared helpers. The tray icon,
macOS scroll controls, and Windows and macOS recording and capture-preview
hover controls use native tooltips.
If a screenshot or recording preview window cannot be created, GPUI now sends
a notification that the capture was saved and leaves the editor closed. Two
Windows headless tests exercise the failed window creation and retained files;
actual operating-system notification delivery remains unverified.
The earlier camera/zoom export expansion passed the native gate with 960 tests and one
skip, plus typecheck, lint, formatting and parity-feature Clippy. The seven decoded
samples verify camera visibility and source-frame changes across a cut; an
independent checker comparison requires the manual zoom to affect the image.
The preceding capture-preview expansion passed all 1,928 images, including the
permission isolation, icon-only contrast and capture-preview checks. The 64 added
preview images were reviewed; the preceding 1,864 images stayed byte-identical.
Moving these 1,928 images into the local NVIDIA renderer profile preserved every
hash. The fixed video input and renderer-profile lookup then passed a fresh
16-case exact comparison of all 1,928 images.
The attachment fit
assertion first failed with a 2,201.6px composition in a 1,280px viewport, then
passed after the fit cache included content dimensions. The pin's annotated
render has zero changed pixels against its saved PNG in the fixed fixture.

OCR/QR/scroll checks passed 19 production workflow scenarios and 53 Electron
reference tests in four files. Packaging/compliance checks passed 15 tests.
The full-window QR regression failed before the fix and passed afterward. Against
the original payload files, the local-threshold fix improved exact native matches
from 42/65 to 48/65; a global-threshold fallback raised this to 50/65. Additional
local threshold scales raised it to 59/65. Regional and additional threshold
retries raised it to 65/65 without losing a previous success. The
earlier 38/65 and 43/65 counts normalized line
endings on only the expected side and undercounted CRLF payloads. The single-pixel
baseline failure regression also passed separately. The Windows native gate passed
980 tests with one skip after the earlier QR threshold regressions.

A low-contrast photographed code failed before the guarded normalization retry.
Against 520 original and brightness/contrast variants of 40 existing photographs,
the shared decoder improved from 367 exact matches to 493, with no prior match lost.
The Windows regression also covers quarter-turns and a valid code beside a damaged
candidate. These generated variants are development evidence, not a CI fixture.
The Linux QR loader passed eight isolated tests against the same shared decoder.
For a synthetic low-contrast 4K UI without a code, an alternating-order run with
two warmups and eight measured samples had a median of 1.32 seconds before the
retry and 2.04 seconds with the guard. The guard avoids the final retry when it
finds no candidate, but its threshold scans still add latency. The normalization
buffer can reach 64 MiB at the supported image limit.
The shared decoder now computes the same clamped 5×5 black-point averages with
rolling sums. The direct-average regression covers grids from 5×5 to 17×17;
all 520 brightness/contrast variants produced identical payloads, with the same
493 expected matches. On this Windows machine, an alternating release-mode
benchmark with one warmup and eight samples reduced median processing of an
already-decoded 3840×2160 low-contrast screen from 991.35ms to 927.75ms, and a
4K image with an embedded code from 398.80ms to 352.95ms. These timings exclude
capture, image decoding and IPC; the rolling sums add two bytes per block column.

Hosted [Linux GPUI CI](https://github.com/Porabuild/Poratake/actions/runs/42786386596/job/129728542346)
passed at `2cfb70e`: 798 GPUI tests, 28 Linux-daemon tests including the full-window
photographed QR regression, and both X11 capture runs (normal and `GDK_SCALE=2`).
This adds Linux service/test evidence; it does not establish interactive Linux
app parity or physical monitor scaling.

Hosted [macOS tests](https://github.com/Porabuild/Poratake/actions/runs/42786386596/job/129728542646)
passed at `2cfb70e`, including 800 GPUI tests, the Swift daemon build, Rust checks
for Apple Silicon and Intel, and Clippy. Interactive macOS app parity remains
separate verification.

| Check                           | Verified result                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Annotate → save → reopen        | Real pointer events create pen, highlight, rectangle and redact annotations. Mouse release differs from the last move. Persisted history reopens identically; flattened PNG reopens without editable annotations and with identical decoded output.                                                                                                                                     |
| Screenshot preview versus PNG   | Native rendered image region versus saved output: zero pixels differ by more than two channel levels. Rectangle/circle/line/arrow previews use the export rasterizer; cache tests distinguish captures with identical annotations.                                                                                                                                                      |
| Wallpaper scaling and output    | At simulated 125% and 200%, simple framed and rounded single-image regions match the PNG within two channel levels; translucent custom backgrounds can differ by three. Frame styles, shadows, corner annotations and selection/crop handles are checked in their supported combinations. Complex compositions remain outside this check.                                               |
| Edited video versus MP4         | Windows Media Foundation fixture, trim/gap removal, 2×/0.5× speed, wallpaper and drawing; project save/reload; decoded output dimensions/duration, four sampled frames, whole-frame and drawing-region error checks.                                                                                                                                                                    |
| Camera and zoom versus MP4      | Synthetic two-color camera, recorded on-periods mapped across a source cut, manual 2× checker zoom, saved-state reload and seven decoded frames. Independent visibility, source-color and zoom-change assertions accompany preview/export error checks.                                                                                                                                 |
| Audio preview versus MP4        | Synthetic audio and keyboard sounds are mixed in preview and compared with a decoded AAC export across a cut and speed changes. Both source pitches, two short sounds, the two-second duration and each keyboard click are checked independently.                                                                                                                                       |
| Native screenshot/editor        | Owned editor window captured through the daemon; native pointer annotation and Save Screenshot dialog produced a real PNG.                                                                                                                                                                                                                                                              |
| Native static recording         | Pause/resume recording formerly finalized at 0.0333333s; fixed daemon reports 1.6345311s, decoded stream 1.634500s, excluding the pause.                                                                                                                                                                                                                                                |
| Windows recorder status         | Seventeen CPU-only Windows recorder tests cover static-source live-duration and queued Pause/Stop. Hosted Windows polls status during a recording and decodes the stopped output; queued Pause/Stop remain unverified in that workflow.                                                                                                                                                 |
| Native changing recording       | Pointer/shape edits during recording finalized at 52.0008992s; decoded stream 52.000867s with nine change-driven frames, exercising the tracked-texture pool.                                                                                                                                                                                                                           |
| Native record → edit → export   | The static recorded project opened in the GPUI editor, accepted a wallpaper change and exported through the native Save dialog: 1052×750, 50 frames, 1.666633s at 30 FPS.                                                                                                                                                                                                               |
| Recording-control UI/RPC        | Production controls with deterministic device/recorder responses; Escape cancels countdown and leaves an active recording open; pause/resume and live input changes issue the expected requests; discard removes the fixture project.                                                                                                                                                   |
| Accessibility action dispatch   | An isolated GPUI test platform exercises the actual AccessKit dispatch path for Select expansion and Slider value actions, including disabled and range controls. It does not exercise an operating system accessibility adapter or screen reader.                                                                                                                                      |
| Loaded-video state and preview  | Production editor opens an encoded project with a drawing; wallpaper/export changes autosave and reload; the UI preview bytes equal a fresh composition of persisted state. All ten side panels have image baselines.                                                                                                                                                                   |
| Native countdown cancellation   | Accessibility tools identify the named recording-control window; Start displays a ten-second countdown and a background Escape message closes the bar before recording starts.                                                                                                                                                                                                          |
| Native light control menu       | A light-theme development window displays readable controls, enumerates the actual microphone list, opens its menu, dismisses it with Escape and closes through its accessible Close button.                                                                                                                                                                                            |
| Selection capture routing       | Production window, area and all-in-one display startup, saved pixels, history registration and editor-export equality for live, frozen, failed and unavailable-freeze paths. Physical rectangles and capture-before-release ordering are asserted. In-memory clipboard checks cover automatic copy, disabled copy and clipboard-only capture.                                           |
| Timer capture lifecycle         | Production selection/countdown completion, cancellation, timeout, show failure and retry. The frozen selection frame is released before countdown; completed captures use live pixels; unsuccessful attempts leave no image/history/active selector.                                                                                                                                    |
| OCR/QR capture lifecycle        | Fourteen production scenarios cover success, live capture with hidden desktop icons, whitespace-only results, recognition error, partial-file capture error, Escape cancellation and retry. Clipboard/notification outcomes, preprocessed OCR input, temp-file deletion and no history/output leaks are asserted.                                                                       |
| Scroll capture lifecycle        | Fourteen scenarios and a held-start probe cover cancellation, early events, errors, held responses and retry. Live-screen parameters, cleanup and saved output/history/editor equality are asserted. Two-frame stitches reopen pixel-identically, including exact 97px and 217px overlaps earlier rejected or misidentified, and a 101px tie. Real-screen stitching remains unverified. |
| Editor attachment lifecycle     | Eight production scenarios cover all four edges, frozen/live/fallback pixels, partial capture failure, invalid image, cancellation and retry. Exact composed pixels and history reopen match; temporary files, new history entries and clipboard changes are rejected. Competing capture requests preserve the pending selector, and a selector that cannot open restores the editor.   |
| Pin composition and restoration | Rendered pin pixels match the annotated PNG within two channel levels. Closing the pin restores annotations and identical export bytes with history enabled or disabled, without adding history entries. Enlarged/narrow bounds are checked against natural logical size.                                                                                                               |
| Capture preview lifecycle       | Four corner configurations preserve 200×140 cards with 12px gaps. Adding a fifth capture evicts the oldest card without deleting its file; color pixels verify order. Double-click opens the selected capture with identical PNG export. Auto-dismiss closes the stack while preserving files and history.                                                                              |
| Native analysis services        | Windows capture of an owned OCR editor window contains the expected recognized text. The rebuilt QR service reads the full 952×651 captured window, including its toolbar/margins. Native corpus checks improve from 42/65 to 65/65 exact payloads without regressions. These probes do not certify native app clipboard delivery.                                                      |
| Native live window pick         | Background input selected an owned editor window, closed the selector and opened its 952×651 capture; decoded pixels exactly matched a direct daemon capture of that target.                                                                                                                                                                                                            |
| Native frozen window pick       | With freeze enabled, the former path captured a rectangle added after opening selection. The fixed saved output preserves the pre-change region: zero changed pixels versus before, while the live target differs by 3,761 pixels in that region.                                                                                                                                       |
| Native frozen area capture      | The selector client origin formerly sat four pixels above the display after activation. It now aligns at (0,0); the saved 400×240 region exactly matches the intended frozen crop, excluding a later annotation that changes 2,393 live pixels.                                                                                                                                         |
| Native timer capture            | A source annotation added during the real five-second countdown appears in the saved 400×240 capture: zero changed pixels versus the live target and 1,335 versus the pre-countdown region.                                                                                                                                                                                             |
| Native timer cancellation       | The named native countdown panel is discoverable through accessibility inventory. A background click closes it and the selector; no screenshot or history file appears after the countdown would have ended.                                                                                                                                                                            |

Local native artifacts are under `.scratch-parity/native-workflow-fixed/`,
`.scratch-parity/native-workflow-motion-tty/` and `.scratch-parity/live-data/`.
They are development evidence, not release binaries or portable CI fixtures.
Window-picker before/after evidence is in `.scratch-parity/picker-native-*/`;
the reviewed comparison sheet is `.scratch-parity/window-freeze-native-proof.jpg`.
Area/timer artifacts are in `.scratch-parity/selection-native-*/`; the reviewed
comparison sheet is `.scratch-parity/selection-native-proof.jpg`.
Native OCR/QR input, captured windows and decoder diagnostics are in
`.scratch-parity/analysis-native-*/`. The QR photo is ZXing's
[qrcode-1/1.png](https://github.com/zxing/zxing/blob/56b8a9a7b53392097891463023a26c406b422225/core/src/test/resources/blackbox/qrcode-1/1.png),
downloaded at the pinned revision with its Apache-2.0 license and expected result.
The grayscale full-window and threshold-scale regressions are retained in the
native test fixtures; their sources and license are recorded in `THIRD_PARTY_NOTICES.md`. The remaining
probe assets stay outside the app and visual baselines. Native before/after
results for 20 QR photos, their grayscale originals and framed/resized variants
are in `.scratch-parity/qr-corpus-*/native-{before,after,otsu,medium}.json`.
The current decoder passed all 65 cases and four placements of a photographed QR
inside a synthetic 3840×2160 image. An isolated WSL harness compiled the actual
Linux QR loader and passed its eight tests; it did not build the full Linux daemon.
In an interleaved release benchmark, a 4K gradient without a QR took 493–496 ms
versus 334–370 ms before the regional and extra threshold retries. A uniform 4K
image took about 2 ms after an early exit, versus 291–323 ms before. A later
same-machine run measured 481–505 ms before the transition preflight and
381–404 ms afterward; an embedded 4K QR remained around 367–385 ms in both.
The Windows daemon still matched all 65 payloads and the Linux loader passed its
eight isolated tests. The preflight preserves thresholded rows with at least four
color transitions, including edge patterns. Evidence is in
`.scratch-parity/qr-preflight-corpus.log` and `.scratch-parity/qr-preflight-bench-idle.log`.

Bugs fixed in this pass:

- Editor changes made shortly after opening could remain absent from history
  until the window closed. A trailing save now persists the latest committed
  state; light and dark workflows check the deadline, an unchanged selection,
  reopening matching annotations and PNG pixels while the editor stays open,
  and closing during a new unfinished annotation.
- Windows screenshot requests now reject overflowing bounds. Monitor midpoint
  and crop offset calculations use wider intermediates; synthetic tests cover
  both overflow axes, extreme coordinates and negative-origin crops. Physical
  multi-monitor capture remains unverified.
- Cached Windows captures now reject disjoint frames before allocation and
  stream output in bands of at most 4 MiB instead of allocating the entire
  composition. Dimension checks accept an eight 8K-display layout with padding
  under the 512 Mi-pixel limit; larger cached requests fail before mask capture or file
  creation. Tests preserve off-screen padding, display gaps, alpha masks and
  the empty-cache fallback. A window alpha mask and encoder internals can still
  use additional memory; large native captures and slower hardware remain
  unprofiled.
- Clicking an annotation or resize handle, or reapplying its current option,
  discarded Redo without changing the image. Moves, resizes and option changes
  now create a history entry only when the annotation differs; returning to the
  starting point restores the original. Tests cover all four rectangle handles,
  a clamped circle, unchanged and mixed-selection options, real moves and
  resizes, and queued pointer updates before Undo or an option change. A GPUI
  workflow also selects a circle and exercises Redo and Undo through the window.
  Electron's source commits a drag only after an update was staged, but does not
  compare the final state with the starting state; native Electron behavior for
  these cases remains unverified.
- Area move/resize completion ignored the release coordinate in both shells.
- A window click used the last hovered candidate rather than hit-testing the
  click position. Empty-space clicks now leave selection open; a direct target
  click works without a preceding move.
- Dedicated window screenshots ignored "Freeze screen" and all window captures
  read live pixels. Screenshot pickers now freeze on entry and crop the retained
  frame with the picked window's alpha mask. Failed or unavailable freezes use live window
  capture. The selector hides immediately; capture completion releases its
  generation, including on capture errors.
- Capture overlays now have a native/accessibility window name, including pooled
  windows, so their lifecycle can be inspected.
- Area/display capture queued frozen-frame release before the reserved crop,
  which could block the crop on a single worker. Release now runs after capture,
  including errors. Failed or unavailable freeze responses explicitly use live pixels.
- A failed timer-control `show` left the handed-off selector active. It now closes
  the selector and clears the timer session; cancel/timeout/retry are also exercised.
- OCR/QR capture errors left partially written temporary PNGs behind and exposed
  an internal capture-method error. They now delete the partial file and use the
  same capture-failure notification text as Electron.
- QR thresholding depended on the selected image width and missed a photographed
  code surrounded by toolbar/margins. A local threshold pass now runs when the
  original decoder finds no payload. The full-window regression changed from
  empty to the exact expected payload; existing successful corpus cases remain
  unchanged. Windows and Linux loaders share the fix; Linux's regression passed
  in hosted CI. Interactive Linux app behavior remains unverified.
- A photograph with uneven lighting needed a global histogram threshold after
  both adaptive passes failed. Windows and Linux loader regressions use the same
  test-only photograph; the Windows service returned the exact expected payload
  and the Linux regression passed hosted CI.
- The fixed local threshold size could merge a photographed finder ring into its
  background, or leave an otherwise detected grid undecodable. Two bounded local
  threshold scales run only after the earlier passes fail. Two new photographs
  now decode through the Windows loader; Linux loader tests passed hosted CI.
- Some photographed codes produced a grid that could not be decoded against its
  surroundings, or needed a different threshold scale. Bounded regional retries
  and two final threshold scales recover the remaining corpus cases, including a
  QR at four positions inside a synthetic 4K capture.
- GIF frame delays were truncated independently to centiseconds, speeding up
  24 and 30 FPS exports. Rounded cumulative frame boundaries preserve the edited
  timeline within GIF's centisecond precision.
- Canceling an export could leave its worker writing while a retry reused the
  output, or report success at the final frame. Cancellation now waits for the
  worker and suppresses success actions; Unix encoder children are stopped and
  reaped before the output can be reused.
- Scroll capture subscribed after the start RPC, losing early completion or
  cancellation events and leaving the session active. Subscription now precedes
  start; both early events and a successful retry are exercised.
- Scroll startup waited for the daemon on the GPUI thread. It now reserves the
  session before starting the RPC in the background. A held-reply fixture checks
  that the coordinator returns, rejects a second user request, and still saves
  and reopens an early completion. An isolated test checks that a queued start
  skips the daemon after shutdown begins. The headless dispatcher exercises
  ordering, not native OS responsiveness; an already admitted start can still
  complete during shutdown and is then canceled.
- A new selection could release another active capture's desktop-icon hide.
  Capture flows now retain their own hide through selection, handoff and cleanup.
  An isolated Windows GPUI test covers replacement, cancellation, disabled
  hiding and failed timer/scroll handoffs without changing the real desktop.
  Windows scroll completion and cancellation attempt to restore icons before
  waiting for the daemon's final response. Pending icon transitions are serialized
  off the UI thread; replacement selectors and captures wait before freezing or
  reading pixels. Failed restores are retried; a second failure stops the next
  capture with an error. The fake-daemon workflow holds hide/show replies, checks
  saved pixels and failed-reply recovery, verifies rapid tray toggles, and checks
  that a superseded direct capture cannot report a stale error. An attachment
  restore failure clears the pending attachment and focuses its editor in the
  headless fixture. Isolated tests cover replacement, timer ownership and shutdown.
  Failed hide requests now stop the capture and attempt to restore icons even
  when the reply was lost. The fake daemon checks area, display, direct-screen,
  OCR and QR failures, restoration retry and release of a previous frozen frame.
  Native overlapping captures and real desktop-icon transitions still need
  verification.
- GPUI area screenshots froze the screen before hiding desktop icons, so the
  frozen image could retain them. Like Electron's area flow, GPUI now hides icons
  before selection and retains the hide through capture or cancellation. A fake
  frozen-frame fixture checks saved and reopened pixels and cancellation; real
  desktop compositor timing still needs verification.
- All-in-one now begins the configured icon hide before freezing. Screenshot
  window selections retain that hide through capture, and recording selections
  wait for restoration before opening the controls. New fake-daemon workflows
  cover saved pixels, recording targets, failures, replacement and a second overlay;
  they still need a hosted visual run and native desktop verification.
  Electron also hides icons before taking its frozen snapshot and restores its
  capture reference before revealing the overlay. Synthetic-pixel tests cover
  cancellation, replacement and restoration failures; native icon transitions
  remain unverified. Confirmed frozen frames stay owned until the screenshot
  consumer finishes; held-selection and held-capture tests check that a
  replacement waits. Windows live captures fall back to Electron's window hide
  if the native hide fails; unit tests also cover a delayed hide failure after a
  newer show.
- The frozen multi-display screenshot picker had the same ordering problem. It
  now hides icons before freezing and retains the hide until capture or
  cancellation. A fake-daemon workflow duplicates a display target and verifies
  request order and saved pixels; a separate simulated two-display test checks
  cancellation and the live-picker path. Physical monitors remain unverified.
- Standalone OCR and QR capture now honor the desktop-icon hiding preference
  before selection and freezing. A headless lease test covers cancellation and
  capture failure for both; native icon timing and successful capture remain
  unverified.
- A timer could close a newer selector and capture its old area after the
  selector was replaced. Replacement now cancels its countdown; isolated GPUI
  tests cover replacement, queued completion or cancellation events and a held
  hide operation. A normal completion still attempts capture through the fake
  daemon. Native overlapping countdown and selection behavior still needs
  verification.
- On Windows, a single-display screen capture could leave a previous selector
  open and use its frozen pixels. It now closes the selector and releases its
  freeze before capturing live pixels; a newer selector invalidates a pending
  screen request. Mock-platform tests cover live and frozen replacement. A
  daemon-fixture workflow verified release before an uncached capture, saved
  pixels and clipboard pixels in the hosted run at `2d50016b`.
- On macOS and Linux a replaced selector could open after its freeze completed,
  or a live capture could start before that freeze was released. Pending freezes
  now retain ownership across replacements and cancellation; a failed release
  prevents the next selector from opening and remains available for retry.
  Isolated GPUI tests force the delayed-open path, simulate successful freezes
  and release failures, and cover rapid replacements and cancellation. Native
  macOS replacement and cancellation are verified on one display in the
  follow-up below. Native Linux and multiple displays remain unverified.
- After a completed frozen capture, a failed release could lose its retry token.
  The token now remains pending, with an immediate retry and notification if it
  still fails. Isolated tests cover the shared capture/release path, replacement
  generations and live-selector retry; native failure behavior remains unverified.
  The coordinator-level fake-daemon retry passed in both light and dark profiles
  in [hosted CI](https://github.com/Porabuild/Poratake/actions/runs/43679096462).
  That run failed its baseline state-list check because the test emitted an
  unintended extra image; the diagnostic is now written outside that list.
  A successful release clears the matching cached generation. A capture that
  reserves that generation while Release is in flight now waits for its reply
  before choosing pixels. A successful reply sends the capture down the live
  path. A failed reply leaves the existing frozen generation available for retry.
- Direct Record Area now uses a live selector even when screenshot freezing is
  enabled, matching Electron. A headless test also checks that All-in-one Record
  retains its configured freeze. The native Windows workflow in [hosted CI](https://github.com/Porabuild/Poratake/actions/runs/43667610426)
  passed the freeze-preference positive control and live-pixel check. The visual
  matrix then failed two profiles because a fixture still expected direct recording
  to freeze; the revised fixture needs hosted verification.
- A failed or timed-out freeze could continue in the daemon after the app opened
  a live selector. The app now releases the failed freeze before falling back;
  if cleanup fails, it closes the selector and retains the generation for retry.
  The native failure-injection checks below verify cleanup, live fallback,
  retry after a rejection, and the actual app deadline with a delayed reply.
  Natural ScreenCaptureKit stalls remain open.
- On Linux, release could complete while an X11 freeze window was still being
  presented or a replaced window was still closing. Generation changes,
  presentation and release now share a lock, and partial presentation failures
  explicitly destroy and synchronize their created windows. Deterministic lock
  tests cover the ordering. Xvfb keeps the client connection open and confirms
  a mapped window is removed after a later frame fails; an isolated probe of the
  previous cleanup fails that check. A real desktop handoff remains unverified.
- Starting a screen recording on one display could leave a previous selector
  open or let it appear after the recording bar. The recording request now
  replaces the selector and waits for its freeze to release. Isolated GPUI
  tests cover an existing selector, a pending freeze, a newer selection,
  a superseded screen capture and a failed release followed by retry. Native
  recording handoffs remain unverified. A daemon-fixture workflow checks that
  the bar is initially closed, then checks the release RPC and the settled bar;
  it passed in the hosted run at `bbf5e4e9`.
- Releasing a selector while its freeze was starting could deadlock the freeze
  worker. Release now waits outside the operation lock and rechecks later
  reservations. Thread-coordinated tests cover the deadlock and a reservation
  arriving during release.
- A full GPUI scroll progress queue could discard Done or Cancel. Terminal and
  interruption signals now use a separate queue while progress remains bounded.
  The old bounded queue could also discard the final auto-scroll or cursor state
  during a burst of previews, leaving the toolbar or prompt stale. Pending progress
  now retains the latest value of each kind. A GPU-free test sends 2,000 previews
  with state changes and verifies the latest values; another sends 20 warnings and
  verifies terminal actions take priority. A GPUI control test feeds a frame burst
  and stop state through the queue, then checks the tooltip and next button action.
  The isolated light-theme 1280-point, 1× simulated workflow passed after this
  change, including scroll selection, cancellation, failures and save/reopen; its
  output is under `/tmp/poratake-parity-scroll-coalescing-final/`. Its generated
  screen, area and window recording fixtures also compared decoded exports with
  the editor preview. Native interaction and presentation remain unverified. The
  earlier burst-event workflow fixtures completed in the hosted run at `8b764bc0`
  before its image baseline comparisons failed.
- Scroll capture cleared its active state before finish or cancellation completed,
  allowing a second session to start while the daemon was still processing the
  first. Held-response fixtures now verify the state through both RPCs.
- Both shells previously timed out a scroll stitch after 30 seconds while the
  daemon could still be writing. They now wait for the response or daemon exit.
  A 31-second held-worker GPUI workflow saves the expected pixels; restoring
  the old timeout makes it fail. Electron tests advance the daemon client's
  clock and verify ordinary requests retain their timeout.
- Electron scroll capture also allowed a new session before cancellation
  completed. It now waits for cancellation or an in-flight stitch, stages the
  stitch in a unique sibling file, and removes only that file if canceled.
  Filesystem tests preserve an existing destination on cancellation and compare
  promoted bytes on success. A failed promotion retains the staged PNG and
  reports its location. Auto-scroll cannot restart while the stitch is in flight;
  real daemon and native UI delivery remain unverified.
- The macOS GPUI scroll controls treated global logical selection coordinates as
  scaled, display-local coordinates. Control, preview and prompt bounds now use
  the selected display's local coordinates; a platform-specific unit test covers
  a negative display origin at 1× and 2×. Native multi-display behavior remains
  unverified. Narrow selections at either display edge also placed the GPUI and
  Electron control bars offscreen; both now clamp the horizontal position, with
  Electron tests covering primary and negative-origin displays. The macOS GPUI
  preview now uses the same native click-through setting as its prompt, matching
  Electron's overlay; native pointer delivery remains unverified. Both shells
  keep the growing preview within the display, including a 200px-wide simulated
  display. Electron renderer tests cover offset and short displays, and a
  headless GPUI session verifies window and painted-image positions as frames
  arrive; a narrow-display test also checks that tall previews retain the bottom
  pixels. Physical narrow-display behavior remains unverified.
- The macOS GPUI scroll preview's rounded container did not clip the image
  corners. The image now has the same radius, and the Windows GPU harness samples
  all four corners and the center of the production preview at each matrix scale.
  A later [native macOS check](#2026-11-18-macos-scroll-preview-corner-clipping)
  verified all four corners on Retina.
- The macOS scroll-control tooltips painted above their 52px window and were
  clipped. They now use the native view tooltip, matching Electron's native
  tooltip behavior without enlarging the window. A headless test exercises all
  three controls, checks their hover labels and verifies Start/Stop requests and
  Done/Cancel signals. Native tooltip appearance and timing remain unverified.
- The macOS scroll preview appended the first rows of each new frame, while the
  saved stitch appends its last rows. It now uses the trailing rows. A macOS CI
  fixture compares one, two and three preview frames with saved PNG pixels when
  measured overlap matches the expected scroll step; the fixture passed in
  [hosted CI](https://github.com/Porabuild/Poratake/actions/runs/42998144733).
- A macOS scroll capture could discard its first frame when its sampled hash was
  zero. The duplicate detector now starts without a previous hash. The macOS CI
  fixture exercised a black zero-hash frame and subsequent duplicates in the
  same hosted run.
- The coarse scroll overlap search rejected a true 97px overlap and selected
  716px for a true 217px overlap in patterned fixtures. When the sampled
  result is rejected or farther from the expected scroll, a unique exact
  full-row match now takes precedence. Two-frame PNGs reopen with identical
  pixels, including an equal-distance tie. A synthetic 4K no-overlap check
  took a median 3.20–3.34 ms locally versus 1.43 ms before; physical
  pages and slower machines remain unverified.
- The Windows GPUI backend wrote PNG without a standard bitmap format and
  misread DIB headers with embedded masks. It now supplies CF_DIB alongside PNG;
  five byte-level tests cover bitmap headers, alpha, 16-bit input and invalid
  offsets. An isolated [hosted Windows test](https://github.com/Porabuild/Poratake/actions/runs/43059430548)
  passed native text and PNG writes, PNG and DIB format availability, and
  PNG-only and DIB-only reads. Interactive delivery through the app and another
  application still needs verification. A synthetic 4K
  PNG-to-DIB conversion took 48–53 ms locally, excluding clipboard API calls,
  and produced a 31.6 MiB bitmap; slower hardware has not been measured.
- Polish-copy and editor wallpaper padding previously used unscaled image pixels.
  A 1600×1000 capture with 50px padding produced 1700×1100 at 200% instead of
  Electron's 1800×1200. Wallpaper geometry and frame scaling now use the same
  displayed-image ratio; padding without a background remains transparent in the
  saved PNG. Same-size effect edits retain the prior preview while its replacement
  renders. Physical monitor changes remain unverified.
- The padded, unframed single-image preview painted an opaque surface beneath
  partially transparent pixels. It now reveals the wallpaper backdrop in the
  tested case without corners, shadow, inset, balance or attached layers.
- A framed single-image preview also blended translucent pixels against the
  editor surface, and its extra GPUI shadow darkened them. The frame compositor
  now follows Electron: wallpaper shows through without a shadow, and the frame
  content color backs the image when a shadow is present. Shared raster edges
  also preserve the frame border and rounded clipping above the image and its
  annotations; selection and crop handles stay visible above the frame. At
  simulated 125% and 200%, every pixel of a 640×360 preview matches the PNG
  within two channel levels for all four styles, with and without shadow.
  Transparent and frame-only backgrounds, corner annotations and GPU atlas
  eviction are checked. Translucent custom backgrounds with blur are within
  three channel levels in the interior and two at the frame edges after theme
  changes. Generating 4K edge tiles took 4–5 ms locally and used
  about 40 MiB of temporary mask and border buffers, excluding the backdrop and
  GPU upload. Complex compositions and arbitrary zoom remain unverified.
- A rounded single-image preview left square corners and painted an opaque
  surface beneath transparent pixels. Without a window frame, inset, balance or
  attached layers, the shared raster mask now clips the live image and
  annotations to the exported shape, with or without a shadow. At simulated
  125% and 200%, the whole padded canvas matches the PNG within two channel
  levels over solid or transparent backgrounds; translucent custom backgrounds
  are within three levels. Corner
  annotation pixels, all four selection and crop handle quadrants, and crop
  dimming over the rounded cutout are checked. A local
  4K probe generated four corner tiles in 1 ms at 48px radius and 5 ms at 400px;
  the latter used 2.5 MiB of tiles plus an 8 MiB temporary mask, excluding the
  backdrop and GPU upload. A tiled shadow keeps the outline at its true size;
  a local 4K backdrop took 53–58 ms without a shadow and 88–93 ms with a 24px
  radius and moderate shadow. A 400px radius with the strongest tested shadow
  took 196–200 ms; GPU upload and slower hardware are not included. Plain,
  inset and framed 17,000px-wide and -tall captures retain shadows in saved and
  reopened PNGs. CPU preview shadow checks match the saved output. An isolated
  GPUI Metal fixture now checks visible portions of oversized framed previews
  with padding, inset and shadow against reopened PNGs. The unframed matrix
  below also covers selected inset, corner and shadow settings; physical
  display behavior remains unverified.
- A plain screenshot wider or taller than the GPUI atlas limit could render as
  the editor background. The editor now tiles oversized source images, keeping
  neighboring pixels at each edge and retaining the original image for export
  and redaction. A generated 17,000×100 image was blank at a sampled pixel
  before the change. The [macOS Metal evidence](tests/native/evidence/macos-ui/oversized-screenshot-preview-20261117.txt)
  checks both orientations, panning, save and reopen in light and dark themes
  at simulated display scales. CPU tests cover tile bytes, alpha, crop and undo.
  This does not verify an actual scroll capture, physical display scaling or
  interactive performance on large two-dimensional images.
  A subsequent detailed comparison found that independently scaled tiles could
  distort textured pixels: 1,176 of 3,472 compared pixels differed from an
  untiled source crop by more than two levels, maximum 95. On macOS the plain
  image tiles now use the full image's snapped sampling grid and sufficient
  overlap for the minimum zoom. The [follow-up evidence](tests/native/evidence/macos-ui/oversized-screenshot-sampling-20261117.txt)
  checks opaque and translucent texture around a seam at 25% and 50% zoom in
  both appearances and three simulated scales; the maximum difference was one
  level. The same distortion occurred in a tiled composed backdrop; macOS now
  applies the same sampling grid and overlap there. With a solid wallpaper and
  no padding, the [composed evidence](tests/native/evidence/macos-ui/oversized-composed-sampling-20261117.txt)
  compares textured and translucent previews with cropped references and
  reopened large PNGs. Frames, shadows, padding, other platforms and physical
  capture remain to be checked with the same detailed content.
  An additional [seam annotation matrix](tests/native/evidence/macos-ui/oversized-annotation-seams-20261117.txt)
  places a red stripe before blackout, blur or pixelation and a green shape
  afterward. At 25% and 50% zoom, both themes and three simulated scales, the
  generated preview matched the reopened large PNG within one level; the
  exported crop matched a smaller reference exactly. An unredacted control
  confirms each effect changed pixels, and blackout covers the earlier stripe.
  This uses a solid wallpaper without padding or frames; real capture and
  physical input remain unverified.
  A separate [framed oversized matrix](tests/native/evidence/macos-ui/oversized-framed-reopen-20261117.txt)
  covers both macOS frame styles, padding, two inset and shadow settings,
  textured opaque and translucent images, and panning to a tile seam. Across
  both app appearances, 25% and 50% zoom and three simulated display scales,
  visible preview pixels matched the reopened PNG within one level. This is an
  isolated renderer check, not a physical display or live capture.
  A rounded, shadowless oversized image with padding took a different path:
  its separate wallpaper backdrop exceeded the texture limit and disappeared.
  macOS now tiles that backdrop separately from the rounded-edge masks. The
  [unframed evidence](tests/native/evidence/macos-ui/oversized-unframed-backdrop-20261117.txt)
  checks both appearances, 25% and 50% zoom, panning, and opaque and
  translucent sources. The earlier 125% run found 17,218 pixels more than two
  levels apart at 25% zoom and 45,978 at 50%. The macOS preview now composes
  the image and wallpaper together before scaling; annotation patches retain
  the export's source-boundary and rounded clipping. The [follow-up evidence](tests/native/evidence/macos-ui/oversized-rounded-shared-composition-20261117.txt)
  compares generated previews with reopened PNGs across appearances, simulated
  scales, zooms, alpha and redactions. The earlier [125% preview](tests/native/evidence/macos-ui/oversized-rounded-125-preview-20261117.png)
  and the [corrected preview](tests/native/evidence/macos-ui/oversized-rounded-125-after-20261117.png)
  are retained. This does not verify physical displays, live capture or
  interactive performance.
  A further [translucent and drag check](tests/native/evidence/macos-ui/oversized-rounded-translucent-interactive-20261117.txt)
  covers a 50%-alpha wallpaper and synthetic rectangle gestures. Across both
  appearances and three simulated scales, 576 visible-region comparisons
  matched reopened PNGs within two levels; 96 drags showed a draft and
  committed a rectangle. The fixture's move, settle and capture timing does
  not measure physical input latency or sustained interactive performance.
  A further 60 runs covered five unframed corner, shadow and inset combinations
  in both appearances at all three simulated scales and both zooms. All 480
  visible-region comparisons matched reopened PNGs within one level, and
  controls confirmed that the enabled corner and shadow settings changed the
  exported pixels.
  An [ordered annotation and image-wallpaper check](tests/native/evidence/macos-ui/oversized-ordered-image-wallpaper-20261117.txt)
  covers a preceding shape, blackout/blur/pixelation and a following shape at
  a tile seam. Its 120 generated preview comparisons matched reopened PNGs
  within two channel levels. It also exposed a separate transparency bug:
  background blur drew a second copy over the original, raising a 50%-alpha
  wallpaper to 75% alpha. The shared renderer now applies the blurred source
  once. The check includes image wallpapers with and without blur.
  A later [Electron comparison](tests/native/evidence/macos-ui/wallpaper-blur-cover-20261117.txt)
  found that GPUI omitted Electron's expansion of the wallpaper before blur.
  The renderer now uses expanded cover geometry, reducing the maximum error in
  one high-contrast margin from 43 to 13 channel levels. A patterned wallpaper
  still differs visibly. A [sampling follow-up](tests/native/evidence/macos-ui/wallpaper-image-sampling-20261117.txt)
  uses bicubic sampling for screenshot image wallpapers, matching Electron's
  high-quality setting on a normal-sized generated fixture. The same follow-up
  measures oversized patterned margins with mean maximum-channel errors of
  4.496 to 10.306 levels and maxima up to 24. Electron changes its sampling
  behavior for the oversized canvas on this Mac; the full cross-device cause
  remains unconfirmed.
  An [annotation seam check](tests/native/evidence/macos-ui/oversized-rounded-annotation-20261117.txt)
  exercises blackout, blur and pixelation across a tile boundary. At simulated
  100% and 200%, 192 visible-region comparisons matched reopened PNGs within
  two levels. Its earlier 125% full-preview failure is addressed by the
  shared-composition follow-up above. Separate export and bounded-patch tests
  guard annotation and redaction clipping at rounded corners and their order.
- Unframed attachments without inset or balance were rounded and shadowed as one
  group, unlike Electron's per-image rendering. Their images now receive their
  own corners and shadows in both preview and PNG. Light/dark GPU fixtures at
  simulated 125% and 200% compare all four attachment edges, a five-layer stack,
  two aspect ratios, transparent content, annotation pixels and history reopen.
  They also verify replaced tiles leave the GPU atlas. Solid or transparent
  backgrounds match within two channel levels and a translucent background
  within three, except at most 16 overlapping-shadow pixels within four levels.
  A local 4K moderate-shadow probe now takes 248–266 ms with one attachment and
  501–516 ms with five, retaining 38.69 MiB and 76.41 MiB of backdrop textures
  respectively, before GPU upload. The previous tiled preview retained 45.59 MiB
  and 118.35 MiB in the same probe.
  Physical DPI, interactive performance and framed or balanced combinations
  remain unverified.
- Unframed attachments ignored the inset when there was no outer padding. Each
  image now has its own inset and edge color, and annotations use the inset
  origin after the attached images are drawn. A 144×72 Electron canvas fixture
  independently confirms the layout and colors for two 60×60 images with a
  6px inset. CPU checks cover all four attachment edges and transparent source
  pixels. Light and dark GPU fixtures check zero-padding right attachments at
  simulated 125% and 200%, including transparent edge pixels, an annotation
  crossing the attachment, and a PNG reopened at 73% and 100% zoom. Blackout,
  blur and pixelation now cross a textured right attachment; the checks verify
  their effects on both images, full blackout coverage, no changes outside the
  redaction bounds, and preview/export equality. At 125%,
  selection halos are checked both inside the primary and across the attachment;
  a selected highlight retains its independently checked blended color. Tied
  edge colors now use the first sampled color, matching Electron and avoiding
  random preview/export differences. The two local NVIDIA runs each passed 191
  exact baselines without updates. Frames, balance, other inset effects, physical
  DPI and native interaction remain unverified.
- GPUI drew annotations into the primary image before attached images and their
  shadows, so a bright mark could be obscured at an edge. In the tested
  unframed, non-inset, non-balanced path, preview and export now draw annotations
  after the attachments, as Electron's rendering order does. Black, pixelated
  and blurred redactions crossing an edge are checked in both themes at
  simulated 125% and 200%, along with overlapping and clipped redactions,
  history reopen, crop, undo and texture retirement. Direct cross-shell pixel
  comparison and other wallpaper combinations remain unverified.
- A frame-only preset with no background or padding was ignored. It now activates
  the framed editor and export paths: the original 1600×1000 Polish capture is
  1600×1056 at 200%, and Polish copy matches editor export. The title bar and
  translucent image patch are checked in the preview at 125% and 200%.
- Corner- and shadow-only settings without padding or a background were also
  ignored. Without balance or inset, they now activate for single captures and
  attached images. Light/dark previews match saved pixels at simulated 125% and
  200%; Polish copy matches the editor for zero-padding corners. Transparent
  previews now use the active theme background, and a same-window theme change
  replaces and retires the old backdrop. Balanced single captures now also keep
  the crop, rounded corners, shadow and inset at zero padding; the editor uses
  the same composed backdrop for preview and export. Generated GPU fixtures in
  both themes check 72 settings combinations before and after reopen, including
  translucent source pixels and shape, blackout, blur and pixelation annotations.
  See the [native evidence](tests/native/evidence/macos-ui/balanced-unframed-effects-20261117.txt),
  [dark preview](tests/native/evidence/macos-ui/balanced-unframed-effects-dark-20261117.png)
  and [light preview](tests/native/evidence/macos-ui/balanced-unframed-effects-light-20261117.png).
  The fixtures cover simulated scales, not physical displays or live captures.
  Balanced attached images with corners or shadows now use the per-image
  composition too. An [Electron layout comparison and GPUI GPU fixture](tests/native/evidence/macos-ui/balanced-attachment-effects-20261117.txt)
  cover all four attachment edges, zero or 20px padding, simulated 100%, 125%
  and 200% scale, and a blackout crossing each seam. A follow-up [GPU fixture](tests/native/evidence/macos-ui/balanced-attachment-all-seams-20261117.txt)
  also checks blur and pixelation across all four edges in both themes, with
  and without padding. The [dark](tests/native/evidence/macos-ui/balanced-attachment-seam-dark-20261117.png)
  and [light](tests/native/evidence/macos-ui/balanced-attachment-seam-light-20261117.png)
  previews show one generated blackout case; the [dark blur](tests/native/evidence/macos-ui/balanced-attachment-blur-dark-20261117.png)
  and [light blur](tests/native/evidence/macos-ui/balanced-attachment-blur-light-20261117.png)
  previews show a right-edge case. Physical DPI, live input, other content and
  additional framed attached inset combinations remain open.
- Framed attached screenshots previously used one title bar for the group. The
  compositor now frames each image before laying out attachments, and restores
  the balanced primary crop and sampled inset color on reopen. A CPU fixture
  matches the 119×184 layout and interior landmarks of an isolated Electron
  canvas fixture; 510 of 21,896 pixels differ by more than two channel levels,
  up to 123 at transparent frame corners. This does not represent a complete
  Electron editor workflow. An anchored wide-stroke preview patch matches the
  full composition in CPU tests, including clipping at 100%, 125% and 200%
  scale. Hairline, dashed and shadowed stroke paths retain the prior behavior.
  [Hosted CI at `808730eb`](https://github.com/Porabuild/Poratake/actions/runs/43240023837)
  passed 14 of 16 visual cases and 2,498 exact baselines. In both themes, the
  balanced and unbalanced attachment previews and reopened PNGs matched within
  one channel level at simulated 100% and 125%. The 200% case stopped before
  its preview comparison because a title-bar landmark sampled the antialiased
  frame corner; the landmark now samples its interior. Physical DPI and broader
  frame/inset combinations remain unverified.
- An unframed single-image inset was omitted at zero padding when corners or a
  shadow were enabled, and otherwise occupied the outer padding. The inset now
  surrounds the image and the preview uses the exported composition. Light and
  dark GPU checks at simulated 125% and 200% cover rounded, shadowed and combined
  effects with zero or 20px padding and transparent or solid backgrounds. Both
  themes also check edge-crossing blur and a flattened PNG reopened at 73% and
  150% zoom. A CPU test verifies the corner, shadow and 17,000px tile edges.
  An isolated native Windows editor displayed the composition and responded to
  an inset slider click; native save and reopen were not exercised in that probe.
  An Electron canvas comparison of one 170×150 rounded-shadow export found 185
  of 23,475 pixels outside the annotation area more than two channel levels
  apart, with a maximum difference of eight near rounded edges. A local 4K CPU
  composite took 151–159 ms in six samples, excluding inset-color detection,
  GPU upload and interactive behavior; slower hardware is unmeasured.
- A window frame ignored the inset around a single unbalanced capture. The
  frame, inset and annotations now share the saved composition. Light/dark GPU
  checks cover all four frame styles, zero or 20px padding, transparent or solid
  backgrounds, shadow, translucent content and clipped blur at simulated 125%
  and 200%. A solid-background PNG was reopened at 73%, 100% and 150% zoom.
  At simulated 125%, a pointer test selects and drags a framed-inset annotation;
  its handle remains visible two pixels beyond the image's left and top edges,
  and the moved result matches a reopened PNG at those three zoom levels.
  An independent Electron 170×185 frame sample originally differed by more than
  two channel levels at 786 of 28,950 pixels outside the annotation area, with
  a maximum difference of 43. Higher-precision antialiasing for the Windows
  maximize icon reduces that sample to 707 pixels and a maximum of 27; other
  controls, shadows and frame edges still differ. A reviewed Electron image
  baseline covers the maximize icon in both themes at seven scales from 100%
  to 250% and four fractional positions, with every sampled channel within two
  levels. CPU checks also cover clipping, alpha and canvas edges; transformed
  canvases retain the previous stroke path. Local light and dark Windows GPU
  runs each passed 191 exact baselines and the framed preview/export checks at
  simulated 125% and 200%. Physical DPI remains unverified.
- Fixed aspect ratios that add height placed unbalanced content at the top.
  The shared layout now centers it; a 9:16 framed and unframed inset with a
  solid background matched its reopened PNG at 73%, 100% and 150% zoom. A
  composited attachment also rounded its preview height differently from its
  PNG; its 9:16 preview now matches the reopened output at those zooms in the
  local light/dark probes. Source inspection shows Electron applies the aspect
  ratio to its editor preview but does not pass it to PNG export.
- Single unbalanced frames and shadows used separate scaled image and effect
  layers in the preview. In a light-theme, simulated 125% probe, a 9:16 frame
  without inset differed from its reopened PNG at 73% zoom by 948 pixels with a
  shadow and 947 without one, beyond two channel levels. They now share the
  saved composition and match the reopened PNG at 73%, 100% and 150% in the
  local light/dark probes. CPU tests cover horizontal and vertical 17,000px tiled
  frames, including the unshadowed case. Full-sized backing pixels remain
  necessary for blur and pixelation sampling across tiles. For a 3840×2160
  shadow with 20px padding, an alternating optimized CPU test with one warmup and
  six measured samples found median backdrop preparation times of 89ms before
  and 149ms after for an unframed image, and 92ms before and 146ms after for a
  Windows frame. The new image buffers were 32.56 and 32.98 MiB respectively;
  a subsequent row-order and opaque-pixel conversion optimization reduced median
  preparation from 142–148ms to 109–113ms in a separate alternating 4K test
  using opaque and mixed-alpha sources. The compared output buffers were byte
  identical. Source pixels, temporary allocations, GPU upload, interactive
  behavior and slower hardware are not measured by these tests.
- Balanced single captures now restore their source crop on reopen and recompute
  it after crop and undo. The crop is detected before annotations, and preview
  geometry and translucent pixels follow the saved PNG. Light/dark GPU checks at
  simulated 100%, 125% and 200% cover zero or 20px padding, transparent and solid
  backgrounds, border annotations and history reopen. An unframed inset now
  increases the canvas by its width on each side, as in Electron, instead of
  consuming the padding. Light/dark GPU checks at all three scales cover zero
  and 20px padding, transparent and solid backgrounds, translucent pixels and
  annotations; balanced cases also check the source-edge color and backdrop
  replacement after an equal-size crop. Annotation patches are composited against
  the underlying backdrop once, avoiding double blending on transparent captures.
  Attachments on all four edges without a frame,
  inset, corners or shadow are checked at those scales with padding and
  background variants, including a pixelated redaction crossing each join and
  the reopened preview. Annotations outside the right-attachment canvas are
  clipped. Complex balanced combinations remain unverified.
- Pixelated video redactions could shift a block boundary by one pixel between
  the saved composition and the local preview patch when their canvas origins
  differed. Their nearest-sample expansion now uses a position-independent
  mapping, with its boundary choice checked against an Electron canvas. A
  Recorded striped patches verify that both pixelation and blur visibly change
  the video preview and match decoded MP4 pixels across a cut and speed edits.
- Screenshot pixelation now averages fixed-size blocks like Electron instead of
  using the video renderer's downsampling. Electron canvas samples cover alpha,
  clipping and scale. Light/dark GPU checks compare saved and restored plain
  previews at simulated 100%, 125% and 200%, including translucent and clipped
  regions, 50% and 150% zoom, wallpaper toggles and a same-window DPI change.
  At 100%, an earlier shape under the redaction is checked with and without a
  gradient wallpaper for opaque and translucent images. Plain captures with an
  earlier shape also match the flattened image, including translucent source
  pixels. The redaction and its edge band now match within two channel levels
  at 50%, 73%, 137% and 150% effective zoom in both themes at simulated 100%,
  125% and 200% DPI, including clipped regions and a later shape and blackout.
  At 150%, the edge band previously differed by up to 37 channel levels for an
  opaque image and 18 for a translucent image. New blur and pixelation drafts
  over an existing filtered redaction retain their dark drag preview in single
  captures and attached-image compositions. Six earlier NVIDIA baseline
  changes and two later history-reopen changes were reviewed. A local opaque 4K block operation
  took 29–31 ms in six samples, excluding GPU upload; a redundant retained
  plain-image cache that took 128–137 ms and 31.6 MiB per redaction was removed.
  Complex compositions, physical DPI and slower hardware remain unverified.
- Committed screenshot blur now replays preceding annotations at source
  resolution and composites the backdrop once. Its radius scales with native
  image scale; light/dark GPU checks compare unframed transparent captures with
  and without inset to saved pixels at simulated 100%, 125% and 200%, including
  a clipped blur and annotations before and after it. The blur region and its
  immediate boundary now match a flattened PNG within two channel levels at
  50%, 73%, 137% and 150% effective zoom in both themes at simulated 100%,
  125% and 200% DPI, including inset, transparent padding, a gradient and
  overlapping shape, blackout and pixelation annotations. Before the change,
  an inset fixture differed by up to eight levels at 50% zoom and 65 at 150%.
  The image and annotation overlay use the same measured layout bounds. An
  aligned patch is reused across integer-device-pixel panning when its size and
  relative stage position remain unchanged. At 4K, preparing a 1920×1080 blur
  at 150% took 179–189 ms in three local samples, excluding GPU upload; cached
  lookups took about 2 μs. Large textures that exceed the existing patch budget
  fall back to the previous rendering path.
  Before the patch-local composition change, a whole-frame comparison of the
  inset fixture at simulated 100% DPI stayed within one level at those zooms
  when the blur was not clipped. The clipped variant differed outside the
  checked region, reaching 56 levels at 137% and 55 at 150%. A later whole-frame
  comparison of the [hosted `b8dd8672` artifact](https://github.com/Porabuild/Poratake/actions/runs/43688781846)
  found maxima of 23 and 20 levels,
  with 238 of 20,988 and 381 of 25,056 pixels differing by more than two levels,
  respectively, in both themes. These pixels remain outside the strict regional
  assertion; physical DPI and the current head remain unverified.
  The shared blur kernel previously used the box width as its radius. Sampled
  Electron canvas edge profiles at sigma 0.5–32 now differ by at most three
  channel levels; 56 shadow samples differ by at most four alpha levels.
  Translucent screenshot blur now composites the filtered source over the
  original temp image as Electron does, with interior and clipped samples within
  two channel levels. Narrow buffers and transparent padding have CPU checks.
  Unframed single images without balance, corners or shadows now composite the
  gradient or inset before redaction in export and patch-local preview tests.
  Twelve independent Electron canvas samples, including a clipped blur, differ
  by at most three channel levels. Light and dark local GPU workflows at 1280px
  and 100% now check annotation and draft transitions and a fractional aspect
  ratio against reopened PNG pixels. A CPU sample at one fractional origin
  matches export with either the rounded or unrounded sampling offset. Screenshot
  export and CPU preview patches now apply redactions before vectors and
  highlights, with a rounded blackout pass in vector order. Eight independent
  Electron center samples for overlapping annotations match within two levels;
  one rounded corner differs by 22 levels. Light and dark local GPU checks cover selection
  and both blackout/shape orders at 73% and 150% zoom, within two channel levels.
  Framed, balanced,
  rounded, shadowed and attached compositions, native interaction and broader
  images remain unverified. On this machine, the revised transparent kernel took
  220–235 ms for 19,200×160 pixels at sigma 104 in three local kernel-only
  samples; the screenshot path's estimated peak live buffers, including the
  canvas, are about 49 MB for that case. At 3840×2160 and sigma 20, three
  kernel-only samples took 265–280 ms for color and 86–92 ms for alpha, versus
  258–263 ms and 67–74 ms before this change. GPU upload and slower machines
  remain unmeasured.
- Plain attached captures could show a one-pixel seam at their join. Unframed
  attachments without balance or inset now share a single preview composition,
  with annotations above the images. The workflow checks joins on both sides;
  preview and export comparisons cover transparent attachments and black,
  pixelated and blurred redactions at simulated 125% and 200%. Redactions sample
  the image's transparency before the preview applies the theme background. Ten
  NVIDIA attachment baselines were reviewed; the other 372 images in those two
  workflow runs were unchanged. The [hosted Windows run](https://github.com/Porabuild/Poratake/actions/runs/43037376649)
  passed its native tests but failed the visual gate on those same ten attachment
  states. Of 2,880 Basic Render Driver images, the other 2,870 were unchanged;
  the ten replacements were reviewed against the joins and copied from the run.
- An attachment composition wider or taller than the Windows 16,384px texture
  limit could leave the preview blank. The display now uses bounded, overlapping
  source tiles with opaque theme-matted pixels; the original transparent pixels
  remain available for annotations and redactions. NVIDIA GPU checks cover
  19,200×160 and 160×19,200 compositions in both themes, including translucent
  joins, the final tile, annotations, 25% zoom scrolling and texture eviction.
  A small 2D reconstruction test checks tile intersections without allocating a
  large surface. Long annotation patches also respect the texture
  limit; very large two-dimensional GPU compositions and memory use remain to
  be measured.
- A small selection at a display edge could place its size label outside the
  overlay; near the bottom, the label could also cover the selection. The label
  now snaps inside the viewport and moves above small bottom-edge selections.
  Electron's selection label is centered without horizontal clamping in source;
  native cross-shell edge behavior has not been compared.
- At 125% and 150% simulated scaling, independently rounded dim regions left a one-pixel
  vertical strip of the desktop undimmed. The right region now shares the
  selection edge, and pixel scans check fractional selection positions.
- The GPUI frame's drop shadow darkened the whole selected region by 35%.
  A separately painted outside border preserves the outline without changing
  selected pixels; its position and width snap to the frame at fractional DPI.
- The recording handoff and window-picker scrims could leave the last display
  column or row undimmed at fractional scale. Their shared edges now snap to
  device pixels and their outer strips anchor to the viewport.
- The color picker's hardcoded card height clipped it near a display edge, and
  its marker shadow darkened the sampled pixel by 40%. Placement now uses the
  measured card and a separately painted outline preserves the source color.
  Tests cover pointer clearance at five simulated scales and click-to-copy from
  a four-color frame. If a 200×180 window cannot fit both the full card and
  its cursor gap, the card takes the least-clipped placement with the gap.
- A failed color-frame capture left the all-in-one overlay without a redraw.
  It now exits through the normal picker shutdown path. The isolated daemon
  fixture writes a partial file and fails the request; the test verifies the
  stopped-state notification, restored overlay pixels, error message and cleanup.
- All-in-one toolbar tooltips opened above the top edge and clipped their labels.
  They now open below their controls, matching Electron. Normal and 400×300
  overlays, including selected Capture text mode, check all three tooltip bounds
  and clipping masks at each matrix scale.
- Escape closed the all-in-one capture overlay while its target menu was open.
  It now dismisses the menu first and returns focus to its trigger. A second
  Escape closes the overlay; keyboard input can reopen the menu and select a target.
- Selecting a target left keyboard focus on the disappearing menu, and clicking
  a menu item could start a full-screen capture behind it. Selection now returns
  focus to the trigger after the activation key is released; pointer selections
  return it immediately. The fixture exercises repeated Enter/Space keydowns,
  pointer and outside presses, and selection without an underlying capture. A
  window-deactivation test clears state if a key release is lost.
- A history grid at simulated 175% and 456 physical pixels wrapped its second
  card onto a new row. The cards now fit the snapped available width. The unit
  test sweeps every physical width corresponding to 140–400 logical pixels at
  100%, 125%, 150%, 175% and 200%, and verifies the scale after each resize.
- At 140px the two-column history cards clipped their actions, and the list
  clipped its labels. The grid now uses one column with room for the selection
  ring; the list stacks its thumbnail and details. Keyboard navigation follows
  the displayed grid, and a visible selection stays in view across resize,
  scale and layout changes without pulling back a selection the user scrolled
  away from. The visual fixture checks all four sides of the selection ring.
- History could open partly outside a display when its tray icon was near an
  edge. GPUI now fits the popover to the work area and converts macOS
  secondary-display bounds as the tray menu does. Geometry tests cover offset
  displays, 140–400px window widths, both themes, simulated 100/125/150/175/200% scaling,
  grid and list selection, and wrapped header and filter controls. Narrow list
  actions now sit over the thumbnail so labels remain readable, with feature
  icons wrapping as needed. Grid tooltips align inward according to their column.
  The local visual fixture checks complete reveal and delete tooltips in grid
  and list states, and deletes an isolated item without opening an editor. Its
  144 NVIDIA and 144 hosted history images were reviewed; physical monitors
  remain unverified. Electron's horizontal placement remains unclamped.
- Failed editor-attach captures also left a partially written temporary PNG.
  The attachment path now removes it before restoring the editor.
- A competing capture or second attachment could replace the active attachment
  selector and leave its editor hidden. Those requests now leave the attachment
  in place, and a selector that cannot open clears the pending operation and
  reactivates its editor. The fixture covers competing intents and unavailable
  displays with freeze on and off.
- Choosing an attachment edge could also begin an annotation on the editor
  canvas. Edge presses now stop at the overlay; both mouse buttons work, and
  the edge and camera icon highlight on hover as in Electron. The workflow
  fixture presses the actual strips and rejects an unintended draft. The four
  edges now have names, keyboard activation and in-bounds focus borders. Windows
  accessibility inspection exposed all four buttons; invoking the right edge
  opened the selector, and Escape returned to the isolated test editor.
- The editor retained its original zoom after an attachment expanded the
  composition, clipping the result. The fit cache now includes composition
  dimensions, matching Electron's recalculation dependencies. Attachment checks
  require the entire composed rectangle to remain inside the editor viewport.
- Enlarging a pin upscaled its image beyond the original display size. Pin
  images now cap at their natural logical size and remain centered when resized.
  The shared drag area's inset also rescaled edge pixels; pin content now has
  zero inset and matches the annotated export.
- Closing a pin restored annotations only through persisted history, losing
  edits for untracked images or when history was disabled. The pin now retains
  the current annotations, wallpaper and layers in memory for restoration.
- OS close requests bypassed the pin's hover Close handler and never restored
  the editor. Both close paths now consume the same in-memory state once.
- The native timer panel had an empty accessibility window name. It is now named
  "Poratake timer capture", making the countdown identifiable by users and tools.
- GPUI's first activation restored a hidden Windows selector's initial placement
  after exact client bounds were applied, shifting its client origin four pixels
  above the display. The activation observer reapplies the display bounds, keeping
  area and timer output aligned with the selection.
- GPUI annotation completion also discarded the final release coordinate.
- Scheduling pointer updates requested an animation frame outside GPUI painting,
  causing a native debug panic during drawing.
- Shape previews differed from saved PNGs; cached patches could mix captures
  having otherwise identical annotation geometry.
- Narrow editor options could push Close outside the window.
- Countdown disabled Start while leaving focus on it, preventing Escape from
  reaching the control bar. Countdown now moves focus to the bar.
- The recording popup had no native or accessibility window title. It is now
  named "Poratake recording controls" and can be identified by desktop tools.
- Pre-recording audio and camera choices now use Electron's global or
  all-in-one preference scope. The fixture exercises ordinary controls,
  remembered and temporary all-in-one choices, live recording changes, and
  area, window and screen handoffs. Closing the bar also hides its camera
  preview; physical media-device verification remains in the backlog.
- The all-in-one toolbar's full-width positioning container intercepted clicks
  outside its visible controls. The fixture now starts a recording selection
  beside the toolbar and verifies that a toolbar click does not start a box.
- Keyboard sounds enabled at zero volume did not become audible in preview when
  the volume increased. Audio rebuilds could also replace an undone timeline with
  a stale result. Both transitions have regression coverage.
- Speed edits weakened the original tone in preview and export. Constrained
  overlap alignment preserves the tested pitches while limiting source drift;
  regressions cover short sounds, including at silence and input boundaries.
- Cut All split only the first overlapping drawing and audio clip. It now splits
  each eligible clip, matching Electron and preserving the audio source trims.
- Shared overlay buttons and recording device triggers used white foregrounds
  on light toolbars in both shells. Foreground and hover colors now follow the
  active theme. Electron light/dark computed styles were checked; browser
  screenshot and hover-state evidence remain unavailable in this environment.
- The recording bar and capture-preview controls now use native hover tooltips
  on Windows and macOS to let their labels extend beyond the small window.
  An isolated Windows tooltip painted readable text outside a 52px owner window;
  focused Windows headless tests exercise GPUI pointer hit-testing, the 150 ms
  presenter request, leave cancellation and keyboard dismissal. A hosted
  real-app probe displayed and measured the Windows recording tooltip, then
  verified dismissal and re-entry after the Windows backend began dispatching
  pointer-exit input. macOS tooltip appearance still needs native verification.
- A keyboard-opened all-in-one target menu could leave its trigger pressed after
  focus moved. The shared keyboard release now runs in the capture phase; the
  vendored control test reproduces the focus transfer and stopped propagation.
- The missing-thumbnail video placeholder now fades when its controls appear,
  keeping the film icon out of the Copy and Edit controls. Idle and hover
  pixel checks reached the visual baseline gate across both Windows renderers.
- Windows change-driven capture wrote every frame with a fixed short duration,
  truncating static takes. Pending samples now receive their real duration before
  submission; finalization and pause accounting use the capture clock.

## Remaining bugs and verification backlog

Entries distinguish observed defects from missing evidence. An unverified path
is not a demonstrated product defect; reproduce before changing its behavior.

Selecting through the empty interior of an outlined rectangle or circle now
reaches the visible annotation underneath; line hit testing follows its stroke
rather than its bounding box. Headless tests cover overlaps, rounded corners and
zoom-aware tolerance. Straight and curved arrows hit their rendered shafts and
heads. Pen and highlight hits follow their rendered outlines within a per-click
work budget. Over-budget marks fall back to their old bounds when no confirmed
hit exists; a confirmed lower hit takes priority over an uncertain upper mark.
This can still select empty bounds or let a lower mark win over a visible upper
stroke. The outline builder now stops at the actual vertex budget instead of a
worst-case estimate, allowing tested 4,096-point smooth strokes. A release-mode
scratch probe measured the capped builder at 0.15 ms median for that smooth
stroke and 0.85 ms before rejecting a 4,096-point serpentine; the earlier
unbounded 100,000-point probe took 107 ms and about 309 MB.
An editor pointer-event test also drags a filled shape through a U-shaped pen
gap and then selects the visible pen stroke, without moving the wrong annotation.
A decoded-video regression also checks that a blackout covers an earlier filled
annotation in both preview and export at four timeline positions.
[Hosted CI at `45f1adbf`](https://github.com/Porabuild/Poratake/actions/runs/43578561127)
passed all active jobs, including 16 Windows visual profiles and 2,912 baseline
comparisons. Windows packaging was skipped.

Windows screenshot Save As now backs up an existing file before writing through
its original handle and restores it on a returned write error. The backup is kept
and its path reported if restoration fails. The handle preserves the original
security descriptor; encrypted originals require an encrypted backup. Compatible
readers can see an incomplete write, and a process or power failure can still
leave a partial destination with an undiscovered temporary backup. macOS and
Linux use same-file recovery for returned write failures as well; 13 standalone
Linux filesystem tests passed in WSL, and the integrated Linux job passed in
[hosted CI](https://github.com/Porabuild/Poratake/actions/runs/43605856597). Both
macOS ACL fixtures passed in [hosted CI](https://github.com/Porabuild/Poratake/actions/runs/43613554180),
including inherited ACL removal and an Everyone delete-child grant. Unix recovery is not crash-atomic,
and it refuses overwrites where a private backup cannot be secured. This includes
some writable ACL-managed directories owned by another user; write-only existing
files also fail before modification because they cannot be backed up.
A hosted Windows scratch probe found that unprivileged `ReplaceFileW` removed a
seeded audit ACE, so it is not used for this save path.

[An earlier run](https://github.com/Porabuild/Poratake/actions/runs/43610852822)
stopped the native Windows attachment test before capture: another
runner window covered the fixture after the editor hid. The test now places the
fixture directly below the selector.
The [next run](https://github.com/Porabuild/Poratake/actions/runs/43613554180)
reached the first live attachment but stopped at the owned Save dialog because
its filename control was not exposed by the test's UI Automation query. The
test now uses the dialog's filename shortcut and requires the exact output path.
The [following run](https://github.com/Porabuild/Poratake/actions/runs/43660349585)
passed two native freeze-off attachments with exact output pixels, confirmed
editor hide and restoration, and verified cancellation preserved the saved
composition. It later failed while foregrounding the record-area selector.

The [Windows run at `50f9db03`](https://github.com/Porabuild/Poratake/actions/runs/43603549394)
stopped before the visual matrix: its pre-marker preview comparison counted 493
small color changes against a limit of 377. The harness now tolerates those small
changes while checking the actual initial marker against a decoded frame; the
corrected workflow still needs hosted verification.

In an isolated native Windows editor, the inset slider responded to a pointer
click. Two control-tool set-value attempts left its value unchanged, but that
tool documents set-value for text fields, not numeric range controls. The
Windows adapter maps a range-value request to the numeric action handled by the
slider; delivery through the operating system remains unverified.

Unframed layered annotations now render after attachments in the tested path,
including the inset rectangle and cross-boundary redactions above. Verify their
ordering where annotations overlap, and with frames and balance; compare the
complete workflow directly with Electron.
The 4K attachment preparation cost above also needs interactive and
slower-hardware measurement.

Selected highlights and committed redactions now draw their outlines above all
annotations, so later marks cannot cover them. This selection order deliberately
differs from Electron; saved pixels and highlight blending remain unchanged.
Light and dark fixtures check all four sides of blackout, blur and pixelate
outlines, an overlapping highlight, and plain and inset-attached images at
simulated 125% DPI. Rectangles now use an outside-only outline above later marks
so their stroke and fill remain visible. Light and dark fixtures check overlapping
thin and thick rectangles, all four sides, sampled stroke and fill pixels, export,
and plain and inset-attached images at simulated 125% DPI. Circles use the same
outside-only order; the fixtures check all four sides, the interior, thick stroke,
and unchanged export for thin and thick circles. At fractional zoom, antialiasing
can blend the ring and a thin stroke into the same device pixel. Verify selection
outlines for other overlapping annotation types and on physical displays.

An isolated Electron canvas probe applied its current redaction operations to the
same unredacted inset-attachment PNG used by GPUI. Across 4,136 interior pixels
per effect at simulated 125% and 200%, blackout and pixelation matched exactly.
Blur differed by up to five and four channel levels respectively; 487 and 602
pixels differed by more than two levels. This checks the effect functions, not
the complete Electron editor workflow.

A separate 1× offscreen probe compared Electron's video blur operations with
the GPUI blur function on a manually composed translucent checker and wallpaper.
Across 1,088 interior pixels, the mean RGBA-channel difference was 0.480 and
the maximum was three; across 64 pixels at a flat clip edge, the mean was 0.473
and the maximum was two. Pixels just outside the annotation matched. This does
not exercise rounded-edge antialiasing, playback or the full export pipeline.

A CPU regression compares complete GPUI preview and export pixmaps for two
fractional annotation origins, both rectangle/blackout input orders and five
coordinate scales. A separate Chrome 157 proxy matched the saved Electron corner
sample at 1×, but 27 of 120 corner samples differed from GPUI by more than two channel
levels, up to 27. This proxy does not establish parity with Poratake's pinned
Electron version at the other positions and scales. An offscreen fixture using
the pinned Electron 43.4.0 reproduced the 1× corner sample and the tested export
order; it does not exercise the complete editor workflow.

A 1024px-wide Windows/Linux scroll fixture stops after the first frame when a
21×16px footer block changes. A wider structural footer candidate recovered that
case but stopped a valid periodic page early, so it was not adopted. Rounding the
2% tolerance up recovered the fixture but introduced 33 early stops in the
5,412-case periodic corpus. A narrower contiguous-band fallback avoided those
stops but misclassified a no-footer page and shifted an animated block by 16px,
changing 672 pixels relative to the baseline stitch. Neither candidate was applied.
A three-frame uniqueness prototype also changed 672 pixels in that no-footer case.
Separate periodic-content models with 16px and 24px scroll steps produced identical
three-frame pixels but different expected heights. Additional frames alone cannot
resolve every sequence; a safe lookahead would require lifecycle changes and must
retain the accepted prefix when evidence remains ambiguous.

A separate 64×240, 8px-step fixture with a fixed 8px footer accepts four frames
without trimming that footer, producing 1,536 incorrect pixels. An aligned
stationary-suffix conflict guard avoided that output but stopped a valid
full-width animated no-footer control early, so it was not adopted. A fixed-footer
page and an animated no-footer page can produce the same four captured frames
even though their first-frame-preserving stitched outputs differ. This is synthetic
evidence; real pages and broader animation patterns remain unverified.

A 1024×400 two-frame fixture with an 8px fixed header and 40px fixed footer
also produced a false overlap: the stitch was 408px tall instead of 680px,
with 49,152 mismatched pixels. The equivalent footer-only fixture matched
exactly. A hosted Windows test now exercises the footer-only case through native
wheel input, capture and pixel-by-pixel PNG verification; its first CI step
passed in [run `43586226485`](https://github.com/Porabuild/Poratake/actions/runs/43586226485).
The four-frame extension passed its native step in
[run `43590070286`](https://github.com/Porabuild/Poratake/actions/runs/43590070286);
all active jobs passed, including the visual matrix.
A candidate header fix corrected that fixture but corrupted 256 pixels
when an animated body block touched the footer, so it was not adopted. Native
fixed-header stitching remains open. A stricter scratch variant also inferred a
footer on a no-footer page with a one-pixel animated marker, producing 40 wrong
pixels; it was not adopted either.
Another scratch pair produced byte-identical 1024×400 captured frames for a
fixed-footer page and a changing-body page, but their expected 680px stitches
differed in 40,960 pixels. Pixel matching alone cannot distinguish that pair.
A proposed guard stopped the corrupt header fixture, but also stopped a valid
8px scroll whose naturally repeated top and bottom rows looked like fixed UI;
that guard was not adopted.

Capture-preview thumbnails now decode once on a background worker. On synthetic
noisy PNGs, 12 warm samples measured median generation at 70 ms versus 135 ms
for 4K, and 283 ms versus 561 ms for 8K. Normal and blurred thumbnail bytes
matched the previous path in the tested fixtures. Peak measured allocation was
approximately unchanged; these measurements exclude GPUI upload and full-app
interaction, so they do not establish end-to-end responsiveness.
Failed screenshot or recording deletion now reports an error and retains the
preview. A Windows headless button test covers failure and successful deletion;
native interaction remains unverified.

Saved video projects with empty or invalid timeline ranges now recover a usable
clip in memory while preserving valid ranges and the original saved file on open.
An isolated generated-video test checks the filtered timeline's preview against
export; real legacy projects remain unverified. Camera sidecar ranges are mapped
after normalization, and an explicitly saved empty camera track is preserved.
New recordings leave that track uninitialized until sidecar mapping, including
when the preview's Copy action exports directly. Generated-recording tests cover
both camera visibility and an explicitly empty track. Older GPUI recordings with
an empty saved track are ambiguous and remain unchanged.
Loaded camera ranges now use the same video-duration cleanup as timeline edits
and export. An isolated Mac editor test opens a four-second synthetic project
with an eight-second music lane and saved camera ranges at 3–6 and 6–7 seconds;
the displayed camera lane contains only 3–4, as in Electron, and opening leaves
the saved project bytes untouched. A later editor change can persist the cleaned
ranges. Tests also cover speed-adjusted duration, minimum clip length, invalid
duration and the enabled first-frame offset. A generated macOS video and camera
fixture tests a saved camera clip that would last less than 0.1 seconds after
clipping: the uncleaned preview visibly shows the camera on the final sampled
frame, while the normalized preview hides it and matches the decoded MP4 within
three mean channel levels. Loading and exporting leave the saved state bytes
unchanged. This is a file-only test, not native window or live camera evidence.

Changing a video clip's speed now retimes zoom, camera and drawing ranges as
Electron does; previously those overlays stayed at their old times. A synthetic
editor test checks the three tracks and confirms that one undo or redo restores
the entire edit. Boundary tests cover ranges before, inside and across a changed
clip, shortening and lengthening, and invalid speed input. On macOS, a generated
camera recording was saved and reopened after a speed change; five decoded MP4
frames matched the editor composition within a mean three levels per channel,
including camera visibility before and after the retimed ranges. This file-only
test does not establish live camera, capture or playback behavior.

Deleting or trimming video now bounds zoom, camera and drawing ranges to the
new duration in the same undo step. During a trim gesture, dragging the edge
back restores the ranges from the gesture's starting state. A synthetic editor
test covers deletion, trimming in both directions and undo/redo; an isolated
macOS file test saved and reopened a shortened project and compared three
decoded MP4 frames with the editor composition. Imported music is unchanged.

Repeated start-trim movements previously accumulated the pointer delta, and a
previously trimmed start could not be extended into the timeline gutter. A
headless GPUI pointer test now exercises both handles, reverses an end trim in
one gesture, and restores a start edge in a separate gesture into the gutter.
Handle presses retain the pointer's offset from the edge. These are simulated
events; physical pointer behavior and dragging beyond the window remain
unverified.

Zoom and camera clips now reject overlapping additions, moves and resizes, as
Electron does; touching edges remain allowed. A headless GPUI pointer test draws,
moves and resizes a zoom clip and rejects a crossing draw. Separate state tests
cover both track types, including rejected drags and adjacent ranges. These do
not verify physical pointer delivery.

Opening an edited video project previously reset the drawing ID counter, so a
new drawing could reuse a saved drawing's ID and deleting it could remove both.
New clip and annotation IDs now avoid IDs already in the project, including
saved annotation IDs. Splits also avoid IDs already on their track. Synthetic
tests reopen a project, add a drawing, delete and undo it, save and reopen again;
they also cover simultaneous drawings and collisions during cuts. Previously
saved projects that already contain duplicate IDs are not repaired automatically.

The empty or short video timeline previously shrank its lanes to the duration's
pixel width; with no video this was one point and clipped the empty-lane hint.
The lane now fills at least the available viewport, matching Electron's minimum
width, and hides the add-zoom hint when no video is loaded. The isolated macOS
renderer records the empty editor at 950 points [before](tests/native/evidence/macos-ui/video-empty-timeline-before.png)
and [after](tests/native/evidence/macos-ui/video-empty-timeline-after.png). A
headless geometry test checks empty and two-second timelines, and pointer tests
check that clicking beyond the video or with no video does not create a clip
outside its duration. These are synthetic renderer checks, not a physical display
or pointer test. After the change, the isolated 950-point macOS fixture passed
in light and dark appearance at simulated 2×, including its capture-preview,
history, toolbar and recording-control workflows.

An isolated macOS raster probe compared the existing rounded-blackout path with
generated SVG pixels at integer and fractional origins. The 2px-radius top-left
pixel over magenta was 191 in Tiny Skia 0.11.4 and 158 in headless Chrome 157;
an older GPUI test expected 169 without recording its source. At a half-pixel offset, the adjacent
sample was 223 in Tiny Skia and 189 in Chrome. A 4× supersampled Tiny Skia scratch
render produced 176 and 203 for those two respective samples. This isolates a
coverage difference from the surrounding annotation stack, but Chrome is not
the pinned Electron runtime and supersampling does not reproduce its pixels.
The [samples and probe sources](tests/native/evidence/macos-ui/blackout-corner-results.json)
are retained alongside the JSON; no production rendering change was made. The
rounded-corner mismatch was observed before the ordering changes below.

A later local macOS probe used the already-present Electron 43.4.0 runtime
(Chromium 150) with a hidden, isolated window. For a synthetic magenta rectangle
overlaid by a rounded blackout, its 1× top-left sample was `[158, 0, 158, 255]`
versus GPUI's `[191, 0, 191, 255]`; the source of the older GPUI test's expected
value of 169 has not been established. Across nine scale/offset cases,
62 of 576 sampled RGBA channels differed by more than two, by at most 34. Reversing
the order matched all sampled channels. [Electron](tests/native/evidence/macos-ui/blackout-order-electron-43.json)
and [GPUI](tests/native/evidence/macos-ui/blackout-order-gpui-20261116.json)
samples and [reproduction details](tests/native/evidence/macos-ui/blackout-order-probe-20261116.txt)
are retained. A separate simple SVG comparison found the same Electron pixels as
the previous Chrome proxy; 4× supersampling reduced but did not eliminate the
error. The probes recreate the export operations and do not exercise either full
editor or a physical display. No production rasterizer change was made.

A follow-up on the same Electron 43.4.0 runtime invoked the renderer's actual
annotation exporters through an isolated Vite fixture. All 18 scale, offset and
order cases matched the earlier Electron reproduction exactly, including the
rounded-blackout samples. The [actual-exporter samples](tests/native/evidence/macos-ui/blackout-order-actual-exporter-20261116.json)
and [probe instructions](tests/native/evidence/macos-ui/blackout-order-probe-20261116.txt)
are retained. This validates those export operations, not the complete Electron
editor, its native window or physical display pixels. The later ordering changes
below replace the compared composition sequence; the rasterizer coverage
difference itself still exists for rounded geometry.

The first actual-exporter fixture omitted Electron's initial rectangular blackout
pass. Adding it did not change the 18 earlier samples because the shape covered
those samples. A separate fractional standalone blackout exposed a different
issue: at the left boundary Electron saved black while GPUI retained 64/255 of
the white source. GPUI now aligns the base blackout to the saved composition's
pixel grid, and its preview patch includes the additional covered pixels. The
[boundary evidence](tests/native/evidence/macos-ui/blackout-fractional-boundary-20261116.txt)
records the Electron sample and isolated GPUI checks. This fix does not resolve
the separate rounded SVG corner rasterization difference above.

A second fractional case crossed the far pixel boundary. The previous width
rounding left a touched right-edge pixel white in GPUI and at 229/255 in
Electron; a bottom-edge pixel also retained 229/255 in Electron. Both screenshot
export paths now round the far edge outward; sampled pixels beyond the touched
area remain unchanged. [Electron before/after samples](tests/native/evidence/macos-ui/blackout-far-edge-electron-20261116.json)
come from its actual export helper and SVG exporter in an isolated offscreen
window. GPUI tests cover the saved boundary, preview patch, clipped and empty
rectangles. An isolated GPUI Metal workflow in light and dark mode also checked
the right and bottom export pixels and compared its displayed preview with the
saved PNG. These generated cases do not prove all live redaction geometries.

Electron's screenshot preview had a separate copy of the older rounding rule,
so it could still leave a far-edge pixel visible after export was fixed. Preview
and export now share the same coverage helper. A component test exercises the
preview at simulated 1× and 2×. In an isolated Electron 43.4.0 offscreen window,
the actual preview component and exporter produced black at the tested far-edge
pixels and white just outside them at 1×; the [samples](tests/native/evidence/macos-ui/blackout-preview-electron-20261116.json)
use a generated image. An attempt to use DevTools device emulation crashed the
isolated Electron process, and a separate fixture-only ratio override stalled
and was terminated. Neither establishes a product defect or a physical 2×
result. The preview and export use different pixel grids when their scale factors
differ, so broader pixel equality remains unverified.

A generated Electron 43.4.0 comparison now confirms a mismatch when those
grids differ. It rendered the actual blackout preview component and SVG over a
200×160 white source displayed at 100×80, then reconstructed the 2× export
using the actual blackout and annotation exporters and downscaled it with Canvas
image smoothing. Selection guides were removed from the preview SVG before the
comparison. At the 1× preview, 52 of 8,000 pixels differed by more than two
channel levels, with a maximum difference of 191; four mid-edge samples were
black in the [preview](tests/native/evidence/macos-ui/blackout-scale-preview-20261116.png)
and 128/255 gray in the [downscaled export](tests/native/evidence/macos-ui/blackout-scale-export-20261116.png).
The [measurements](tests/native/evidence/macos-ui/blackout-scale-comparison-20261116.json)
and probe are retained. All 676 pixels touched by the selection were opaque
black in the native-resolution export; the difference appears after resampling.
It does not exercise the full
editor, its interaction guides, other compositions or a physical display. The
preview/export difference was reproduced in the full editor below and has since
been addressed there.

An offscreen Electron 43.4.0 screenshot-editor fixture now reproduces the
scaled blackout difference through the actual save shortcut. It uses a generated
200×160 white PNG, a 100×80 editor image, renderer device pixel ratio 1 and a
200×160 offscreen capture; the dashed editor guide is hidden for comparison.
In the [preview](tests/native/evidence/macos-ui/blackout-editor-scale-preview-20261116.png),
26 pixels immediately below the blackout appear fully black but are white in
the [saved PNG](tests/native/evidence/macos-ui/blackout-editor-scale-export-20261116.png).
This could give misleading assurance about adjacent real content. All 676
native pixels touched by the selected rectangle were black in the saved PNG,
which contained only the synthetic white source and black redaction colors.
The [measurements](tests/native/evidence/macos-ui/blackout-editor-scale-comparison-20261116.json)
also show 716 differing pixels at the outer image perimeter and 250 near the
blackout; a repeat produced the same measurements. This verifies neither a
physical display nor exclusion of live capture controls.

The fixture also reopened that PNG in a second offscreen editor at the same
displayed size. Its [reopened image](tests/native/evidence/macos-ui/blackout-editor-scale-reopened-20261116.png)
differed from the annotated preview at 416 pixels, maximum 158 levels; 192 were
fully black in the preview and lighter after reopen, though none was fully
white in the reopened display. The [same-size measurements](tests/native/evidence/macos-ui/blackout-editor-reopen-comparison-20261116.json)
reproduced on a second run.

A local trial rendering the redaction layer at the source scale instead of the
renderer scale reduced the same-size maximum difference to 22, but still left
312 differing pixels in the [trial preview](tests/native/evidence/macos-ui/blackout-editor-scale-trial-preview-20261116.png).
Suppressing the additional SVG blackout fill reduced the
single-blackout comparison to within one level, but broke annotation order: a
red rectangle underneath the blackout appeared in 388 preview pixels while the
saved selection was black. The [baseline ordering result](tests/native/evidence/macos-ui/blackout-editor-order-baseline-20261116.json),
[trial measurements](tests/native/evidence/macos-ui/blackout-editor-order-trial-20261116.json)
and [trial preview](tests/native/evidence/macos-ui/blackout-editor-order-trial-preview-20261116.png)
are retained. Neither trial was kept in production.

Electron's screenshot editor now previews the same native-pixel composition it
exports. Geometry is shared for the image, aspect ratio, frame, inset and
attachments, and the compositor applies vector annotations and redactions in
their recorded order. The generated offscreen editor comparison now finds zero
same-size differences from the reopened PNG in the original blackout case,
including the previously misleading edge. The [preview](tests/native/evidence/macos-ui/electron-shared-blackout-preview-20261116.png),
[saved PNG](tests/native/evidence/macos-ui/electron-shared-blackout-export-20261116.png),
[reopened image](tests/native/evidence/macos-ui/electron-shared-blackout-reopened-20261116.png)
and [measurements](tests/native/evidence/macos-ui/electron-shared-blackout-result-20261116.json)
are retained. A shape underneath remains covered;
an earlier shape is filtered by pixelation or blur, and a later shape remains
visible. Selection, dragging and resizing are exercised before save and reopen.
The fixture also covers generated attachment, frame, zoom and transparency
cases, and nine annotation types remain selectable through the interaction
layer. The [interaction](tests/native/evidence/macos-ui/electron-shared-interaction-20261116.json),
[attachment](tests/native/evidence/macos-ui/electron-shared-attachment-20261116.json),
[aspect-ratio](tests/native/evidence/macos-ui/electron-shared-aspect-20261116.json),
[pixelation](tests/native/evidence/macos-ui/electron-shared-pixelate-before-20261116.json),
[blur](tests/native/evidence/macos-ui/electron-shared-blur-before-20261116.json)
and [selection](tests/native/evidence/macos-ui/electron-shared-hits-20261116.json)
measurements are retained. An invalid SVG or attached image produces a preview error and prevents
the incomplete composition from being saved. The previous preview stays visible
with an updating indicator while a new one is composed.

Noise now uses a repeatable varied pattern: a generated 50%-noise case that
previously differed at 2,104 displayed pixels matches its reopened image in the
offscreen fixture ([measurements](tests/native/evidence/macos-ui/electron-shared-noise-20261116.json)).
The background is cached while noise is enabled; a separate generated fixture
checks [cache invalidation](tests/native/evidence/macos-ui/electron-shared-cache-20261116.json).
A local 4K
synthetic canvas render and forced pixel readback measured roughly 14–27 ms
without noise and 33–48 ms for a warm noisy background; the first noisy render
was about 689 ms. These are not interactive latency or memory measurements, and
large redactions, slower hardware and physical displays remain unverified. The
offscreen Electron harness has occasionally timed out during load and succeeded
on retry; a framed top-attachment case timed out on its sizing condition in the
latest run after an earlier pass. None of these generated tests verifies live
capture or recording.

After Electron switched to annotation-order composition, GPUI still applied
all screenshot redactions before vector annotations. A generated regression
confirmed that a red stripe under a later pixelation stayed fully red in the
saved GPUI image. GPUI now applies each redaction and highlight in annotation
order in both export and preview; selection hit testing follows the same order.
The old rounded-blackout comparison above no longer describes a shape followed
by blackout: the rectangular blackout now covers that shape first, as in the
current Electron composition. This does not establish equal antialiasing for
other rounded shapes.

The native-scale cached blackout patch also rounded its width and height
independently of the fractional origin, which could omit the last covered row
or column. Its bounds now round the far edges, with tests for reversed and
clipped rectangles and empty selections. An isolated GPUI Metal fixture compares
the entire generated editor image with its saved and reopened PNG for both
orders of pixelation and blur over a red stripe. In light and dark appearance at
simulated 100% and 200% scale and 73%, 100% and 137% zoom, all 24 cases per
theme match within one channel level; the stripe is filtered when below the effect and remains red when
above it. The [dark](tests/native/evidence/macos-ui/ordered-redactions-dark-20261116.txt)
and [light](tests/native/evidence/macos-ui/ordered-redactions-light-20261116.txt)
measurements, plus the 2× pixelation [preview](tests/native/evidence/macos-ui/ordered-pixelate-preview-20261116.png)
and [reopened image](tests/native/evidence/macos-ui/ordered-pixelate-reopened-20261116.png),
are retained. The [blackout matrix](tests/native/evidence/macos-ui/ordered-redactions-blackout-20261116.txt)
also matches within one level at four zooms and three simulated scales in dark
appearance. These generated tests do not verify physical input, captured content,
larger patches or all wallpaper and attachment combinations.

A generated large-image probe exposed a preview/export mismatch for
both pixelation and blur. At 25% zoom, a 4096×2160 image with a full-image
redaction exceeded the preview patch budget. Pixelation differed from the saved
and reopened PNG at 533,724 of 552,960 displayed pixels by more than two channel
levels; blur differed at 15,034 pixels. An unredacted control matched exactly,
and both effects changed every one of 1,024 sampled source pixels. An otherwise
identical 3840×2160 pixelation case, below the budget, matched the reopened PNG
exactly. Both appearances were checked using generated content. The
[measurements](tests/native/evidence/macos-ui/large-redaction-preview-mismatch-20261117.txt),
pixelation [preview](tests/native/evidence/macos-ui/large-redaction-pixelate-preview-20261117.png)
and [reopened output](tests/native/evidence/macos-ui/large-redaction-pixelate-reopened-20261117.png),
and blur [preview](tests/native/evidence/macos-ui/large-redaction-blur-preview-20261117.png)
and [reopened output](tests/native/evidence/macos-ui/large-redaction-blur-reopened-20261117.png)
are retained. The oversized path reduces the raster scale before applying the
effect; the export applies it at source resolution. The probe does not verify
live capture, other zooms or physical DPI.

The large-image preview now replays screenshot paints at source resolution in
bounded tiles, keeping the pixelation grid and blur dependencies aligned with
the full composition. It clips large previews to the visible editor region at
native scale and invalidates cached patches when that region moves. In the
generated 4096×2160 Metal workflow, both appearances now match the saved and
reopened PNG exactly for pixelation at 25% and 100% zoom; blur differs by at
most one channel level at 25% and matches exactly at 100%. The 25% comparison
covers 552,960 displayed pixels and the 100% comparison covers a 318,500-pixel
visible region. Unit tests also compare tiled and full-resolution rendering
with ordered effects, transparent pixels, a preceding shape, fractional
offsets and forced small tile budgets. The [measurements](tests/native/evidence/macos-ui/large-redaction-bounded-composition-20261117.txt),
25% pixelation [preview](tests/native/evidence/macos-ui/large-redaction-fixed-025-pixelate-preview-20261117.png)
and [reopened PNG](tests/native/evidence/macos-ui/large-redaction-fixed-025-pixelate-expected-20261117.png),
and 100% blur [preview](tests/native/evidence/macos-ui/large-redaction-fixed-1-blur-preview-20261117.png)
and [reopened PNG](tests/native/evidence/macos-ui/large-redaction-fixed-1-blur-expected-20261117.png)
are retained. A dependency footprint that cannot fit the existing patch budget
still falls back to the older preview path; unusually deep overlapping effects
remain unverified. These generated checks do not verify live capture or
physical display scaling.

A generated large-window check then exposed a second budget boundary: with a
4600×2600-point editor, the 4096×2160 pixelation preview differed from its
reopened PNG at 8,277,799 of 8,575,200 compared pixels. The displayed output
is now split into bounded, individually cached textures on physical-pixel
boundaries. Both opaque effects match exactly across the same region in light
and dark appearance. A separate varying-alpha source exposed an older preview
conversion mismatch even in a normal-size editor: pixelation differed at
29,904 of 318,500 pixels, by up to 11 channel levels. Preview conversion now
uses export's rounding for original images while preserving the rounding used
to reconstruct composed backdrops. With the transparent source, both effects
match the reopened PNG within one level across the large comparison region in
both appearances. See the [measurements](tests/native/evidence/macos-ui/large-output-transparent-redaction-20261117.txt),
opaque pixelation [before](tests/native/evidence/macos-ui/large-output-pixelate-before-20261117.png)
and [after](tests/native/evidence/macos-ui/large-output-pixelate-after-20261117.png),
and transparent pixelation [preview](tests/native/evidence/macos-ui/large-output-transparent-pixelate-preview-20261117.png)
and [reopened PNG](tests/native/evidence/macos-ui/large-output-transparent-pixelate-expected-20261117.png).
Transparent effects also match within one level at 25% zoom and simulated
100%, 125% and 200% display scale. The blackout, effect-order, attachment and
pixelation Metal regressions pass in both appearances after the change. These
scratch-editor checks do not verify a physical large display or live capture.

After the bounded-composition change, the generated macOS Metal regressions for
scaled blackout, redaction ordering, framed attachments, clipped and transparent
pixelation, and balanced attachment seams also pass in both appearances. The
[rerun record](tests/native/evidence/macos-ui/redaction-regression-after-bounded-composition-20261117.txt)
covers 180 cases per appearance across simulated 100%, 125% and 200% display
scales. This verifies the generated editor and saved-image comparisons only;
it does not establish the result of a live capture or physical DPI change.

The large-image workflow now also dispatches a simulated Command-drag to both
the annotated editor and reopened PNG, asserts that the image moved, and
compares their visible regions after panning. In both appearances at simulated
100%, 125% and 200% display scale, pixelation and blur match within one channel
level at 100% zoom. At 25% zoom, the complete displayed image also matches
within one level at simulated 125% and 200%. The [measurements](tests/native/evidence/macos-ui/large-redaction-pan-dpi-20261117.txt)
cover generated content and a 1280×738-point scratch editor; physical input,
real display transitions and actual captures remain open.

The framed-attachment Metal workflow now also places a narrow red annotation
before and after blur and pixelation crossing the primary/right-attachment seam.
For macOS light and dark frame styles, translucent source content, both app
appearances and simulated 100%, 125% and 200% scale, all 72 generated cases
match their saved compositions within two channel levels across the whole stage.
The redaction changes pixels on both sides of the seam, filters the earlier
annotation, and leaves a later annotation red. The [dark](tests/native/evidence/macos-ui/ordered-framed-redactions-dark-20261116.txt)
and [light](tests/native/evidence/macos-ui/ordered-framed-redactions-light-20261116.txt)
measurements and representative 125% dark [preview](tests/native/evidence/macos-ui/ordered-framed-redaction-dark-preview-20261116.png)
and [export over the editor matte](tests/native/evidence/macos-ui/ordered-framed-redaction-dark-expected-20261116.png)
are retained. These use generated editor content and do not verify real
capture, physical DPI, selection input or all attachment and wallpaper layouts.

The equivalent GPUI Metal fixture found a separate sampling boundary mismatch
at simulated 2× scale and 137% zoom: the [preview](tests/native/evidence/macos-ui/scaled-blackout-before-preview-20261116.png)
and [reopened PNG](tests/native/evidence/macos-ui/scaled-blackout-before-reopened-20261116.png)
differed on 73 pixels in one row below the blackout, by at most 19 levels. The
preview was white where the reopened output was slightly darker; the underlying
red annotation remained covered. The aligned preview patch used the fractional
annotation bounds even though export rounds the blackout out to source pixels.
It now uses the same pixel-covered bounds as the raster patch. In light and dark
appearances, the whole displayed image matches its reopened PNG within one
channel level at 50%, 73%, 100% and 137% zoom and simulated 100%, 125% and
200% scale. The fixed dark-mode [preview](tests/native/evidence/macos-ui/scaled-blackout-after-preview-20261116.png)
and [reopened PNG](tests/native/evidence/macos-ui/scaled-blackout-after-reopened-20261116.png)
are retained. The selected source pixels are black in the PNG, and the underlying
shape is hidden. This generated fixture does not establish live capture or
physical display behavior.
The [current-branch recheck](tests/native/evidence/macos-ui/scaled-blackout-current-20261117.txt)
also passed all 24 light and dark cases after the later editor changes, with a
maximum difference of one level.

The GPUI Metal fixture exposed a separate image sampling mismatch at 73% zoom,
even without annotations, for opaque and translucent generated sources. The
primary image and simple wallpaper were snapped and scaled separately. The
preview now uses the existing composed-backdrop path for that case and includes
the source image identity in its cache key. In the generated dark-mode 114×99
composition, the [previous preview](tests/native/evidence/macos-ui/scaled-backdrop-before-preview-20261116.png)
and [reopened export](tests/native/evidence/macos-ui/scaled-backdrop-before-reopened-20261116.png)
differed by more than two levels at 115 pixels, with a maximum channel difference
of 59; the [new preview](tests/native/evidence/macos-ui/scaled-backdrop-after-preview-20261116.png)
and [reopened export](tests/native/evidence/macos-ui/scaled-backdrop-after-reopened-20261116.png)
matched at every pixel. The whole unannotated composition and the redaction
region also passed generated comparisons at 73% and 150% zoom in light and dark
appearances. This is not a physical-display or live-capture verification. A
local five-run debug-build 4K backdrop microbenchmark showed a warm generation time of
about 145–173 ms with composition versus 78–94 ms without it; it excludes GPU
upload and does not measure interactive behavior on other hardware. Performance
and broader wallpaper/annotation combinations remain to be checked.
The editor now borrows its existing RGBA source when converting it for the
backdrop, inset-color detection and export, avoiding one temporary 31.6 MiB
source copy for a 3840×2160 image. A non-RGBA source still uses the conversion
fallback. Alternating local debug timings were too variable to establish a
reliable latency improvement, so the 4K performance concern remains open.
The full dark-mode GPUI parity workflow subsequently completed at simulated 1×,
including generated capture and recording handoffs, screenshot save/reopen,
large attachments and wallpaper cases at simulated 1.25× and 2×. It used the
fake daemon and generated media; it does not establish live capture, real
device access or physical-display behavior. The GPUI editor unit suite passed
334 tests with two ignored, and the same-size source replacement test and
parity-feature Clippy check passed.

A transparent rounded screenshot with inset, shadow, blur, blackout and
pixelation exposed a separate reopen difference. A plain GPUI image used the
surface color behind transparent pixels, while the wallpaper preview used the
editor background. The plain image and redaction preview now use that same
background; Electron's screenshot canvas is transparent over its background.
In a generated dark-mode 73% comparison, the [previous preview](tests/native/evidence/macos-ui/transparent-reopen-before-preview-20261116.png)
and [reopened PNG](tests/native/evidence/macos-ui/transparent-reopen-before-reopened-20261116.png)
differed by more than two levels at 7,572 of 13,516 pixels, maximum 11. The
[new preview](tests/native/evidence/macos-ui/transparent-reopen-after-preview-20261116.png)
and [reopened PNG](tests/native/evidence/macos-ui/transparent-reopen-after-reopened-20261116.png)
matched at every pixel. The generated Metal fixture also passed at 100% and
150% zoom in both appearances at simulated 125% display scale, and checked a
plain partially transparent image with redactions. The offscreen Electron
43.4.0 screenshot editor, using the fixture preload and the same generated PNG,
shows its transparent corner over the editor background in both [dark](tests/native/evidence/macos-ui/transparent-editor-electron-dark-20261116.json)
and [light](tests/native/evidence/macos-ui/transparent-editor-electron-light-20261116.json)
appearances. Neither probe establishes physical-display or live-capture behavior.
The GPUI comparison also matched an eight-pixel band around the composition in
both appearances at the three tested zooms. The full dark-mode synthetic GPUI
workflow passed after the change.
The restored-pixelation fixture also passed after the matte change in both
appearances at simulated 100%, 125% and 200%, with opaque and transparent
sources, clipped and unclipped regions, and reopened PNG comparisons.
Those comparisons now cover every displayed image pixel, at 50%, 73%, 137%
and 150% effective zoom. For a translucent, unclipped source at 137% zoom and
simulated 100% display scale, the [dark preview](tests/native/evidence/macos-ui/pixelation-whole-dark-preview-20261116.png)
and [reopened PNG](tests/native/evidence/macos-ui/pixelation-whole-dark-reopened-20261116.png),
and the [light preview](tests/native/evidence/macos-ui/pixelation-whole-light-preview-20261116.png)
and [reopened PNG](tests/native/evidence/macos-ui/pixelation-whole-light-reopened-20261116.png),
each differ by at most one channel level across 44,936 pixels. These checks do
not cover framed or attached images, physical display scaling or live capture.

The generated transparent reopen matrix now covers both macOS frame styles,
with and without shadow, at 73%, 100% and 150% effective zoom and simulated
125% and 200% display scales in both app appearances. It includes an inset and overlapping
blur, blackout, pixelation and shape annotations. Each whole-composition
preview matched its reopened PNG within two channel levels. For the dark frame
with shadow at 73% and simulated 125% display scale, the [dark preview](tests/native/evidence/macos-ui/transparent-framed-dark-preview-20261116.png)
and [reopened image](tests/native/evidence/macos-ui/transparent-framed-dark-reopened-20261116.png),
and the [light preview](tests/native/evidence/macos-ui/transparent-framed-light-preview-20261116.png)
and [reopened image](tests/native/evidence/macos-ui/transparent-framed-light-reopened-20261116.png),
each have a maximum channel difference of one across 16,740 pixels. The
[unshadowed PNG](tests/native/evidence/macos-ui/transparent-framed-no-shadow-20261116.png)
retains 14,880 fully transparent pixels. These are generated images rendered
by the native GPUI Metal fixture; they do not verify real capture, physical
display scaling or Electron's frame pixels.

The Electron comparison initially could not load its stylesheet: Tailwind scanned
a vendored Rust test and generated an invalid CSS declaration. The stylesheet
now excludes the vendor directory from automatic source scanning. A fresh Vite
renderer server returned HTTP 200 for the stylesheet after previously returning
500, and the offscreen screenshot editor then loaded in both appearances. This
does not establish a packaged-build result.

An additional native comparison found that Core Graphics was closer on average
to the pinned Electron rounded-rectangle samples but still differed by up to 24
red levels. In the same Electron process, an SVG rectangle with rounded corners
rendered differently from an equivalent SVG cubic path and Canvas path; disabling
image smoothing did not change the SVG result. A candidate analytic coverage
calculation matched the integer-origin corner but still differed by up to 12
levels at fractional offsets. The [probe sources and results](tests/native/evidence/macos-ui/blackout-rasterizer-20261116.txt)
therefore narrow the mismatch to the rounded-rectangle rendering path; no
production rasterizer change was made. Full editor and physical display results
remain open.

The follow-up matrix expands the pinned Electron SVG samples to seven radii
and independently varied horizontal and vertical offsets. For radius 2, the
Tiny Skia scratch render differs by over two red levels in 229 of 4,608 sampled
pixels, maximum 34. Tested analytic candidates reduce the difference but do not
eliminate it; the best fitted candidate still misses ten samples by over two,
maximum five. It lacks a verified rendering rule and was not adopted. The
[expanded matrix and comparison](tests/native/evidence/macos-ui/blackout-rasterizer-20261116.txt)
cover an opaque upper-left corner; the other corners, transparent blending and
full editor rendering remain open.

The pinned Electron runtime also produces different rounded-rectangle pixels
when hardware acceleration is disabled. Its default configuration reported
accelerated Skia Graphite on this Mac; the software run reported GPU raster and
canvas acceleration disabled. Both matrices repeated exactly. At radius 2,
117 of 4,608 red-channel samples differed by more than two levels, by at most
34; the integer top-left sample was 158 in the accelerated run and 169 in the
software run. The latter reproduces the older assertion's value, though it does
not establish that assertion's origin. The [backend comparison](tests/native/evidence/macos-ui/blackout-rasterizer-20261116.txt)
is synthetic and does not establish which backend other systems use. No GPUI
production rasterizer was changed; preview/export parity and the Electron
comparison for other colors, alpha, corners and full editor workflows remain
open.

The macOS fake-daemon recording workflow now exercises the target-close retry
sequence that was previously Windows-only. It exposed a failed-start regression:
the window outline disappeared even though the pre-recording controls stayed
open. The failed-start rollback now retains an existing window outline and
still hides an area overlay created for the failed attempt. The fixture checks
that the empty failed project is removed, the window outline is not recreated or
hidden before retry, a queued target-close event survives a failed stop, and
the next stop finalizes the project and clears the controls and overlay.

A separate isolated window-recording fixture queued two target-close events
followed by a terminal capture error before the control could process them. The
one-entry queue dropped the terminal error and the partial-recording warning.
The queue now retains pending errors and chooses a terminal error ahead of
target-close events already in the same batch. The fixture failed before the
change and passes in both app appearances afterward: it checks the recovery path
in the warning, unchanged recording bytes and no normal stop after that error.
The [test results](tests/native/evidence/macos-ui/recording-queued-error-20261116.txt)
use a fake daemon and generated video; they do not establish live event timing or
physical target closure.

Another fixture sends a terminal capture error while a Stop request is in flight,
then returns a successful stop response. Previously the control removed its
error listener and lost the warning. It now retains the first terminal message
as the daemon event arrives and checks it after Stop returns, before treating the
take as successful. The fixture failed before the change; afterward it checks
that the warning includes the retained project path and that the generated video
is unchanged. The [results](tests/native/evidence/macos-ui/recording-error-during-stop-20261116.txt)
cover the event-before-response ordering in an isolated fake daemon, not live
recorder callbacks or errors delivered after the stop response.

The macOS daemon previously rejected Stop while the same session was still
starting. Its stop coordinator now waits for that session's startup task before
finalizing, and concurrent Stop requests join one finalization. The [native Swift
fixture](tests/native/evidence/macos-ui/recording-stop-during-start-20261116.txt)
checks successful and failed startup, duplicate Stop requests and a
replacement session without accessing capture devices. GPUI now displays
“Starting” without waiting for the Start response, retains that response with
Stop still available, and keeps capture protection and the project until the
daemon resolves the request. A definite start failure before Stop restores the
pre-recording controls and retains any partial files; the restored controls are
captured in [light](tests/native/evidence/macos-ui/recording-start-rejected-control-light-20261116.png)
and [dark](tests/native/evidence/macos-ui/recording-start-rejected-control-dark-20261116.png).
[Fake-daemon workflows](tests/native/evidence/macos-ui/recording-start-timeout-20261116.txt)
cover late success and failure before and after Stop, both failure response
orders, and target closure during startup, including a discard retry; the [light starting controls](tests/native/evidence/macos-ui/recording-starting-control-light-20261116.png)
and [dark starting controls](tests/native/evidence/macos-ui/recording-starting-control-dark-20261116.png)
are retained. GPUI now shows “Stopping” without waiting for the Stop response
and retains that response until it resolves. The [fake-daemon stop fixture](tests/native/evidence/macos-ui/recording-stop-timeout-20261116.txt)
checks successful finalization, discard, retryable failures, a paused recording,
terminal errors, an overlapping startup, and target closure during a failed
Stop. The [light](tests/native/evidence/macos-ui/recording-stopping-control-light-20261116.png)
and [dark](tests/native/evidence/macos-ui/recording-stopping-control-dark-20261116.png)
control images show the pending state. A daemon that neither responds nor
disconnects leaves the controls stopping. Sending the request and clearing
camera-preview protection still use synchronous daemon calls; their behavior with
an unresponsive daemon and actual ScreenCaptureKit timing remain open.

The macOS ScreenCaptureKit delegate also now reports failures from the current
stream during startup, including a closed window target, instead of discarding
them while the recorder's public state is still idle. A [native callback fixture](tests/native/evidence/macos-ui/recording-start-stream-error-20261116.txt)
checks session and error classification without starting a stream. Actual
callback timing remains unverified. The recorder now also retains unexpected
current-stream failures handled during Stop until its writers finish, then
reports the error while preserving the movie. A stream-specific synchronized
inbox now also carries a callback received before finalization even if main-thread
delivery is delayed, and the recorder module emits active-session errors before
scheduling cleanup. A [generated-movie fixture](tests/native/evidence/macos-ui/recording-stop-stream-error-20261116.txt)
decodes the retained file and checks delayed callbacks, stale streams, normal
closure and user-stop cases. Startup errors are emitted after successful Start
and before a queued Stop completes. Live startup timing and real callbacks
arriving after the inbox closes or after Stop remain unverified.

The screen recorder now checks the stream identity on sample callbacks as well
as error callbacks. A generated frame from a previous stream previously could
anchor the next session's timeline; the [native fixture](tests/native/evidence/macos-ui/recording-stream-identity-20261116.txt)
checks that it is ignored and that the current stream's frame starts the
timeline. An empty generated writer returns an error when stopped without a
video frame. Neither fixture establishes whether ScreenCaptureKit actually
delivers a late callback in a real session. Frames arriving while startup is
still in the idle state are currently ignored; whether a static screen can
then fail to deliver another frame is unverified. Capture-window exclusion
and audio synchronization must be preserved in any startup change.

The area variant checks that a failed start hides the temporary overlay and
passes a synthetic toolbar ID in `excludedWindowIds`. It also writes generated
video before rejecting the start: the old cleanup deleted it, while the new
cleanup retains the nonempty project and reports its path. A definite rejection
removes an empty project; a timeout or daemon disconnect retains even an empty
project because the start outcome is uncertain. Real-filesystem tests cover
those cases, and the [fake-daemon fixture](tests/native/evidence/macos-ui/recording-partial-start-20261116.txt)
passes in both appearances. A successful retry then
uses a generated checkerboard video, opens the GPUI editor, applies a visible
zoom, saves the project and exports MP4. In the light and dark runs, decoded frames at
0.5, 1.0 and 1.5 seconds matched the editor composition with mean channel
errors of 0.653, 0.713 and 0.712. The fixture exposed a duration mismatch:
GPUI used 3.6 seconds of control-bar elapsed time for a two-second recording
even when the daemon returned the finalized two-second duration. GPUI now uses
the returned duration for the editor and history, falling back to elapsed time
when it is absent or invalid. The [synthetic editor image](tests/native/evidence/macos-ui/recording-area-edited-preview.png)
shows the corrected two-second timeline. A native Swift unit probe also passed target
closure and stopped-sample classification without opening a capture stream.
The daemon, window IDs, pointer input and media in this workflow are synthetic;
it does not prove native capture exclusion or physical target movement.

A separate fake-daemon control fixture rejects an attempt to disable the camera
while recording. Previously GPUI hid the camera preview before the rejected
request, leaving it hidden while recording continued with the camera enabled.
It now keeps the preview until the recorder accepts the disable request. The
fixture verifies the failed request leaves the preview in place and a successful
retry hides it. No camera or device permission is used by this fixture; native
camera behavior remains unverified.

The macOS fake-daemon workflow now carries area, screen and window recordings
through the GPUI editor, draws a zoom with simulated timeline input, saves and
reopens the project, and compares three decoded export frames with the reopened
preview. A local run used the earlier screen-only native synthetic area and
move/close window takes; the screen take was scaled and time-stretched from the
earlier synthetic exclusion probe. In both appearances, mean channel errors at
0.5, 1.0 and 1.5 seconds were below 0.72. At one second the zoom changed 320,
85,669 and 2,209 pixels by at least 16 levels for the area, window and screen
fixtures respectively; on those pixels the export favored the edited preview
over the unzoomed image. The [reopened window preview](tests/native/evidence/macos-ui/recording-window-reopened-preview.png)
is retained. The daemon handoff and target-close events in this workflow are
simulated, and the area image is nearly uniform. This does not prove live GPUI
recording, physical movement, capture exclusion, audio or camera behavior.

The fake-daemon area, window and screen handoffs now also draw a filled rectangle
on the zoomed preview using simulated pointer input, save and reopen the project,
and compare decoded MP4 frames with the reopened preview. Both appearances passed
with the same generated checkerboard recording and FFmpeg 9.0.2. At 0.5, 1.0 and
1.5 seconds, mean channel errors were 0.694, 0.744 and 0.743. At one second,
2,716 pixels changed because of the drawing and 31,354 because of the zoom; on
those pixels the decoded export was closer to the respective edited preview than
to the version without that edit. The [dark](tests/native/evidence/macos-ui/recording-drawing-zoom-dark-20261116.txt)
and [light](tests/native/evidence/macos-ui/recording-drawing-zoom-light-20261116.txt)
measurements, [editor](tests/native/evidence/macos-ui/recording-drawing-zoom-window-preview-20261116.png)
and [decoded frame](tests/native/evidence/macos-ui/recording-drawing-zoom-window-decoded-20261116.png)
are retained. Tool settings are selected through the test API; the drawing gesture
and timeline zoom use simulated pointer input. This does not establish physical
recording, target movement or closure, capture exclusion, or media-device behavior.
The separate 720×520 generated drawing workflow also passed on the current code.
On changed pixels its decoded export's mean RGB error was 20.77 against the
edited preview versus 118.72 against the undrawn preview; this narrower result is
less precise than the checkerboard workflow above. The [measurements](tests/native/evidence/macos-ui/recording-drawing-narrow-20261116.txt)
do not establish physical window resizing or live playback.

The retained narrow export is [H.264 with 4:2:0 chroma](tests/native/evidence/macos-ui/narrow-video-codec-20261117.json). An uncompressed
RGB → 4:2:0 → RGB conversion of its preview reproduces the red outline's
difference from the decoded video. Across 1,196 red-dominant preview pixels,
the RGB mean error is 20.770 against the decoded frame and 20.770 against the
uncompressed conversion; the conversion and decoded frame differ by 0.025 on
average, at most two levels per channel. The [conversion](tests/native/evidence/macos-ui/narrow-video-chroma-roundtrip-20261117.png)
and [measurements](tests/native/evidence/macos-ui/narrow-video-chroma-comparison-20261117.json)
were generated with FFmpeg 9.0.2. This isolates the dominant difference in this
one synthetic frame to color subsampling, not a displaced annotation. It does
not prove that other colors, content, export settings or hardware have the same
error, or that live preview and export match. No encoding settings were changed.

The same fixture exposed a capture-exclusion ordering gap: GPUI started the
recorder before applying camera-preview protection, and removed that protection
on a camera-off toggle. On macOS and Windows it now enables protection before
starting the recorder, retains it throughout the session, and clears it after a
successful stop or a failed start when the daemon accepts that cleanup. A simulated protection failure prevents the
recording from starting and rolls the setting back. Light and dark fixtures check
the request order and off/on toggles; a full macOS fake-daemon run also checks
failed-start rollback with the camera enabled and protection before starts with
the camera disabled. Native recorded pixels still need verification.

For live macOS screenshots with requested exclusions, the daemon previously
silently discarded IDs absent from ScreenCaptureKit's window list. It now
checks missing IDs against the on-screen Core Graphics window list and fails
the capture if a requested control is still visible or the list cannot be
verified. IDs for windows no longer on screen remain permissible, since normal
capture closes its selector before the request. Pure native tests cover the ID
resolution cases, and both macOS targets typecheck. The queries are separate,
so a window appearing between them can still race the check; native overlay
exclusion and captured pixels remain unverified.

The GPUI framed-attachment workflow now includes macOS light and dark frames
with translucent primary-image content, a top attachment, a right attachment,
and a drawn and moved annotation. In light and dark app appearances at simulated
100%, 125%, and 200%, the headless editor preview matched the composited PNG
within two channel levels before and after reopening the editing state. The PNG
retained the source alpha. The synthetic 125% [light preview](tests/native/evidence/macos-ui/framed-attachment-alpha-light.png)
and [export](tests/native/evidence/macos-ui/framed-attachment-alpha-light-export.png),
and [dark preview](tests/native/evidence/macos-ui/framed-attachment-alpha-dark.png)
and [export](tests/native/evidence/macos-ui/framed-attachment-alpha-dark-export.png)
are retained. These checks do not establish physical DPI behavior or complete
Electron parity for every frame and attachment combination.

The same synthetic workflow now also checks balanced primary images with both
macOS frame styles, top and right attachments, and a moved annotation. At
simulated 100%, 125% and 200%, its headless preview matches the exported PNG
within two channel levels before and after reopening, in both app appearances.
Representative 125% [light preview](tests/native/evidence/macos-ui/balanced-framed-attachment-light-preview.png)
and [export](tests/native/evidence/macos-ui/balanced-framed-attachment-light-export.png),
and [dark preview](tests/native/evidence/macos-ui/balanced-framed-attachment-dark-preview.png)
and [export](tests/native/evidence/macos-ui/balanced-framed-attachment-dark-export.png)
are retained. These examples have no inset or shadow; physical display scaling
remains unverified.

The same unannotated compositions were compared with Electron's actual export
hook running in isolated Chrome 157, with the same source pixels, crop, frame
styles and scale. Dimensions and three interior content samples matched. After
compositing onto white, 678–1,478 pixels per image differed by more than two
channel levels across the six cases, with the largest differences around frame
edges, corners and traffic lights. At 125%, 1,477 of 48,762 light-frame pixels
and 1,478 dark-frame pixels exceeded that difference; the maximum was 42 levels
and the mean maximum-channel differences were 0.294 and 0.367 respectively.
The reference and GPUI images at 125% are retained for
[light](tests/native/evidence/macos-ui/balanced-frame-electron-light.png),
[GPUI light](tests/native/evidence/macos-ui/balanced-frame-gpui-light.png),
[dark](tests/native/evidence/macos-ui/balanced-frame-electron-dark.png) and
[GPUI dark](tests/native/evidence/macos-ui/balanced-frame-gpui-dark.png), with
[all measurements](tests/native/evidence/macos-ui/balanced-frame-comparison.json).
This confirms a small frame-rendering mismatch; it does not establish its cause
or verify the packaged Electron runtime. Insets, shadows and physical display
scaling remain unverified.

An isolated frame-raster probe separates the remaining differences from image
cropping and wallpaper composition. It draws the same rounded outline, fill and
traffic-light circle with Tiny Skia 0.11.4 and Chrome 157 Canvas 2D at four
scales and two offsets. At 125%, the one-pixel-scaled outline differs by more
than two color levels at 1,082 of 57,600 canvas pixels, with a maximum difference
of 16. On a straight top edge, the outer and inner pixel values are 200 and 184
in Tiny Skia, versus 184 and 200 in Chrome. The
[GPUI-rasterized outline](tests/native/evidence/macos-ui/frame-raster-outline-125-gpui.png),
[Chrome outline](tests/native/evidence/macos-ui/frame-raster-outline-125-chrome.png),
[probe sources](tests/native/evidence/macos-ui/frame-raster-probe.rs),
[browser source](tests/native/evidence/macos-ui/frame-raster-probe.html),
[comparison script](tests/native/evidence/macos-ui/frame-raster-compare.py) and
[measurements](tests/native/evidence/macos-ui/frame-raster-probe-results.json)
are retained. [Reversing the outline path](tests/native/evidence/macos-ui/frame-raster-probe-reverse.rs)
did not resolve the discrepancy. No
rendering change was made: an offset tuned to one scale would not establish
correct rasterization at the others, and Chrome is not the packaged Electron
runtime.

At simulated 133⅓% display scale, a moved rectangle in a framed attachment
composition previously differed between GPUI preview and export at eight pixels,
by up to 22 channel levels. The preview treated the effective zoom as native for
placement but passed its nearly-one value to the patch rasterizer. The rasterizer
now uses the same native scale in that branch. In both app appearances, strict
preview/export and reopen comparisons pass for Windows and macOS frame styles;
the original case has no pixels more than two levels apart and a maximum
difference of one. The [before preview](tests/native/evidence/macos-ui/fractional-frame-preview-before.png),
[after preview](tests/native/evidence/macos-ui/fractional-frame-preview-after.png)
and [export over the editor matte](tests/native/evidence/macos-ui/fractional-frame-export-matte.png)
are retained. Physical display scaling remains unverified.

Electron draws the title bar at a fractional height even though it rounds the
layout height. GPUI now does the same. At simulated 133⅓%, the number of pixels
more than two levels apart over white fell from 2,351 to 1,507 for the light
frame, and from 2,042 to 1,660 for the dark frame, out of 49,896 pixels per
image. Mean maximum-channel differences fell from 0.921 to 0.511 and from 1.120
to 0.569 respectively. Framed attachments and unbalanced single-image frames
with inset and shadow pass strict preview/export comparisons at this scale in
both app appearances. The [Electron reference](tests/native/evidence/macos-ui/balanced-frame-electron-fractional-light.png),
[GPUI before](tests/native/evidence/macos-ui/balanced-frame-gpui-fractional-light.png),
[GPUI after](tests/native/evidence/macos-ui/balanced-frame-gpui-fractional-light-after.png)
and [measurements](tests/native/evidence/macos-ui/fractional-title-comparison.json)
are retained. Frame edge and control differences remain open, and the browser
reference does not verify packaged Electron behavior.

For a balanced single screenshot with a macOS frame, the headless preview
previously omitted the saved frame's border and rounded clipping. At simulated
133⅓%, the strict comparison found 442 pixels more than two channel levels
apart. The [old preview](tests/native/evidence/macos-ui/balanced-single-frame-preview.png)
and [export over the editor matte](tests/native/evidence/macos-ui/balanced-single-frame-export-matte.png)
show the original failure. Balanced frames without inset now use the existing
edge preview, painted above the image and annotations; fractional title-bar
boundaries retain the partially covered image row. In both app appearances,
the new matrix checks macOS light and dark frames, with and without shadows and
backgrounds, at simulated 100%, 125%, 133⅓% and 200%, before and after reopen.
It includes annotations crossing all four crop edges and one entirely outside.
Each comparison stays within two levels except at most one rounded-edge pixel
that may differ by three. The 133⅓% [dark preview](tests/native/evidence/macos-ui/balanced-single-frame-after-dark-preview.png)
and [export](tests/native/evidence/macos-ui/balanced-single-frame-after-dark-export.png),
and [light preview](tests/native/evidence/macos-ui/balanced-single-frame-after-light-preview.png)
and [export](tests/native/evidence/macos-ui/balanced-single-frame-after-light-export.png),
match within two. Other annotations, large captures and physical display
scaling remain unverified.

When a balanced framed source contained transparent pixels, its preview used
the editor surface color behind them while the saved PNG remained transparent.
The preview now exposes the stage background. In the 100% dark case, pixels
more than two levels from the saved image over that background fell from 601
to one; the maximum difference fell from 11 to three levels. The
[before preview](tests/native/evidence/macos-ui/balanced-alpha-before-preview.png),
[after preview](tests/native/evidence/macos-ui/balanced-alpha-after-preview.png)
and [export over the stage background](tests/native/evidence/macos-ui/balanced-alpha-export-matte.png)
are retained, as are the 133⅓% [light preview](tests/native/evidence/macos-ui/balanced-alpha-light-preview.png)
and [export](tests/native/evidence/macos-ui/balanced-alpha-light-export-matte.png).
The matrix now includes fully transparent and 50%-alpha source regions in both
app appearances, with and without inset, shadow and background; the transparent
PNG retains alpha 0 and 128 when those effects do not fill the region. Each
preview/export comparison meets the same two-level tolerance, with at most one
rounded-edge pixel at three. The 100% [GPUI PNG](tests/native/evidence/macos-ui/balanced-alpha-gpui.png)
and [Electron-hook PNG](tests/native/evidence/macos-ui/balanced-alpha-electron.png)
have matching sampled alpha; 294 of 20,720 pixels differ by more than two levels
over white. With an inset, the [GPUI PNG](tests/native/evidence/macos-ui/balanced-alpha-inset-gpui.png)
and [Electron-hook PNG](tests/native/evidence/macos-ui/balanced-alpha-inset-electron.png)
match at the sampled inset, translucent and clear regions; 310 of 25,584 pixels
differ by more than two levels. The reference runs in Chrome.

The same headless matrix now includes a two-stop 50%-alpha solid gradient behind opaque
and transparent sources, with and without inset and shadow, in both app
appearances. The saved wallpaper retains alpha 128 where unobstructed, and the
preview matches the saved PNG composited over the editor background within two
levels, except at most one rounded-edge pixel at three. At simulated 133⅓%,
the [light preview](tests/native/evidence/macos-ui/balanced-translucent-wallpaper-light-preview.png)
and [export](tests/native/evidence/macos-ui/balanced-translucent-wallpaper-light-export-matte.png),
and [dark preview](tests/native/evidence/macos-ui/balanced-translucent-wallpaper-dark-preview.png)
and [export](tests/native/evidence/macos-ui/balanced-translucent-wallpaper-dark-export-matte.png)
match within two levels throughout. At 100%, the [GPUI PNG](tests/native/evidence/macos-ui/balanced-translucent-wallpaper-gpui.png)
and [Electron-hook PNG](tests/native/evidence/macos-ui/balanced-translucent-wallpaper-electron-hook.png)
have matching alpha at sampled wallpaper, translucent-image and clear-image
pixels. Over white, 199 of 33,840 pixels differ by more than two levels, with a
maximum difference of 42; all are within the frame bounds. The
[measurements](tests/native/evidence/macos-ui/balanced-translucent-wallpaper-comparison.json)
come from the Electron export hook running in Chrome, not packaged Electron.
Other wallpaper types, real capture and physical scaling remain unverified.

At the edge of a video composition, GPUI's blur used clamped source pixels
where Electron's canvas filter samples transparency. With a synthetic
half-alpha canvas, the unit-scale corner was `[64, 128, 191, 192]` in GPUI
and `[65, 128, 192, 145]` in Electron 43.4.0. GPUI now uses transparent-edge
sampling for video blur. Tests compare translucent and opaque clipped cases
with the [Electron reference](tests/native/evidence/macos-ui/video-blur-parity-20261116.txt),
and a generated edited video checks a half-size preview against decoded export
pixels across cuts and speed changes. An isolated GPUI fake-daemon workflow
also passed generated area, window and screen recording handoffs, with sampled
decoded-export versus preview mean channel errors below one; that workflow
checks zoom, not the blur case or live capture. Electron's blur also changes apparent
size at scaled transforms; GPUI keeps its composition-proportional scale so
the scaled preview remains consistent with export. That Electron difference,
other scales and live recording/playback remain open.

Electron includes the inset around a balanced screenshot with a frame; GPUI
previously omitted it from both preview and export. With an 8px inset, the
synthetic 100% export was 140×148 rather than Electron's 156×164. GPUI now
uses the framed composition for this case and samples the balanced image edge
for the inset color. The light/dark headless matrix covers inset 0 and 8,
shadows, backgrounds, annotations across all four crop edges and reopen at
simulated 100%, 125%, 133⅓% and 200%. Preview and export stay within two color
levels, except at most one rounded-edge pixel that may differ by three. The
[previous GPUI export](tests/native/evidence/macos-ui/balanced-inset-before.png),
[corrected GPUI export](tests/native/evidence/macos-ui/balanced-inset-gpui.png)
and [Electron-hook output](tests/native/evidence/macos-ui/balanced-inset-electron.png)
are retained. Their corrected 100% dimensions and inset and image sample
colors match; 310 of 25,584 pixels differ by more than two levels over white,
with a maximum difference of 62. A 133⅓% [preview](tests/native/evidence/macos-ui/balanced-inset-preview.png)
and [export over the editor matte](tests/native/evidence/macos-ui/balanced-inset-export-matte.png)
are also retained. The reference runs Electron's hook in Chrome, and physical
scaling and other inset combinations remain unverified.

Recording project allocation also had a name collision race in both shells:
the existence check and directory creation were separate, so concurrent starts
could share one project and a failed start could delete another take. A local
24-thread GPUI test produced only four distinct directories before the fix.
Both shells now reserve project directories with exclusive creation and retry
only name collisions. The GPUI test now obtains 24 unique directories while
preserving an occupied name; an Electron real-filesystem test preserves an
existing recording, and unit tests cover retry and other errors. The full
synthetic Mac workflow and the full Vitest suite passed; the first unrestricted
Vitest run timed out in the rebrand scan, which passed alone and in the full
two-worker rerun. Simultaneous recordings across the two running shells remain
unverified.

The macOS timer and camera-preview panels converted top-left coordinates using
`NSScreen.main`, which follows the key window rather than the global origin. On a
secondary display of a different height, this offset the panels and the camera
move event by the height difference. They now use the first display's height
for forward and reverse conversion. A native arithmetic fixture covers panels
above and below a 900-point primary display, including a 1,200-point secondary;
both macOS targets are typechecked. A native `FloatingPanel` placement probe
also passed on this Mac's primary display without showing the panel. This Mac
has only one available display, so physical multi-display placement and camera
interaction remain unverified.

For a narrow timer selection at either horizontal edge, GPUI previously centered
the 140-point countdown outside the display. It now resolves the selected
display's bounds and clamps the control within them; if the bounds cannot be
resolved, its known left origin still prevents left-edge overflow. Unit tests
cover both edges, a negative-origin display and a display narrower than the
control. On this Mac's 1728-point display, an isolated GPUI probe placed the
control at x=0 and x=1588 for 10-point edge selections, instead of the previous
x=-65 and x=1653. A Swift probe constructed the actual hidden `FloatingPanel`
and `TimerButton` at both positions and verified their AppKit frames. The
[native and fixture record](tests/native/evidence/macos-ui/timer-display-edge-20261116.txt)
also includes a fake-daemon timer workflow for the left edge, completion,
cancellation, timeout, failure and retry. A separate [native lifecycle check](tests/native/evidence/macos-ui/timer-control-lifecycle-20261116.txt)
showed the actual timer module's panel at both edge positions, canceled it with
synthetic AppKit mouse events, waited for a one-second countdown to complete,
and showed and hid it again. Each path removed the panel. Its transport was
mocked; physical interaction, the live GPUI-to-daemon handoff, timer capture,
another display and scaled Windows placement remain unverified.

The macOS countdown's completion event previously followed `orderOut` while
the window server could still list the timer panel at full opacity. The panel is
now made transparent before it is ordered out and before completion, cancel or
hide is reported. A [native handoff fixture](tests/native/evidence/macos-ui/timer-window-server-handoff-20261116.txt)
failed on the prior completion path and passed three times after the change;
it checks on-screen window metadata at each event. It does not capture the
screen, so exclusion from saved timer captures remains to be verified with pixels.

An isolated macOS renderer check now places blur and pixelation across the seam
between a framed primary screenshot and a right attachment. At simulated 133⅓%
scale, both effects change pixels on both sides of the seam, and the complete
preview matches the flattened PNG over the editor matte within two channel levels in light and dark
app appearances with light and dark macOS frames. The source also contains a
translucent area away from the redaction. The dark-frame [blur preview](tests/native/evidence/macos-ui/framed-attachment-blur-preview-20261116.png),
[blur export over matte](tests/native/evidence/macos-ui/framed-attachment-blur-expected-20261116.png),
[pixelation preview](tests/native/evidence/macos-ui/framed-attachment-pixelate-preview-20261116.png)
and [pixelation export over matte](tests/native/evidence/macos-ui/framed-attachment-pixelate-expected-20261116.png)
are retained. This uses simulated input and a generated image, not a physical
display, live capture or an Electron pixel comparison.

GPUI now retains the daemon's camera-preview position while recording, so a
preview dragged before the camera is disabled reappears at that position when
the camera is enabled again. The position is session-scoped and movement events
are ignored while the preview is hidden. The headless recording-control fixture
injects a negative-coordinate move, an invalid move and a hidden-preview move,
then verifies the next show request and unchanged saved preferences. This uses a
fake camera and does not verify physical dragging or camera access on macOS.

A permission-free [native camera timeline probe](tests/native/evidence/macos-ui/camera-timeline-gap-20261116.txt)
found a recording issue that the visibility ranges do not resolve. The previous
camera timestamp calculation started at zero on its first frame and removed time
spent suspended. With generated source samples at 100, 101, 106 and 107 seconds,
and suspension between the second and third samples, its accepted presentation
times were 0, 1 and 2 seconds; the third sample was rejected as non-increasing.
If the main video began at source time 90, its corresponding times would be 10,
11, 16 and 17 seconds. GPUI preview and export seek the camera at main-video
time, so a camera first enabled during recording or enabled again after an
off-period could show the wrong frame. With an injected H.264 writer, the old
recorder path produced a [generated movie](tests/native/evidence/macos-ui/camera-timeline-before-20261116.mov)
whose decoded frames occur at 0, 1 and 2 seconds. An independent writer probe
anchored at zero retained frames at 10, 11, 16 and 17 seconds; it does not use
the recorder's timing logic. The screen recorder now converts camera timestamps
between capture clocks and removes the screen recorder's pause duration while
preserving camera-off gaps. It anchors the camera movie at zero and delays a
camera enabled during a pause until the screen timeline resumes. A generated
writer fixture using the changed recorder produced frames at 0, 10, 11, 16, 17
and 19 seconds across a delayed start, toggle and pause; GPUI's decoder selected
the expected frame at each tested video time. A generated project then trimmed
and cut that recording, doubled the speed of the second segment, and compared
six GPUI preview frames with decoded MP4 export: the mean channel error was below
three at each point, and camera visibility and sampled colors matched. Stopping
the camera before its first frame previously returned a zero-byte movie; it now
removes the empty file and returns no camera track, as checked by a native
fixture. A canceled writer after a generated frame also previously returned a
path even when no movie existed. Stop now returns no camera track unless the
writer finishes successfully; a native cancellation fixture checks the result
and cleanup. A separate stop fixture injects a canceled camera writer and
successfully decodes a frame from the finalized primary movie with no camera
track. The stop response now reports a missing camera track, and both GPUI and
Electron present a warning after saving; deliberately discarded recordings do
not warn. Unit and native fixtures check the flag and handoff without displaying
the warning. These probes do not access a camera and use the same clock for both
synthetic sources. Distinct hardware clocks, live editor interaction, multiple
toggles and physical media verification remain open.

Camera visibility markers also used the last screen-frame duration. In a
generated callback sequence with the first screen frame at 100 seconds and no
later screen frame, enabling and disabling the camera at clock times 120 and
125 previously recorded a zero-length range at 0. The recorder now timestamps
those changes on the screen clock and freezes that position across a pause. A
native fixture verifies the 20–25 second range, a 20–22/22–25 transition across
a pause, and fallback when the clock is unavailable or invalid. It does not
establish how long a real static ScreenCaptureKit stream can go without a screen
callback. Physical camera toggles and full preview/export with actual media
remain unverified.

A generated native writer probe now confirms that the recorder's pause boundary
also used the last screen callback. With a frame at 100 seconds, a pause at 125,
a resume at 130 and the next frame at 135, the old path counted 35 seconds as
paused. Pause and resume now use the stream clock when available; the fixture
checks the five-second pause, the camera range while paused and a 27–29 second
range immediately after resume. Without a usable clock the existing sample-based
fallback remains. The primary movie was also incorrect for sparse callbacks:
a 30-second sequence decoded as 60 seconds, and a 25-second sequence with one
frame reported zero and decoded to about 0.067 seconds. Finalization now holds
the last written image through the stop time, waits for writer readiness and
ends the video session at that time. Generated cases for a static frame, a
pause/resume, stopping while paused and missing clock passed native decoded-frame
checks; FFmpeg reports 25 and 31 seconds for the first two cases. The finalizer
also completed after observed writer backpressure and reported a canceled writer.
GPUI decoded the pause fixture through forward and backward seeks, and its
trimmed preview matched sampled pixels in the decoded export. See the
[native probe and outputs](tests/native/evidence/macos-ui/static-recording-timeline-20261116.txt).
Whether real ScreenCaptureKit supplies this callback sequence, actual target
closure, and live pause/resume and preview/export remain unverified.

A subsequent generated-frame test forced the video input to be busy during the
last screen change. The old finalizer held the previous image for the remainder
of the movie. The recorder now retains the latest pending image and its original
timestamp, flushes it when the writer recovers or before stopping, and keeps the
pending buffer count bounded at one. Native decoded-frame tests verify a change
at 10 seconds, recovery with a later change at 15 seconds, and stopping while
paused after the first frame arrived under backpressure. Four sampled GPUI
preview frames around the injected busy-frame change matched a decoded export
after cuts. Intermediate frames can
still be dropped during sustained backpressure. The test injects readiness at
the recorder boundary; a physical ScreenCaptureKit stream and hardware encoder
under sustained load remain unverified. See the
[native backpressure evidence](tests/native/evidence/macos-ui/pending-video-frame-20261116.txt).

Microphone alignment also used the last screen callback as its upper timeline
bound and as the toggle time. With a generated screen frame at 100 seconds, no
later screen callback and a microphone toggle at screen-clock time 120, the
old path placed the first audio at 1 second in the fixture. The microphone path
now reads a synchronized screen-clock position, pause duration and video-start
state together, falling back to the last screen time when the clock is not
available. Generated AAC tests place audio at 20–20.51 seconds and verify silence
before it, including enabling while paused and a first screen frame not yet
accepted by the writer. This is synthetic audio: actual microphone permission,
device clock conversion and live toggle latency have not been tested. See
[native microphone evidence](tests/native/evidence/macos-ui/static-microphone-clock-20261116.txt).
An edited GPUI project using that generated microphone track also preserved
silence before the toggle and audible audio afterward in preview and decoded
export.

A separate generated startup-delay case enabled the microphone at screen time 20
and delivered its first captured sample at time 21. The previous recorder placed
that sample at 20. The first sample now uses the microphone capture clock and
screen stream clock when both are available, and silence is written up to the
converted capture time. The microphone writer is also installed and started
immediately when the video timeline is already running, so it does not depend on
a later screen callback. Native AAC tests cover writer installation before and
after the first screen frame, a synthetic clock origin offset, and a callback
delivered two seconds after capture. GPUI preview and decoded export retained the
resulting silence and audio after trimming. The [native evidence](tests/native/evidence/macos-ui/microphone-startup-clock-20261116.txt)
uses generated PCM and clocks; physical microphone startup, device clock drift,
device swaps and the fallback when a clock is unavailable remain unverified.

The native macOS compact controls register their tooltip on an AppKit view in
an unfocused window. An isolated GPUI window reported that AppKit's
`allowsToolTipsWhenApplicationIsInactive` was false after registration, which
prevents tooltips while the application is inactive. Registration now enables
that setting while text is present and clears it when the tooltip is removed.
The [native probes](tests/native/evidence/macos-ui/inactive-app-tooltip-20261116.txt)
check the actual window setting, text replacement and clearing. A subsequent
isolated preview placed under a stationary pointer registered “Close preview”
and Core Graphics listed a separate tooltip-sized window with its own window ID
and sharing enabled; the recording filter excludes the control window
ID, not this dynamically created tooltip ID. Whether ScreenCaptureKit includes
that tooltip in recorded pixels remains unverified, as do normal pointer entry,
hover exit, placement on other displays and tooltip appearance across themes.

The GPUI compact controls now request a daemon-owned tooltip panel instead of
AppKit's separate implicit tooltip window. The panel is registered for capture
exclusion while hidden, and waits for filter updates and in-flight screenshots
before it becomes visible. Hover changes and active-owner closure hide it, and stale
requests cannot restore an older tooltip. A [native lifecycle fixture](tests/native/evidence/macos-ui/protected-tooltip-lifecycle-20261116.txt)
checks window registration, screenshot coordination, cancellation, out-of-order
requests and failed filter updates. It uses a simulated recording filter; it does
not establish that tooltip pixels are absent from a live recording or screenshot.
In a separate [isolated GPUI probe](tests/native/evidence/macos-ui/protected-tooltip-gpui-20261116.txt),
synthetic hover requested recording and preview tooltips, and macOS window
metadata confirmed that daemon-owned panels appeared and disappeared on exit.
The probe did not check the painted text. The panel now receives Poratake's
resolved light or dark appearance, and the lifecycle fixture checks the
effective AppKit appearance in both modes. A light-mode GPUI probe also displayed
the panel while macOS was in dark mode; it did not measure its colors. A generated long preset label exposed a separate layout defect: its panel reserved
two lines, while AppKit truncated the text to one. The label now wraps and has
room for both lines without enlarging the panel. Isolated AppKit label snapshots
show the [before](tests/native/evidence/macos-ui/tooltip-long-label-before-20261116.png)
and [after](tests/native/evidence/macos-ui/tooltip-long-label-after-20261116.png)
result; the native fixture checks painted pixels in both lines for English and
Japanese labels in light and dark appearance. These bitmaps contain only generated label content, not desktop
pixels. A subsequent generated-label probe found that longer text could exceed
the intended four lines and that explicit line breaks could wrap prematurely.
The panel now uses the text field's fitted size and truncates its last visible
line. Isolated label bitmaps show [long text before](tests/native/evidence/macos-ui/tooltip-four-line-before-20261116.png)
and [after](tests/native/evidence/macos-ui/tooltip-four-line-after-20261116.png),
and [explicit lines before](tests/native/evidence/macos-ui/tooltip-explicit-lines-before-20261116.png)
and [after](tests/native/evidence/macos-ui/tooltip-explicit-lines-after-20261116.png).
A native fixture also checks painting and placement at all four corners of this
Mac's visible display, alternating light and dark appearance. It does not test
physical pointer input, multiple displays or tooltip capture exclusion. Physical
hover, placement and visual theme parity still require native verification.

An ordering review found two cross-window races in that handoff: a late exit from
the previous window could suppress the new window's tooltip, and a new window's
exit delivered before its show could leave the previous tooltip visible. GPUI
now sends an exit only for the current owner, and the daemon applies the latest
exit even if its preceding show has not arrived. A GPUI owner-state test and the
[native lifecycle fixture](tests/native/evidence/macos-ui/protected-tooltip-ordering-20261116.txt)
cover these cases with simulated input and filter updates. Actual capture pixels
and physical pointer transitions remain unverified. An isolated GPUI probe also
switched from the recording control to the preview and sent the old control's
exit; the preview tooltip stayed mapped until its own simulated exit.

Exclusion-based macOS area and scroll screenshots previously took a window-ID
snapshot without coordinating with a newly presented daemon overlay. A hidden
recording overlay or camera panel could therefore become visible while an older
snapshot was in flight. Those screenshots now add the registered daemon-window
IDs to their exclusion snapshot, and the protected panels wait for in-flight
snapshots before becoming visible. A [native coordination fixture](tests/native/evidence/macos-ui/screenshot-exclusion-barrier-20261116.txt)
checks overlapping snapshots and a new snapshot arriving while presentation is
waiting. This alone did not protect AppKit's separate tooltip window or prove saved
pixels on a live capture; the GPUI tooltip now uses the protected panel described
above. Direct area screenshots now also hold the coordination
barrier while their background capture runs; if a protected daemon window is
already registered they use the exclusion-based path. The [direct-path fixture](tests/native/evidence/macos-ui/direct-screenshot-barrier-20261116.txt)
checks the dispatch, wait and failure cleanup without capturing the screen. If
the requested rectangle does not fit one display, a new window-list fallback
preserves the requested desktop rectangle and removes excluded IDs before using
Core Graphics. [Geometry and live window-metadata checks](tests/native/evidence/macos-ui/cross-display-window-list-20261116.txt)
pass, but the fallback's pixels, color handling and mixed-DPI output are not
verified on multiple physical displays. The legacy direct scroll path now holds
the same snapshot barrier and switches to a filtered capture when protected daemon
windows are registered; it includes the native scroll boundaries and control panel
in that filter. A native fixture with injected frames checks a waiting overlay,
filter IDs, a stale request, and a failed filtered frame after two frames; the retained PNG
reopens pixel-identical to the generated stitched image. The [fixture evidence](tests/native/evidence/macos-ui/legacy-scroll-capture-coordination-20261116.txt)
does not prove actual ScreenCaptureKit filtering or live scroll pixels. A window removed during a strict
exclusion capture can also cause a capture to fail closed.

Native window-server probes found that the recording dimmer and outline, scroll
boundaries and control panel, camera preview, and freeze overlay could remain
opaque immediately after their cleanup paths called `orderOut`. They now set
window alpha to zero and flush pending Core Animation changes before removal;
the protected tooltip and timer dismissal do the same. The
[cleanup fixtures](tests/native/evidence/macos-ui/overlay-window-server-cleanup-20261116.txt)
observed the previous opaque state and then verified that each tested surface
was absent or transparent at cleanup. The camera preview and freeze tests use
generated stand-ins, and the tests inspect window metadata rather than captured
pixels. Live capture, filter behavior and other displays remain unverified.

On macOS, a selected window that temporarily left the on-screen window list
previously caused its recording outline and tracking timer to be discarded.
The outline now becomes transparent while the window is unavailable and resumes
following it when it returns; it remains registered for capture exclusion until
the recording UI explicitly removes it. A [native generated-window fixture](tests/native/evidence/macos-ui/window-outline-lifecycle-20261116.txt)
reproduced the old failure and checks automatic tracking through movement,
hiding, restoration, closure and explicit cleanup. It does not start a recording
or verify captured pixels, another display, or finalization after target closure.

The current macOS development probe also shows the microphone menu in both
[dark](tests/native/evidence/macos-ui/recording-microphone-menu-dark-20261116.png)
and [light](tests/native/evidence/macos-ui/recording-microphone-menu-light-20261116.png)
appearance with generated device names. The normal-level toolbar has one border,
and returns to its fitted width after the menu closes. The
[native evidence](tests/native/evidence/macos-ui/recording-microphone-menu-native-20261116.txt)
covers window capture, accessibility names and hit testing with a fixture daemon;
it does not verify hardware access, physical input or production floating-level
appearance.

| Priority | Remaining work                                           | Acceptance evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| -------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1       | Native compact-window tooltips                           | Windows verified placement, dismissal and WGC exclusion. On macOS, [desktop hover](tests/native/evidence/macos-ui/native-tooltip-physical-pointer-20261117.txt) switched and dismissed both tooltips. A [GPUI area recording](tests/native/evidence/macos-ui/native-gpui-tooltip-exclusion-area-20261117.txt) and later [window recording](#2026-11-20-macos-window-recording-under-the-toolbar-and-tooltip) excluded the visible toolbar and tooltip. The screenshot filter and AppKit dispatch passed isolated checks. Full-display tooltip exclusion and mixed-DPI behavior remain open.                                              |
| P2       | Linux recording tooltips                                 | [Hosted Linux CI](https://github.com/Porabuild/Poratake/actions/runs/43362521397/job/131679275651) passed hover and keyboard-focus tooltip geometry tests, including unbroken and Unicode titles at simulated 100% and 200%. Very long titles remain shortened. Verify real Linux, macOS focus and native screen-reader behavior. A separate popup cannot yet be verified as excluded from Linux recordings.                                                                                                                                                                                                                             |
| P1       | Linux recording control capture exclusion                | An outside-window tooltip needs click-through behavior and capture exclusion so it neither intercepts toolbar input nor appears in recordings. X11 screen/area capture reads the root drawable; Wayland recording is currently disabled and its recorder requests no window exclusions. A private Xvfb probe captured a synthetic override-redirect popup while mapped and the underlying pixel after unmapping; it did not exercise Poratake's actual GPUI toolbar. Verify that toolbar on supported X11 compositors and Wayland.                                                                                                       |
| P1       | Linux window recording                                   | Hosted Xvfb raw-frame tests cover movement, resize, occlusion, remapping, cursor tracking and closure; fixtures also cover reparenting, missing Composite and XI2.1 grabs. A private Xvfb run decoded 93 H.264 frames. Blocked encoders report STOP_FAILED; permanently stuck capture workers keep retries reserved. Verify real window managers, outlines and GPUI handoffs. Wayland window recording is unsupported. XI2.0 can miss input during grabs, and rapid events can lag pointer motion. Electron's Linux capability helper disables recording.                                                                                |
| P1       | Wayland screen and area recording                        | GPUI recording is currently disabled on Wayland: static tracing shows the recorder passes zero dimensions from zed-scap to FFmpeg and ignores the selected area. The portal can return a window, but the capturer does not expose its source type, position or size, so first-frame sizing alone cannot establish a correct area crop. Implement and verify source-aware capture on a real Wayland session before re-enabling it.                                                                                                                                                                                                        |
| P1       | Native window/display highlighting and recording handoff | Native Metal scene checks in both appearances verify the scrim, hover exit and recording toolbar handoff. A generated AppKit target retained one outline through move and resize and cleared it on closure. Physical pointer transitions across displays remain open.                                                                                                                                                                                                                                                                                                                                                                    |
| P1       | Physical Windows DPI and monitor transitions             | Run both shells at OS 100/125/150/200%, mixed-DPI and negative-origin displays; verify cursor, crop pixels, bounds, caption hit targets and saved output.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| P1       | Preview display preference identity                      | Windows IDs match Electron 43 on the current monitor; simulated removal repositions the preview and retains the selected ID or falls back to primary. On the current single-display Mac, the GPUI preference ID, Electron ID and Core Graphics ID match; Electron and GPUI also agree on bounds and work area. See the [native comparison](tests/native/evidence/macos-ui/display-identity-20261116.txt). Arithmetic checks cover 125%/150% and negative origins. Unmarked legacy GPUI indexes may require reselection. Verify native multi-monitor transitions, mirroring and restarts, and cross-shell IDs on Linux.                   |
| P1       | Complex framed screenshot preview versus export          | Single unbalanced frames with inset match the PNG in the tested light/dark cases at simulated 125% and 200%; one solid-background case also matches a reopened PNG at 73%, 100% and 150%. A 9:16 frame without inset, with or without shadow, matches a reopened PNG at 73%, 100% and 150% zoom in the local light/dark probes. At simulated 125%, pointer selection and dragging with a framed inset and an edge handle are checked. Balanced macOS frames with and without inset match in the matrix above. Verify more styles and zooms, transparent reopen, additional inset settings, attached layers and physical DPI transitions. |
| P1       | Complex rounded screenshot preview versus export         | Single unframed captures with and without inset or shadow match the PNG in the tested light/dark cases at simulated 125% and 200%, including transparency and annotations. Oversized unframed rounded captures without shadow or inset now match reopened PNGs in the generated follow-up above. Verify additional effects, frames, balance, attachments, more zoom levels and physical DPI.                                                                                                                                                                                                                                             |
| P1       | Zero-padding wallpaper effects                           | Frame-only, corner/shadow-only and single unbalanced framed or unframed inset effects activate at zero padding. Unframed attached insets are checked on all edges by CPU and on the right edge by GPU. Balanced unframed single and attached images with corners, shadows and insets now match preview/export in generated light/dark GPU fixtures. Attachment checks cover all four edges, padding, seam blackouts, blur and pixelation. Verify physical DPI, real input, other content and zooms, framed attachments and more inset combinations.                                                                                      |
| P1       | Wallpaper shadows and complex image compositions         | Single unframed captures with or without inset match saved output in the tested shadow cases. After a pixel-conversion optimization, a local 4K shadow composite took a median 109–113 ms versus 142–148 ms with the preceding conversion; GPU upload, interactive behavior and slower hardware remain unmeasured. At 17,000px, CPU checks cover rounded inset tiles and shadows; generated macOS Metal checks cover framed previews and selected unframed corner, inset and shadow combinations against reopened PNGs. Verify other shadow settings, physical displays and other combinations.                                          |
| P1       | Complex screenshot pixelation                            | Fixed-size averaging matches Electron canvas samples. The tested plain redaction and boundary match saved pixels at 50%, 73%, 137% and 150% effective zoom in both themes at simulated 100%, 125% and 200% DPI, including transparency, clipping and later overlapping annotations. Verify frames, inset, balance, attached layers, physical DPI, large patches and interactive performance.                                                                                                                                                                                                                                             |
| P1       | Screenshot blur preview and Electron parity              | Earlier, blur and nearby annotations matched flattened PNG pixels within two levels at 50%, 73%, 137% and 150% zoom for unframed light/dark cases and simulated 100%, 125% and 200% DPI. Whole-frame differences remain outside that region, and oversized patches use the old path. Simple unframed gradient and inset samples now filter the composed backdrop; new annotation order passed local GPU checks. Verify broader fractional offsets in GPUI, frames, balance, corners, shadows, attachments, physical DPI, large patches and interactive performance.                                                                      |
| P1       | Screenshot annotation order                              | GPUI and Electron now apply redactions and highlights in annotation order. Generated GPUI Metal tests cover blur and pixelation in both orders, both themes, 73%/100%/137% zoom and simulated 100%/200% scale, and compare the entire preview with the reopened PNG. The earlier blackout corner comparison used the old order; rounded raster coverage can still differ. Verify live editing, other annotations, zooms, complex compositions and physical DPI.                                                                                                                                                                          |
| P1       | Full capture-service matrix                              | Fixtures cover window/area/display, timer, OCR/QR and scroll lifecycles. Native area/timer output and OCR/QR service probes are verified. A [live Mac GPUI scroll capture](tests/native/evidence/macos-ui/native-gpui-scroll-20261117.txt) saved a generated page and a history entry. Native pin movement/OS-close and one controlled attachment/save are verified. Extend native display/OCR/QR/scroll, clipboard and attachment cases; compare output with Electron.                                                                                                                                                                  |
| P1       | Frozen all-in-one desktop icons                          | Capture and recording unit checks pass; hosted fake-daemon workflows reach the visual baseline gate for hide-before-freeze and recording restoration. Verify native icon timing, mode switches and recording handoffs on real displays.                                                                                                                                                                                                                                                                                                                                                                                                  |
| P1       | Native pending-freeze handoff                            | Single-display macOS verified pending-freeze cancellation and replacement by live and frozen selectors. Fixtures cover release failures. Verify native Linux and multiple displays.                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| P1       | Native screen-recording selector handoff                 | Mock-platform tests cover replacing an existing or pending selector, release failure and retry, and a newer request that supersedes recording. Hosted Windows verifies the area-recording selection hole, restoration, pooled-overlay reuse and cancellation. Native pending/replacement handoffs and multi-display picker behavior remain unverified.                                                                                                                                                                                                                                                                                   |
| P1       | Capture filename collisions                              | GPUI and Electron screenshot, scroll and clipboard-to-editor captures now use exclusive filenames. Real-filesystem tests cover concurrent saves, cancellation, forced unsupported-hard-link fallback copying, timeout recovery and history/editor handoff. Verify simultaneous captures across both shells and on filesystems without hard links.                                                                                                                                                                                                                                                                                        |
| P1       | Linux daemon shutdown after printing                     | A scaled GTK X11 test failed while waiting for daemon exit with the print dialog open; the next run passed. An earlier X11 exit failure also passed on retry. Capture the exit status if it recurs and identify the cause.                                                                                                                                                                                                                                                                                                                                                                                                               |
| P1       | Scroll stitching with fixed footers                      | Windows/Linux PNGs retain one footer in 20 light/dark three-frame fixtures and synthetic changes to footer size and appearance. A 2% per-row tolerance retains tested small animated footer blocks at three widths, while unrelated-frame controls are rejected. Some animations and footer changes still stop early or produce incorrect output in synthetic multi-frame cases. The separate macOS matcher passed hosted synthetic fixed-footer, changing-footer and ambiguity fixtures. Verify real surfaces, weak overlaps and fixed headers.                                                                                         |
| P1       | Scroll stitching with repeated content                   | Windows/Linux retain the accepted prefix for a tested periodic false seam at 30% and 90% expected overlap; a noisy unique seam with variable movement still stitches in the tested cases. Earlier probes found 1,310 incorrect periodic cases rejected and 2,795 early stops, and 164 early stops in 360 translated-UI cases; those counts predate the new ambiguity guard. Quantify early stops and correctness on broader patterns, animations and real pages; macOS uses a separate stitcher.                                                                                                                                         |
| P1       | Unresponsive scroll stitch                               | Cancellation now cooperatively stops the stitch writer, and the shells wait for the finish and cancellation responses before discarding their temporary output. Windows and Linux worker tests cover joining and retryable startup failures; hosted macOS fixtures and GPUI parity cases also ran. A noninterruptible file write can still delay cancellation, and daemon shutdown can interrupt an in-flight stitch. Verify native UI handoffs and graceful shutdown on all three platforms.                                                                                                                                            |
| P1       | macOS scroll preview with variable overlap               | The preview shares measured overlap with the saved stitch and coalesces background rendering. Hosted native fixtures compare variable-overlap pixels and cancellation; verify real scrolling surfaces.                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| P1       | Native Windows clipboard formats                         | GPUI supplies CF_DIB alongside PNG; byte-level tests cover the bitmap and its headers. An isolated hosted Windows test verifies native format availability and PNG-only/DIB-only reads. Verify interactive app copy and paste into other applications, compare Electron, and profile large captures on slower hardware.                                                                                                                                                                                                                                                                                                                  |
| P1       | Native attachment compositor coverage                    | Hosted Windows verified live attachments and cancellation. Native macOS verified two frozen edges, editor restoration, exact base pixels and native save/reopen. Verify native macOS cancellation and physical DPI transitions.                                                                                                                                                                                                                                                                                                                                                                                                          |
| P1       | Cached Windows screenshot resources                      | Cached composition streams with 4 MiB scratch bands and rejects requests over 512 Mi pixels. Tests retain offscreen padding, gaps, alpha and the empty-cache fallback. Window alpha-mask capture and WIC internals can allocate separately; profile large native captures, encoding latency and slower hardware.                                                                                                                                                                                                                                                                                                                         |
| P2       | QR photo robustness and latency                          | The native decoder matches 65/65 original corpus cases and 493/520 brightness/contrast variants without losing a prior match. The attributed photo fixture is also tested at 10°, 25° and 45° rotations, with perspective distortion and a combined 15° rotation plus perspective warp. Verify additional real photographs and interactive app clipboard delivery; further reduce low-contrast 4K no-code latency and test slower hardware across both desktop versions.                                                                                                                                                                 |
| P1       | Recording media and target continuity                    | Native mic/system/camera enable/disable and device changes, target close/move/resize, audio continuity and camera visible ranges. Control-bar fixtures and image baselines cover UI/RPC transitions; hardware output still needs proof.                                                                                                                                                                                                                                                                                                                                                                                                  |
| P1       | Real-media audio speed edits                             | Synthetic checks now verify source pitch and short sounds in preview and decoded export, plus stereo tones and boundary transients across speeds. Verify live microphone, speech and music against Electron; non-x86 alignment performance is unmeasured.                                                                                                                                                                                                                                                                                                                                                                                |
| P1       | Extended video interactions and export matrix            | Preview/export checks cover zoom, camera, cursor, shortcuts, subtitles, blur at the composition edge, and some audio across cuts and speeds. Verify mic/music and live playback; expand interactions, styles and combined edits. Compare video blur with Electron near translucent or clipped source edges. Representative coverage only.                                                                                                                                                                                                                                                                                                |
| P1       | Video redaction crop and rounded camera                  | Crops include camera clips and rectangle corners. Offscreen macOS editor frames with rounded camera, zoom and pixelation match decoded export within two mean RGB levels in the effect region. 4K offscreen timing is recorded below. The 17 residual channels and interactive frame pacing remain open.                                                                                                                                                                                                                                                                                                                                 |
| P1       | Native accessibility action delivery                     | Native macOS Settings verified dropdown opening and selection, switch toggling, and slider setting and stepping at 880×700. Isolated tests exercise HeroGPUI actions. Verify screen readers and other operating system adapters.                                                                                                                                                                                                                                                                                                                                                                                                         |
| P2       | History visual and native verification                   | Verified native macOS contrast, 2× grid spacing, compact list at 140pt, opening and Finder reveal on one display. Mock tests cover grid and list tooltip boundaries. Native AppKit and Metal checks cover both action tooltips at 400 and 140 points in both themes. Physical multiple-display behavior remains open.                                                                                                                                                                                                                                                                                                                    |
| P2       | Attachment edge accessibility                            | Named controls, keyboard activation and in-bounds focus borders are tested. Native Windows accessibility invocation is verified for the right edge; extend physical keyboard and screen-reader checks to every edge and target platform.                                                                                                                                                                                                                                                                                                                                                                                                 |
| P2       | Complete state coverage beyond representative controls   | Add default/hover/pressed/focused/open cases for each shared control and overlay, with focused content remaining visible in narrow windows.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| P2       | macOS and Linux runtime comparison                       | The 2026-11-17 macOS smoke verified settings appearance, preview/control surfaces, live capture, generated-page scroll stitching, and recorded video edit/export. Ambiguous scroll seams, physical pointer movement between controls, live camera capture, and multi-display workflows remain open.                                                                                                                                                                                                                                                                                                                                      |
| P2       | Other app surfaces                                       | Settings appearance has live dark/light images at 880x700; recording controls and the bottom-right capture preview have live images in both themes. A generated screenshot-editor annotation/save/reopen also passed. History popover visibility, dismissal and editor handoff also passed in both themes. All four onboarding pages and both exit paths passed in both themes. Broader native interaction remains outstanding. [Mac live smoke evidence](tests/native/evidence/macos-ui/macos-live-smoke-20261117.txt) [History popover timings](#2026-11-18-macos-history-popover-dismissal-and-editor-handoff)                        |

GPUI now attempts to restore history after returned write errors and reports when a
capture was deleted but its history update failed. Isolated store tests cover
partial writes, malformed indexes and deletion/clear failures; a Windows headless
preview test exercises the partial-success warning. Native interaction remains
unverified. Electron still swallows history write errors.
History writes are not crash-atomic; readers outside the GPUI process may observe
a partial write.
GPUI saves the history index before evicting old files; failures retain or report
the affected entries. A crash between the index save and cleanup can leave files
outside history. Simultaneous writes from the two shells are not coordinated.
A failed history update after renaming a recording now warns that the index may
retain the old path; isolated tests cover write failure and untracked recordings.

A zero-padding framed attachment also matches CPU preview and export over a
rectangle and blackout crossing the primary-image seam. A positive control checks
the attachment pixel before the blackout is drawn.

[Hosted Windows CI](https://github.com/Porabuild/Poratake/actions/runs/43387203877)
verified the native capture-preview tooltip's text, window state and paint, then
compared every pixel in its screenshot region while the tooltip was hidden and
shown. That paint check does not independently prove its monitor-composited pixels.

The native pixel check below verifies overlapping GPUI scroll windows through
the shared macOS screenshot exclusion helper. The GPUI preview can overlap the
selected area when there is no room beside it. GPUI
now passes the preview, prompt and control window IDs when starting auto-scroll;
the daemon also includes its boundary windows and uses the same ScreenCaptureKit
exclusion and visible-window validation as live area capture. If an ID is missing
or a visible window cannot be resolved, it stops instead of saving an unverified
frame. Async results from a stopped, finished or restarted session are discarded.
The daemon's numeric conversion now rejects non-finite and out-of-range values
instead of trapping, and scroll startup validates coordinates, positive dimensions
and the display ID before altering a session. An isolated native protocol fixture
checks malformed numbers and display IDs, and accepts negative display origins;
the [evidence](tests/native/evidence/macos-ui/scroll-parameter-validation-20261116.txt)
does not start a capture. Scroll startup also now rejects an unavailable explicit
display ID or unmatched display origin instead of starting with an invalid ID or
falling back to the primary display. An [AppKit metadata fixture](tests/native/evidence/macos-ui/scroll-display-resolution-20261116.txt)
checks the available display, unavailable selections and preservation of an
active session; it cancels before UI setup or pixel capture. Physical unplug and
multi-display behavior remain unverified.
The macOS recorder also now rejects an unavailable explicitly selected display
instead of recording the first available display. It still chooses a moved
window's current display when that display is available. A
[native selection fixture](tests/native/evidence/macos-ui/recording-display-selection-20261116.txt)
checks generated and active-display metadata; live unplug, actual recording and
the startup error's presentation remain unverified.
The recorder now also rejects an area that no longer fits the chosen display
instead of passing an out-of-display source rectangle to ScreenCaptureKit.
[Native geometry checks](tests/native/evidence/macos-ui/recording-area-display-bounds-20261116.txt)
cover negative origins, all edges and connected-display metadata. An old area
that still fits after a display rearrangement cannot be detected by this check;
live rearrangement and the resulting error presentation remain unverified.
For a window spanning displays, GPUI's macOS recording control and scroll panel
now choose the display with the largest intersection with the target bounds,
instead of choosing the display containing the bounds' center. This follows
[Chromium's macOS display-matching rule](https://github.com/chromium/chromium/blob/150.0.7871.224/ui/display/mac/screen_mac.mm)
for positive intersections. [Geometry tests](tests/native/evidence/macos-ui/recording-control-display-placement-20261117.txt)
cover unequal overlaps, gaps, ties and negative origins; isolated
[light](tests/native/evidence/macos-ui/recording-control-placement-light-20261117.png)
and [dark](tests/native/evidence/macos-ui/recording-control-placement-dark-20261117.png)
control fixtures also pass. When there is no intersection, GPUI retains its
previous first-display fallback; Chromium uses the focused screen. Equal-area
ties depend on display enumeration order. This machine has one physical display,
so placement across real displays and mixed scale factors remains unverified.
The delayed post-scroll callback and queued auto-scroll start now also retain their
original session generation: stopping and restarting cannot turn an old callback
into a second capture or start an unrelated session. The Swift fixture holds and
releases synthetic frame completions across restarts, checks the timer and error
events, and reopens the retained frame; it also drains a queued stale start without
moving the cursor. These are lifecycle checks, not live scrolling evidence.
Initial session setup also checks that its preview session is still current, so a
cancelled setup cannot recreate boundary or control windows during a later session.
The fixture cancels and replaces a session before the queued setup runs and checks
that the old setup is rejected.
The [hidden native-window probe](tests/native/evidence/macos-ui/scroll-popup-native-frame.txt)
checks that all three GPUI IDs match their real AppKit windows, and the Swift
fixture checks stale-result rejection and pixel-identical saving of a synthetic
frame. Neither exercises actual ScreenCaptureKit scroll pixels or timing. Verify
the result with a real full-width area, including stop and retry, before claiming
that controls are absent. The Electron selection overlay also draws a border and
tint inside the area. Electron now passes the retained selector and control
CGWindowIDs to the same exclusion filter when starting auto-scroll, and declines
to start if either ID cannot be obtained. Mock tests cover the handoff, missing or
destroyed windows and retryable refusal; they do not prove ScreenCaptureKit can
resolve Electron's content-protected windows or that actual pixels are clean. If
the filter cannot resolve both windows, capture stops rather than accepting an
unverified frame. Native full-width capture and retry remain necessary. This is
separate from the recording-window exclusions below.

The scroll-control tooltip also remained registered after a window-exit event in
the macOS GPUI fixture. The control now clears it on exit and restores the active
button's tooltip on re-entry. An isolated native AppKit probe showed “Start
auto-scroll”, no tooltip after exit, and “Start auto-scroll” on re-entry while the
window remained unfocused; the [control image](tests/native/evidence/macos-ui/native-scroll-control-tooltip-fix-20261116.png)
is retained. The probe used simulated events and did not start auto-scroll or
screen capture. Physical hover and capture exclusion remain unverified.

On Linux the recording-control and capture-preview tooltips are drawn inside
their GPUI windows. X11 screen and area recording reads the root drawable with
no window exclusions, so a tooltip overlapping the recorded area can appear in
the output. Native GPUI hover and recorded pixels still need verification.

Windows tooltip capture protection falls back to `WDA_MONITOR` if full exclusion
fails; that mode can leave a black rectangle in a recording. If both affinity
methods fail, the tooltip is suppressed and the error is logged. Verify native
hover delivery, click-through, and recorded pixels on supported Windows displays.

The macOS GPUI recording path requests exclusion of its recording control and
registers daemon-owned camera and overlay windows with its ScreenCaptureKit filter. New
synthetic fixtures cover filter updates and stop coordination and passed in
[hosted CI at `5359f58f`](https://github.com/Porabuild/Poratake/actions/runs/43307382869).
Verify a real recording, including whether
staged transparent windows appear in `SCShareableContent` before they are shown.
Electron does not pass its control or retained selector-window IDs to that filter.
Its content-protection setting does not prevent capture by newer ScreenCaptureKit
applications, according to [Electron's documentation](https://www.electronjs.org/docs/latest/api/browser-window#winsetcontentprotectionenable).
Verify the hidden control's exclusion across restart before changing this handoff.
Electron's `getMediaSourceId()` exposes the macOS `CGWindowID`; the existing
overlay native-handle helper returns an `NSView` pointer instead. The Swift
recorder initially queries only on-screen windows, while Electron hides its
control before restart. Passing that hidden control's ID directly can therefore
make the exclusion resolver reject the restart.

[The hosted macOS fixture](https://github.com/Porabuild/Poratake/actions/runs/43399120426)
resolved a test-owned hidden window, but trapped when starting its filtered stream.
The debugger rerun did not have Screen Recording access and skipped the native
check, so it produced no backtrace. This does not establish an app failure or
verify captured pixels; an already-authorized interactive Mac is still needed.
The native check now skips by default. On such a Mac, run the fixture compiled
by the workflow with `--screen-capture-exclusion` to opt in without a permission
request.

A full-frame comparison alone was rejected as a replacement for the sampled
duplicate hash: a blinking pixel on a stationary page was accepted as scrolling.
The Windows/Linux footer path requires an exact overlap and either movement across
a majority of matching rows or a row appearing at its translated position and
disappearing from its old one. After detecting a footer, it can also accept one
changed pixel on the last overlapping row. In 1,236 generated cases, all accepted
frames produced the expected rows and 291 cases stopped early. Ambiguous or
animated content can still require different evidence. macOS uses a separate
duplicate check and stitcher.

When a sampled hash changes because one pixel blinks on a stationary page,
Windows and Linux now treat the frame as repeated if neither footer detection nor
an exact full-row overlap identifies scrolling. The accepted frames and saved PNG
remain unchanged through the three-frame duplicate limit. An edge pixel leaving
a genuinely scrolled viewport still stitches with both an exact and a biased
expected step. In 5,184 generated stationary-animation variants, erroneous frame
additions fell from 374 to 132; no previous stop became an addition. A 4K local
probe measured 9.86 ms before and 11.29 ms after for the one-pixel case, and
2.47 ms versus 2.61 ms for its translated control. Multi-pixel animations, real
pages, native capture time and slower hardware remain unverified.

The footer matcher now tries exact stationary rows first, then allows sparse
changes above an exact stationary suffix while retaining the full-row overlap and
movement checks. A three-frame test stitches a footer with a 16×16 animation;
the saved PNG reopens pixel-identically, and a sparse-content control retains the
exact-match path. In 240 translated UI cases with small animated blocks, eight
structurally incorrect outputs and 16 early stops became correct; the remaining
accepted-output differences were confined to the animated blocks.

The footer matcher now also finds an exact suffix around a localized animation in
the scrolling content, after trying exact matches first. A three-frame test and a
reopened PNG match every expected pixel. In 2,460 synthetic four-frame cases,
1,886 completed correctly, 471 produced incorrect pixels or dimensions, and 103
stopped early; before this fallback the counts were 1,479, 739 and 242. In 2,400
translated UI variants, 532 earlier stops became correct; 160 incorrect results
and 292 stops remain. A 5,412-case periodic-content matrix preserves all 2,084
previously correct outputs, with 1,310 incorrect results and 1,485 early stops
after the change. No previously correct case regressed in these matrices.
Animation pixels were excluded from those comparisons, and real pages remain
unverified. An exhaustive 1,172,808-case small-row check covers the matcher's
shorter-input boundary. In two local 4K repeated-content probes, median submit
times increased from 7.25 to 9.78 ms and from 9.45 to 11.01 ms; native capture
and slower hardware remain unmeasured.

After footer arbitration, Windows and Linux reject a sampled-perfect overlap
when no exact alternative exists and more than 5% of its full RGB pixels
disagree. A targeted animated-footer fixture retains the prior frame; recognized
footers, exact alternatives and small localized changes remain accepted. In
5,412 synthetic periodic cases, the results changed from 2,617 correct, 1,310
incorrect and 1,485 early stops to 2,617 correct, two incorrect and 2,793 early
stops. No correct case regressed. The 1,236-case footer corpus and translated-UI
corpus were unchanged. A local sampled-perfect, non-exact overlap benchmark
measured an additional 0.25 ms at 1080p and 0.79 ms at 4K, excluding native
capture. Large moving content may stop early, and sparse aliases and slower
hardware remain unverified.

The two remaining incorrect periodic cases were caused by a repeated body being
mistaken for a large fixed footer. A guard now stops those captures before
appending corrupt pixels: the same 5,412-case matrix has 2,617 correct outputs,
no incorrect outputs and 2,795 early stops. The footer and translated-UI probes
showed no new incorrect results. On one synthetic 4K input with one matching
offset and early mismatches at most others, the short-circuit guard took a median
0.035 ms versus 12.326 ms for the equivalent full-score scan. A repetitive 4K
input with mismatches at the last sampled pixel took 5.745 ms versus 11.032 ms.
These isolated timings exclude native capture and slower hardware and do not
bound every input.

In a synthetic two-frame scroll, period-8 main content shifted by 96px with an
independently moving edge produces the same pixels as a complete 104px shift.
Both have a unique 56px full-row match, although the first main-content overlap is
64px. When the expected sampled strip matches exactly and conflicts with a
different alignment supported by full-row evidence, Windows and Linux now stop
and retain the previous frames. The guard also checks three regions when the
expected alignment differs only in up to two sampled columns. Where the overlap
is long enough, those columns must recur in the first and last non-overlapping
regions; shorter conflicts stop because they cannot supply that evidence. Tests
cover sampled and unsampled edge pixels, with and without a fixed footer; the
retained PNG reopens pixel-identical after one or two accepted frames. A local
repeated strip or isolated changes in different columns of the first and middle
regions do not trigger the new guard.
The 1,236-case footer matrix still had no incorrect accepted output and 291 early
stops. A synthetic 4K exact-overlap probe had median times of 3.42 ms before and
4.63 ms after over 30 samples on this machine for the earlier guard. A separate
30-sample 4K probe measured 1.56 ms before and 1.74 ms after this extension for an
exact overlap, and 1.43 ms versus 1.64 ms with a single animated pixel.
A new guard stops when a perfect sampled match disagrees with repeated exact
full-row matches and the selected rows differ in multiple rows and columns. A
synthetic conflict now preserves the first frame, while the correctly aligned pair
still stitches with a one-pixel or 16-pixel single-row or single-column animation;
all saved PNGs reopen pixel-identical. A true conflict in one row or column can
still be mistaken for animation, and a multi-row, multi-column animation may stop
early. A 360-case probe translated four captured UI baselines
with two footer settings, five shifts and nine expected-offset values. The guard
changed 16 incorrect outputs into early stops: 196 correct, 12 incorrect and 152
early stops, versus 196, 28 and 136 before. Extending the conflict check to unique
full-row matches changed the remaining 12 incorrect results into early stops:
196 correct, zero incorrect and 164 stops. All 40 cases with the exact expected
offset remained correct. A valid translation can stop if its expected step is
wrong and the sampled strip is repetitive. A synthetic case with three
independently moving sampled columns still appends an ambiguous frame.
The same input frames also represent a valid 104px translation: the current
matcher selects its correct 40px overlap, while a tested three-column guard
stops. For a 96px main translation with independently moving columns, those
identical frames instead require a 48px overlap. The guard was not adopted;
the input alone cannot distinguish these cases. It left the 360-case translated
UI probe unchanged and had no incorrect output in the 1,236-case footer probe.
Injecting one changed pixel or a 16-pixel horizontal or vertical animation into
each of those 40 exact-offset cases produced 24 correct outputs and 16 early stops,
with no incorrect outputs. All 16 stops had a fixed footer and also occurred before
this guard. The newer localized fallback reduces the stops to four for a single
pixel and eight for a vertical 16-pixel animation; all 16 horizontal cases still
stop. An earlier approximate fallback resumed those cases but changed 13 pixels in
two vertical-animation outputs because its detected footer included static body
rows. That fallback was not adopted; the fixed-footer matcher still needs broader
animation tolerance.
An older near-edge animation failure no longer reproduces: eight light/dark,
image and scroll-distance cases now match all expected pixels with the current
localized-footer fallback. This is synthetic evidence, not a real-page test.
For a synthetic 4K repeated-row overlap with one animated pixel, ten local
post-warmup samples had medians of 3.13 ms before and 4.12 ms after this guard;
native capture and slower machines were not measured.
Isolated changes can also be indistinguishable from a different full-frame
translation: changes in one column at both outer regions can stop early, while
the prior overlap choice remains in effect for the tested changes in two regions.
Other repeated patterns and real scrolling surfaces remain unverified.

Windows and Linux daemons report overlap, capture and input errors when automatic
scrolling stops. GPUI and Electron now notify the user while retaining the previous
frames for saving; normal end-of-content and height-limit stops remain quiet.
Electron event tests cover both native and overlay-based paths and save the capture
after interruption. A GPU-free GPUI unit test checks the event mapping. A GPUI
workflow fixture checks the same sequence and passed in the hosted run at
`27798e87`; native notification presentation still needs verification.

The macOS daemon previously treated a failed screen-frame capture as a duplicate,
and ignored a failure on the initial frame. It now stops and emits `capture-error`
for either failure, while preserving earlier frames; a duplicate still reports
`duplicate`. The native Swift fixture injects a missing image before and after a
frame, checks the stop events, and saves and reopens the retained frame pixel for
pixel. This is synthetic failure injection, not evidence of a real capture or of
on-screen notification presentation. Real GPUI scroll exclusion and the Electron
scroll overlay remain subject to the limitations above.

Keep known platform differences explicit: preview/zoom use a baked blur because
GPUI has no live backdrop filter; the capture eyedropper remains an app overlay;
macOS traffic-light space and Windows custom captions are platform-specific.
These are not evidence that unrelated features match.

A scaled pixelation diagnostic found a preview/export discrepancy in a
generated one-pixel stripe fixture. A half-size editor preview and a 2×2 reduction
of the decoded H.264 export differed by 11.43 mean BGR levels in the pixelated
region. A separate comparison without encoding still differed by 10.14 levels
between half-size and reduced full-size previews; the full-size preview versus
decoded export differed by 1.84, and the unredacted control matched at the two
preview sizes. This indicates an effect-scaling discrepancy in the fixture; live
recordings remain unverified. Weighting fractional pixelation-block edges by
area reduced the same comparison to 5.67 levels against decoded export and 4.51
against the full-size preview; that earlier change did not remove the mismatch. A separate
high-frequency stripe probe also shows a difference from Electron's current
pixel sampling. Composing the whole 4K frame at source resolution for every
playback preview would add substantial CPU cost: a local synthetic case measured
41.09 ms at full size versus 4.90 ms at 960×540, excluding decode, resizing and
upload. See `tests/native/evidence/macos-ui/video-redaction-preview-timing-20261116.txt`;
this does not measure live playback or slower machines. The result and
reproducible instrumentation patch are in `tests/native/evidence/macos-ui/video-pixelation-scaled-20261116.txt` and
`tests/native/evidence/macos-ui/video-pixelation-scaled-diagnostic.patch`.

Pixelation now averages aligned source blocks directly and fills repeated
nearest-neighbor samples in runs; fractional block averaging retains the
previous calculation. Generated tests check the block values and boundaries.
A source-resolution crop experiment matched a full composition in one
overlapping-redaction case and corrected the sampled half-size preview in that
fixture. In a local synthetic 4K timing, the crop median decreased from 9.36 ms
to 6.84 ms after the pixelation optimization; a separate small preview took
4.91 ms. See the initial [fixture and timings](tests/native/evidence/macos-ui/video-redaction-crop-20261116.txt).

Scaled video composition now renders active blur and pixelation from bounded
source-resolution crops and copies only their affected output rectangles.
Overlapping source regions are grouped; distant ones are processed separately.
The original generated stripe fixture now has a mean preview-versus-full-preview
error of 0 at its first sampled time and 0.018 at its later sampled times, down
from 4.51. Its decoded H.264 comparison fell from 5.67 to 1.84 levels at the
first sampled time. Generated tests also exercise clipped selections on four
edges, reverse drags, overlapping effects and annotations, a camera layer,
zoom, transparent backgrounds, multiple separate effects, and output scales
from roughly 13% to 150%. A plain source-blend clip was rejected because it
changed pixels outside the mask; bounded output patches preserved neighboring
pixels in the tested cases. This fixes the measured fixture, not the separate
Electron pixel-sampling difference or an untested live recording. In two local
synthetic 4K timing runs, the corrected 960×540 preview took medians of 11.89
and 14.44 ms versus the earlier 4.91 ms direct preview. Decode, upload,
interactive playback and slower Macs remain unmeasured. The [results and
commands](tests/native/evidence/macos-ui/video-redaction-preview-fix-20261116.txt)
record the limits and the isolated fake-daemon checks.

A deterministic 48-case redaction matrix subsequently checked 99,258 pixels
in affected output rectangles, including their boundaries, against the resized
full composition within one channel level. The other 847,734 pixels matched
both the unchanged scaled composition and a preview without redactions exactly.
The drawings used three authoring-canvas sizes. Its 89 generated effects included blur and pixelation at all ten
intensities, clipped and reverse-dragged rectangles, ten overlaps, and selected
camera/zoom cases. A separate generated test checks composed pixels at segment
start and end times, including two effects active at the same instant. A shared canvas region read now copies bounded rows rather
than testing every pixel. It preserves transparent padding for clipped and
off-canvas reads. In two alternating same-process trials on an 800×600 region,
the previous read took upper medians of 2.154 and 2.021 ms versus 0.275 and
0.254 ms for the row copy. Full-playback results under changing machine load
are not comparable; the editor's 60 fps composition budget is about 8.33 ms,
and real playback, decode and slower-Mac behavior remain open. See the
[matrix and timing evidence](tests/native/evidence/macos-ui/video-redaction-region-copy-20261116.txt).

An expanded 512-case version of that generated matrix found a remaining crop
boundary mismatch: 31 channels in 1,137,319 effect-area pixels differed from the
scaled full composition by more than one level. The largest difference was eight
levels in an upscaled 145×97 output; a downscaled 48×32 case differed by three.
In the largest case, removing either zoom or the camera eliminated differences
above one level in the source crop, and removing the camera's rounded corners
had the same effect. Enlarging that particular crop also matched its original
region, but does not establish a generally safe margin. The [measurements and
reproduction](tests/native/evidence/macos-ui/video-redaction-camera-crop-20261117.txt)
and [diagnostic patch](tests/native/evidence/macos-ui/video-redaction-camera-crop-diagnostic-20261117.patch)
use generated content. A coordinate-anchoring experiment did not fix the issue
and was discarded. Real editor display, capture and decoded export remain
unverified for this case.

Intersecting redaction crops now include the complete visible camera clip and
its shadow knockout bounds, and overlapping crops merge. The original rounded
camera case passes within one channel level at the matrix's four output sizes;
hidden camera segments do not expand crops. Repeating the 512-case diagnostic
left 12 channels above one level, at most three levels, in other generated
cases. The original 48 cases and the camera regression check 100,858 effect-area
pixels and 865,863 outside pixels. The composition suite passed 90 tests, and
the existing edited-video export/reopen test passed with development FFmpeg;
that export test does not combine the camera with this redaction case. In one
local optimized test-profile run, a generated 4K frame fitted to 960×540 took
upper medians of 20.44 ms with a separate 320×240 pixelation and 32.15 ms when
it intersected the camera. These are different scenes, not a before/after
benchmark; decode, GPU upload, real playback and slower hardware remain open.
The [commands and results](tests/native/evidence/macos-ui/video-redaction-camera-crop-fix-20261117.txt)
include the residual diagnostic and timing probe.

A separate generated recording combines a patterned camera, rounded corners,
shadow, zoom and overlapping pixelation. At three times its 160×90 composition
preview matched the resized full composition exactly in the 20×20 effect area;
decoded H.264 averaged 0.34–0.36 RGB levels away. Controls confirm the camera
is visible and pixelation changes its pixels. The [test and measurements](tests/native/evidence/macos-ui/rounded-camera-redaction-export-20261117.txt)
use temporary generated media and do not render the editor window or exercise
physical capture. In the largest remaining 512-case mismatch, a separate source
crop comparison differed by 28 levels at one channel on the crop boundary.
Removing rectangles eliminated this difference; removing redactions did not.
The [diagnostic and controls](tests/native/evidence/macos-ui/video-redaction-rectangle-crop-20261117.txt)
identify a separate open crop-edge issue, not a general explanation for every
residual pixel.

Three local trials did not resolve the rectangle crop issue. Reusing the
screenshot path anchor left all 12 greater-than-one-level channel differences;
expanding each crop's margin left five, and including intersecting rectangles'
full bounds left seven. The full-bounds trial also increased a small-redaction
4K synthetic preview from a 5.69 ms upper median without the trial to 40.82 ms
with it when the redaction intersected a full-frame rectangle. The short runs
were sequential and omit decoding and GPU upload. All three trials were removed;
the [measurements and diagnostic patches](tests/native/evidence/macos-ui/video-redaction-rectangle-trials-20261117.txt)
retain their scope and limitations.

The crop now includes intersecting rectangle-corner coverage, derived from the
renderer’s corner radius and stroke width, while leaving distant corners out.
In an expanded 4,096-case generated comparison, channels differing by more
than one level fell from 120 to 17, and the maximum difference fell from 12 to
two. None of the remaining greater-than-one-level differences were new or worse
at their tested positions; the comparison does not rule out changes of one
level. The original failing rectangle cases pass in the regular regression set.
The composition and annotation suites passed, as did a generated camera export
test and an isolated GPUI recording/editor handoff fixture. The latter renders
a rectangle but does not combine it with pixelation.
The full-frame rectangle timing fixture measured a 5.60 ms upper median with
the bounded crop versus 5.69 ms in an earlier baseline run. These short,
sequential synthetic runs omit decode and GPU upload. The [measurements](tests/native/evidence/macos-ui/video-redaction-rectangle-corners-fix-20261117.txt)
retain the commands and the open residual cases; live capture and physical
display verification remain open.

A follow-up grouped the 17 residual channels into seven generated cases. With
zoom disabled those cases had no differences above one level; disabling the
camera or its shadow left 11. In two cases the cropped source already differed
from the full composition by one level with drawings and camera removed, before
output scaling. Combining the zoom transforms or omitting rectangular clipping
did not resolve the tested cases, and both trials were discarded. The
[diagnostic evidence](tests/native/evidence/macos-ui/video-redaction-zoom-edge-20261117.txt)
records the controls and reproduction; the mechanism and a performant fix
remain open.

An isolated GPUI fixture now draws pixelation across a rectangle corner, reopens
the project and captures its 640×360 scrubbing preview before export. In the
affected area, the displayed preview matched its unencoded composition within
one channel level in dark and light fixtures; decoded H.264 averaged 1.17–1.28
RGB levels away, with individual differences up to 42–62 levels. The unredacted
control confirms the effect is visible. Area, window and screen fixture
handoffs used the same generated source. The [dark preview](tests/native/evidence/macos-ui/video-rectangle-pixelation-gpui-dark-20261117.png),
[light preview](tests/native/evidence/macos-ui/video-rectangle-pixelation-gpui-light-20261117.png),
[decoded frame](tests/native/evidence/macos-ui/video-rectangle-pixelation-decoded-20261117.png)
and [measurements](tests/native/evidence/macos-ui/video-rectangle-pixelation-gpui-20261117.txt)
are retained. This does not verify a live recording, hardware access, target
movement or closure, physical display placement, or capture exclusion.

For that generated redaction area, a same-resolution 4:2:0 control was within
eight levels of all 469 GPUI decoded channels whose error exceeded 32; its mean
difference from the decoded frame was 0.170 RGB levels. This is consistent with
chroma subsampling explaining the large errors in this case. An isolated
single-frame Electron WebCodecs probe on the same input also showed large edge
errors, but only 33 of its 333 channels above 32 were within eight levels of
the same control. The [GPUI measurements](tests/native/evidence/macos-ui/video-redaction-chroma-gpui-20261117.json),
[Electron measurements](tests/native/evidence/macos-ui/video-redaction-chroma-electron-20261117.json)
and [method](tests/native/evidence/macos-ui/video-redaction-chroma-20261117.txt)
do not establish equivalence with the complete Electron exporter or other media.

Fitted video previews previously used the unrounded fit scale after rounding
their output dimensions, whereas export scales to the dimensions actually
allocated. A generated 31×19 frame fitted to 12×7 differed in 156 of 336 RGBA
channels, by up to 11 levels, from composition at the same export dimensions.
Fitted previews now use the same dimension-based rendering path as export.
[Native evidence](tests/native/evidence/macos-ui/video-preview-rounded-size-20261116.txt)
covers generated landscape, portrait and odd-sized frames, an edited H.264
export, and isolated fake-daemon recording workflows in both themes. That sizing
fix alone did not resolve the scaled pixelation difference; the later crop change
above addresses the tested fixture. Neither verifies a live recording.

Fractional redaction bounds previously omitted the last partially covered row or
column when calculating the source region. GPUI and Electron now round the far
edge independently before clipping to the canvas for video and screenshot blur
and pixelation; empty selections still draw nothing. Generated tests exercise
pixelation and blur, scaled and clipped bounds, and the previously unchanged
corner pixel. An isolated Electron screenshot fixture checks the actual preview
overlay and reopened PNG at 1× and 2×. That bounds fix alone did not resolve the
scaled pixelation discrepancy above or establish parity for a live recording. A
[macOS fixture](tests/native/evidence/macos-ui/fractional-redaction-bounds-20261116.txt)
also checks the changed corner through Electron's offscreen drawing renderer.

Recording preview controls previously tinted the thumbnail without Electron's
background blur. GPUI now produces a blurred thumbnail from each decoded video
frame while controls are active, using the screenshot preview's blur. On macOS,
the capture-preview unit suite passed 38 tests; a generated decoded-frame test
checks both thumbnail variants against the same pixels loaded as a screenshot.
Blur work runs on the existing background decoder worker and is skipped while
controls are inactive. A newly hovered frame can remain sharp until the next
thumbnail decode completes. An isolated macOS GPUI renderer exercised the real
decoder and simulated hover and exit in dark at 100% and 200%, and light at
125% and 200%.
The [evidence](tests/native/evidence/macos-ui/video-thumbnail-hover-20261117.txt)
includes [GPUI idle](tests/native/evidence/macos-ui/video-thumbnail-idle-gpui-20261117.png)
and [hover](tests/native/evidence/macos-ui/video-thumbnail-hover-gpui-20261117.png)
images. In a control-free region the GPUI hover differed by about four RGB
levels on average from [Electron](tests/native/evidence/macos-ui/video-thumbnail-hover-electron-ready-20261117.png),
after a synthetic media-ready event removed an Electron fixture placeholder.
Without that event the [placeholder covers the hovered video](tests/native/evidence/macos-ui/video-thumbnail-hover-electron-no-ready-20261117.png)
in the fixture. The media readiness follow-up near the end of this audit
covers that race. Neither probe verifies a physical hover, live recording or
desktop compositing.

An ended GPUI audio preview stem could not be replayed: rodio had removed its
source, and a later seek returned without restoring it. This could leave a
shorter music track or program audio silent after seeking back. The preview now
requeues its existing decoded samples when seeking an ended stem, including
paused scrubbing and normal playback. A [macOS software-mixer regression](tests/native/evidence/macos-ui/audio-preview-replay-20261117.txt)
failed before the fix and checks nonzero generated samples after scrub and
replay; it does not use a physical audio device. Live playback, hardware input
and audio-device changes remain unverified.

Replacing a removed music track with a different file of the same name could
leave GPUI previewing the old decoded samples while export read the new file.
Import now invalidates that path and requests an audio rebuild, even when the
replacement has the same track settings. The generated WAV comparison in the
[macOS evidence](tests/native/evidence/macos-ui/music-cache-and-undo-20261117.txt)
reproduced the mismatch before the fix and passed afterward. External changes
to a project file outside the editor remain unverified.

Different music filenames could also produce the same sanitized track ID,
causing edits or removal to affect more than one track. New imports now choose
an ID unused by existing tracks and groups. The same evidence covers three
colliding names, independent volume changes, save/reopen and removal.

Malformed music filenames in a project could previously delete a file outside
the music folder, and an import could follow a dangling destination symlink.
GPUI now rejects parent and absolute names and direct music symlinks, and
creates imports exclusively. The [temporary-file tests](tests/native/evidence/macos-ui/music-path-safety-20261117.txt)
also cover a symlinked music folder and concurrent same-name imports. Directory
checks remain path-based and have not been shown safe against concurrent
replacement of a directory.

Removing a music track previously deleted its source file even though Undo
could restore a reference to it. GPUI and Electron now retain the file while
removing track references, and GPUI removes a split group in one undo step.
[Generated tests](tests/native/evidence/macos-ui/music-undo-retention-20261117.txt)
cover removal, undo, redo and reimport. The later retention section records
how no-longer-referenced files are collected after editors and exports release
their project leases.
GPUI now replaces its editor state atomically and flushes a pending edit on
orderly close. The [persistence checks](tests/native/evidence/macos-ui/video-editor-atomic-save-20261117.txt)
cover concurrent readers, a failed replacement and closing just after undo.
Its native close callback now refuses to close when the last save fails and
saves an unfinished gesture as well. A [simulated window-close test](tests/native/evidence/macos-ui/video-editor-close-guard-20261117.txt)
checks failed save, retry and an unfinished gesture; the native dialog and an
actual macOS window close were not exercised.
Electron now also replaces its video state atomically and waits for queued
edits to save before closing. A failed save leaves its editor open.

Electron autosave previously dropped `sourceDuration`, which both editors and
reset use as recording metadata. It now preserves the saved duration or stores
the discovered duration for a project without one. Reset also writes its
replacement atomically and waits for outstanding saves; saves requested by that
renderer during a successful reset cannot restore stale edits. The [regression evidence](tests/native/evidence/macos-ui/video-editor-reset-persistence-20261117.txt)
covers failed replacement, metadata preservation and concurrent close/reset
using temporary projects and a simulated renderer. It does not verify a live
editor or decoded video export. If new edits arrive while reset is running, the
renderer retains and saves those edits instead of reloading the reset state, and
informs the user. A crash between the reset replacement and that follow-up save
can still leave the reset state on disk; there is no crash-recovery journal for
those in-flight edits.

Initial Electron state creation now uses exclusive file creation, so an editor
that writes state while initial generation is awaiting cursor data is not
overwritten. The same [evidence](tests/native/evidence/macos-ui/video-editor-reset-persistence-20261117.txt)
includes a temporary-project regression for that race. Initial creation still
uses a complete temporary file before publishing it when hard links are
available. For recognized unsupported or denied hard-link errors, it falls back
to direct, exclusive creation; interruption during that fallback can leave an
incomplete file. An interruption can also leave an unpublished temporary file.

Both editors now write compatible project-local retention records for imported
and removed music, and reserve recorded names when importing again. Electron
also creates imported files exclusively to prevent concurrent imports from
overwriting each other. Collection now waits for editor, import and export
leases, recovers PID-qualified session and collector markers after a crash,
and deletes a candidate only when the last valid saved state no longer
references its file. Candidate records are atomically published before
Electron saves or resets the state. The [earlier retention checks](tests/native/evidence/macos-ui/music-retention-records-20261117.txt)
cover the shared record format, filename collisions, undo and redo, and path
rejection; the [cleanup evidence](tests/native/evidence/macos-ui/music-retention-cleanup-20261117.txt)
covers saved references, overlapping editor/export sessions, stale markers,
atomic record repair, and save/reset behavior when directory syncing fails.
These are generated temporary-project tests, not a live editor, export or
cross-process crash. A malformed legacy `.collecting` lock without a usable
PID remains fail-closed and can prevent that project from opening. Electron's
Windows directory-sync path is a no-op, so Windows crash durability and runtime
identity behavior remain unverified; a missing retention marker leaves an
orphaned file rather than authorizing deletion. If GPUI cannot record a
candidate, it logs the error and preserves the audio file, which may then remain
uncollected.

## 2026-11-17 native macOS smoke

The [initial live smoke evidence](tests/native/evidence/macos-ui/macos-live-smoke-20261117.txt)
uses the GPUI development binary with temporary profiles and generated images. It
captures the settings, recording control and screenshot preview in light and dark,
checks the preview's measured bottom-right placement, and verifies screenshot
editor annotation, save and reopen. The generated editor output matches its
reopened PNG; these checks do not invoke ScreenCaptureKit or prove that real
capture controls are excluded from captured pixels.

The [dark](tests/native/evidence/macos-ui/native-settings-appearance-dark-live-20261117.png)
and [light](tests/native/evidence/macos-ui/native-settings-appearance-light-live-20261117.png)
settings images, [recording controls](tests/native/evidence/macos-ui/native-recording-control-dark-live-20261117.png),
[capture preview](tests/native/evidence/macos-ui/native-capture-preview-dark-live-20261117.png),
[physical light-theme preview hover](tests/native/evidence/macos-ui/native-capture-preview-light-physical-hover-20261117.png),
and [saved editor output](tests/native/evidence/macos-ui/macos-screenshot-editor-arrow-save-reopen-20261117.png)
are retained.

The initial pass checked timer edge geometry but did not run a countdown or real
capture. At that point the GPUI development daemon's Screen Recording grant had
not been verified, so recording, target movement or closure, and device access
were deferred. No permission dialogs or privacy settings were touched. The
follow-up below supersedes that initial capture and recording status. The live
Electron workspace was left untouched; visual comparison used the Electron
control image supplied in this thread.

### 2026-11-17 native capture and recording follow-up

The GPUI onboarding status image
[native-permission-status-onboarding-20261117.png](tests/native/evidence/macos-ui/native-permission-status-onboarding-20261117.png)
shows Screen Recording and Accessibility granted for this development app. A
real GPUI screen recording also succeeded. No permission prompts or privacy
settings changes were involved. The Mac has one built-in Liquid Retina XDR
display at 3456×2234 pixels, so multi-display placement remains unverified.

The real screen take is H.264, 3456×2234, 199 frames, 19.987 seconds. The editor
test imported that captured movie, drew an annotation, saved and reopened the
project, exported it, then compared decoded export pixels with the reopened
preview. Mean channel error was 0.92 on the first frame and 0.38 at 13.9 seconds;
the edited pixels matched the decoded export better than the unedited preview.
The captured display included macOS notification banners and a purple status dot.
It contained no Poratake recording bar or capture overlay. That frame was kept in
the temporary smoke directory because the OS notifications are not suitable as
repository evidence.

A real area take used a 608×360-point region of a generated QuickTime pattern
window and produced H.264 at 1216×720, 179 frames, and 29.145 seconds. The saved
frame shows only the generated pattern and its embedded timecode, with no Poratake
controls or selector. The video editor test saved and reopened an annotation,
exported the take, and compared decoded frames with the preview: mean error was
0.77 on the first frame and 0.38 at 20.4 seconds. The captured system-audio
sidecar was AAC, 48 kHz, stereo, and 29.163 seconds. A generated 997 Hz tone was
present from 10.5 to 13.5 seconds; the same tone survived editor export. The
native smoke harness keeps system audio disabled by default and opts in with
`PORATAKE_NATIVE_SMOKE_SYSTEM_AUDIO=1`.

A real window take followed a generated QuickTime target after moving the window
from `(108,141)` to `(250,220)`. It decoded as H.264, 1280×720, 1590 frames, and
159.773 seconds; its saved frame contains the test pattern without recording
controls or the window outline. A second take finalized after the generated
QuickTime target process closed, as H.264, 1280×720, 1592 frames, and 160.083
seconds. The video editor test imported this take, edited, saved and reopened it,
exported it, and compared decoded frames with the preview: mean error was 0.69 on
the first frame and 0.40 at 112 seconds. The
[area metrics](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/area-export-metrics.json),
[window metrics](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/window-export-metrics.json),
[screen metrics](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/screen-export-metrics.json),
[area source frame](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/area-recording-frame.png),
[area editor preview](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/area-editor-preview.png),
[area export frame](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/area-export-frame.png),
[window source frame](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/window-recording-frame.png),
[window editor preview](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/window-editor-preview.png),
and [window export frame](tests/native/evidence/macos-ui/video-recording-editor-smoke-20261117/window-export-frame.png)
are retained beside this audit.

The app's Export Video button opened the native Save sheet in the scratch editor,
but the background computer-control helper could not target that AppKit sheet
(its child window was unavailable and its accessibility IDs immediately went
stale). No export was claimed from that button interaction. The editor's existing
UI integration test exercised the same editor export path directly with each
real captured movie and compared its decoded output with the reopened preview.
This verifies the editor and exporter, but leaves native Save-sheet interaction
unverified.

The real screenshot area, window, and timer captures saved only the chosen test
content and cleaned up their selectors. Screenshot annotation, save and reopen,
light and dark settings, the narrow 720×520 video editor, and bottom-right preview
placement were also checked. Hover statuses appeared, but a native tooltip bubble
was not visible in the window-only screenshots. The scroll-capture UI was opened,
but no scroll image or history item was produced; the fixed-header scroll-stitch
issue remains open. Microphone and camera capture were not attempted because
authorization for this GPUI identity was not confirmed. No settings were changed
and no dialogs were triggered.

### 2026-11-17 native tooltip placement follow-up

The user-provided [before image](tests/native/evidence/macos-ui/native-recording-tooltip-before-owner-frame-20261117.png)
shows the protected recording tooltip overlapping the toolbar by about four
points. GPUI now sends the owner window frame with tooltip requests. The native
panel uses that frame to sit outside the control, and moves above an owner near
the bottom edge. A synthetic hover through the GPUI tooltip dispatcher reached the
development daemon. The [after image](tests/native/evidence/macos-ui/native-recording-tooltip-after-owner-frame-20261117.png)
shows the bubble below the real GPUI recording-control window with a four-point
gap; a synthetic exit removes it, as shown in the [hidden image](tests/native/evidence/macos-ui/native-recording-tooltip-hidden-owner-frame-20261117.png).
The [same-pointer image](tests/native/evidence/macos-ui/native-recording-tooltip-same-pointer-after-20261117.png)
also confirms the corrected gap at the Start recording control.
The [native fixture](tests/native/evidence/macos-ui/native-recording-tooltip-placement-20261117.txt)
checks no overlap below the toolbar and above a bottom-edge control. Normal
unpressed pointer transitions remain unverified.

The [physical pointer follow-up](tests/native/evidence/macos-ui/native-tooltip-physical-pointer-20261117.txt)
used the normal floating window level and the already authorized computer-use
helper. Dragging onto Start recording, Close, Close preview and Delete screenshot
produced the corresponding tooltip statuses. macOS listed a protected tooltip
window with a four-point gap below the recording bar or above the preview.
After the pointer left the preview, the GPUI tooltip status cleared. The drags
held a mouse button while moving. The November 19 follow-up in the same evidence
file used unpressed desktop-level mouse events in light and dark. WindowServer
observed the expected tooltip on each control and its removal on exit. This
covers the recording bar and screenshot preview on the built-in Retina display.

A [screen-only daemon recording](tests/native/evidence/macos-ui/native-tooltip-exclusion-area-20261117.txt)
then tested the protected panel after an area stream started. Seven decoded
frames taken with a visible timing marker matched the baseline around the
tooltip exactly; an ordinary window at the same bounds appeared in subsequent
frames. The temporary daemon disabled audio and input tracking and preflighted
Screen Recording access. This direct RPC check did not verify the GPUI handoff.

A subsequent [GPUI area recording](tests/native/evidence/macos-ui/native-gpui-tooltip-exclusion-area-20261117.txt)
opened the protected tooltip from the live recording control while a green
timing marker appeared. The first decoded marker frame matched the baseline
around the tooltip; an ordinary window at the same bounds appeared later. The
recording bar and selection borders were absent. This test used a temporary
daemon with audio and input tracking disabled, and leaves screenshot exclusion,
other recording targets, unpressed hover and multi-display behavior unverified.

### 2026-11-17 native GPUI scroll follow-up

The [live scroll check](tests/native/evidence/macos-ui/native-gpui-scroll-20261117.txt)
selected a generated numbered page in GPUI, started auto-scroll and finished it
through the floating controls. The saved 920×4280 PNG and a history entry were
created; selector and control windows cleared. The output matches a continuous
reference within two RGB levels for 97.86% of pixels, with a maximum difference
of 15. No capture controls or boundary appeared in the output. This case has no
fixed header or footer; the ambiguous fixed-header stitch remains open.

### 2026-11-17 tooltip centering and native export follow-up

At `c0c4a02b`, the native tooltip sizes its AppKit text field to the fitted label
and uses equal vertical insets. The
[composited Retina images](tests/native/evidence/macos-ui/native-tooltip-centered-appearances-20261117.png)
show single-line, wrapped and four-line truncated labels in Aqua and Dark Aqua.
The measured top and bottom ink margins were 13/10 pixels for Start recording,
13/13 for the wrapped label, and 14/13 in light and 13/13 in dark for the truncated
label. All six panels stayed four points clear of the owner frame. The optimized
Swift fixture passed the rendered-pixel assertions for the short and wrapped labels
and the existing four-line and placement checks. The old rendering failed the new
centering assertion with 9/14-pixel margins.

The same AppKit probe tested screenshots through the production ScreenCaptureKit
exclusion helper over a generated solid background. All six filtered captures were
byte-identical to the reference with the tooltip hidden. A magenta panel at the
same level and position appeared in its positive-control capture. This verifies
native tooltip exclusion in that helper. The earlier GPUI recording test covers
one area-recording handoff. Ordinary unpressed physical hover remains open.

An isolated offscreen Electron renderer showed the corresponding Start recording
control in both appearances, with a 32×32-point target and the same accessible
name. The fixture captured the toolbar but did not paint a tooltip bubble. The
native AppKit images above provide the tooltip placement and alignment evidence.

The native Save sheet was also used to export a generated color-bar clip from an
isolated GPUI editor. The resulting H.264 MP4 decoded at 3840×2160, with 60 frames
over two seconds, and the decoded first frame showed the expected color bars. The
earlier real screen, area and window takes supply the preview/export comparisons.

### 2026-11-17 live screenshot diagnostic

An independent daemon call excluded the live GPUI selector while it was still
visible. Two filtered 840×520 captures matched the decoded RGB of the generated
pattern exactly. In a later run, 946 of 436,800 pixels differed along two sharp
color boundaries, with a mean channel difference of 0.075.

The screenshot requests initiated from `--native-smoke-recording` repeatedly
failed on macOS 27.2. The area selector closed, the five-second timer completed,
and both requests passed the temporary daemon's screen-access preflight and
obtained ScreenCaptureKit shareable content. Neither completed
`SCScreenshotManager.captureImage` before the app's 30-second RPC deadline.
The window picker sent the generated window's ID and closed. Its daemon returned
`CAPTURE_FAILED` from the window-list image path. An independent instance of the
same daemon could save the generated window and area to pre-created temporary
PNG paths. The area, timer and window screenshot reruns remain open despite the
earlier successful live captures. The diagnostic used generated windows and an
isolated profile with onboarding completed. The temporary daemon disabled audio
and input tracking and preflighted screen access before screenshots.

A physical drag in the normal `--intent capture-area` selector reproduced the
area stall with a 420×260-point selection. macOS reported the selector gone
while the daemon was still awaiting `SCScreenshotManager.captureImage`. A
temporary build that forced the existing window-list capture path returned
`CAPTURE_FAILED` for both area and timer. The failing capture paths remain
open. Foreground control was released immediately after the drag. The existing
editor stayed open throughout.

### 2026-11-17 serialized screenshot verification and timeout recovery

The earlier failed runs kept a second copy of the temporary daemon open to
collect reference captures. With that process still open, the second daemon
stalled on a direct area request before GPUI launched. The newer filtered
ScreenCaptureKit screenshot API and a one-frame ScreenCaptureKit stream also
stalled in this setup. Keeping the dismissed GPUI selector's native window
alive did not recover the capture.

The smoke passed with one test daemon running at a time. GPUI saved a live
840×520 area PNG in 0.32 seconds, an 840×520 timer PNG after 5.22 seconds, and
a 1000×640 window PNG in 0.20 seconds. Each selector cleared and a 208×148
preview opened at the bottom right. The timer target changed from green to
magenta during the countdown, and the saved image contained the magenta block.
References captured the generated window in a separate process after GPUI
exited. Across decoded RGB channels, the area and timer PNGs differed from the
cropped references by at most two levels. The window PNG matched exactly.

A physical drag through the normal `--intent capture-area` selector also
saved an 840×520 PNG and opened the bottom-right preview. A generated backdrop
covered other desktop windows during the drag. The screenshot differed from
the cropped window reference by at most two RGB levels. Foreground control
was released after the drag and the existing editor stayed open.

The filtered screenshot waits up to 15 seconds for a ScreenCaptureKit frame
before returning an error and releasing the capture-exclusion barrier. A native
regression exercises a missing callback followed by a late result and another
successful request. With two daemons deliberately active again, the live area
request returned `CAPTURE_FAILED` after 15.33 seconds. A pending protected
tooltip became visible in the same daemon immediately afterward, dismissed
normally, and no PNG was published for the timed-out request.

### 2026-11-17 and 2026-11-18 Display P3 screenshot color verification

The 840×520 native area fixture carries a Display P3 ICC profile. GPUI's old
decoder treated its channels as sRGB. The green sample at (450, 115) therefore
exported as `[100, 204, 101]` while the ColorSync sRGB reference measured
`[39, 207, 86]`. The native editor window showed the same color error after
its captured pixels were converted to sRGB for comparison. An offscreen canvas
in Poratake's Electron 43.4.0 runtime measured `[39, 207, 86]` as well.

GPUI now converts embedded RGB ICC profiles into its sRGB working space
before preview, editing, pinning, color picking, thumbnail generation, and
clipboard encoding. The original capture keeps its embedded profile. The
decoder preserves alpha and 16-bit precision. Edited exports and clipboard
PNGs carry a standard PNG sRGB tag. The tag has no timestamp, so re-exporting
the same image produces identical bytes. Existing screenshot history thumbnails
regenerate from the original using a versioned cache key, with the previous
thumbnail available when the original cannot be decoded.

The [native comparison](tests/native/evidence/macos-ui/native-p3-editor-comparison-20261117.png)
shows the editor window before and after conversion. The corrected green sample
exported as `[39, 207, 86]`, and the full PNG differed from ColorSync and Electron
by at most one RGB level per channel. An sRGB input exported
pixel-identically. A generated 16-bit Display P3 input also matched the ColorSync
reference exactly in the editor's 8-bit export. Sampling the corrected native
window and the window opened from an already-converted sRGB reference produced
the same green at (1100, 620).
The headless Metal parity runs also completed with the tagged native file in the
default dark editor and a 950×700 light editor.

### 2026-11-18 macOS scroll capture with a fixed header

The previous duplicate detector could discard changed body content when its
sampled strip stayed fixed. A generated regression with an unchanged header
and green-only changes below it failed against the old detector. macOS scroll
capture now samples RGB across the frame and compares full rendered pixels
when those samples hash to the same value. The native fixture covers real
duplicates and a fingerprint collision from an alpha-only change.

The live GPUI test selected a 460×380-point region with a stationary 72-point
header, then scrolled a generated page to its expected offset of 772 points.
Done saved four frames as a 920×2304 PNG and opened the bottom-right preview
with one new history entry. The header appears once in rows 0–143 of the output.
Of 2,119,680 pixels, 2,100,262 were within two RGB levels of the full-page
reference. The mean absolute channel difference was 0.434, with a maximum of 18. Foreground control was released, and the existing editor stayed open.
The [generated page fixture](tests/native/evidence/macos-ui/native-gpui-scroll-fixed-header-pattern-20261118.swift)
can render its reference directly to a PNG given an output path.

Alignment rules are still the same. Cases where distinct scroll histories
produce the same visible frames remain open.

### 2026-11-18 macOS scroll seams under a taller fixed header

The previous exact fallback rejected a three-frame capture when a fixed header
covered part of the matching area. The fallback now tries matching below the
unchanged top rows when the whole-frame comparison has no usable overlap. It
requires a unique exact body overlap and retains the header in the first frame.
The native fixture covers 28 visible matching rows without a footer and 16 with
a footer. For both cases, the reopened PNG matches the generated document and
the preview pixel for pixel. A repeating body with multiple exact alignments
still stops. The fixture reproduced the early stop before the fix.

A separate GPUI selector run used a 460×380-point generated page with a
102-point header. Done saved two frames as a 920×1284 PNG, opened a preview and
added a history item. Compared with the rendered document, the mean RGB channel
error was 0.410 before the seam and 15.116 after it. The latter fell to 0.456
when the reference was shifted by eight pixels, confirming skipped rows at the
seam. The live capture was stopped partway through the page. The exact fixture
passed while that live run still skipped rows.

### 2026-11-18 macOS short scroll overlaps with capture noise

The same generated page was captured through ScreenCaptureKit at offsets of
0, 266 and 532 points without moving the system pointer. Each 920×760 frame
contains a 204-pixel stationary header. The known 228-pixel overlap leaves
24 comparable body rows. With the original 40-row strip, part of each
comparison came from the header. The matcher selected 236 for the first pair
and rejected the second. On the comparable body rows, the mean RGB channel
error at 228 was 0.465 and 0.464, versus 9.369 and 15.862 at 236.

The matcher now scores only the visible body rows for short overlaps and
checks competing offsets before accepting a short seam. It uses the existing
minimum row count and color error limits. Fixtures cover slight capture noise
in the header and body, with and without a footer, and reject repeating body
patterns at two scroll offsets. Both seams between the three native frames aligned at 228.
The reopened 920×1824 PNG matched the preview and the expected source rows
pixel for pixel.

A separate GPUI run used the same matching rules before the optimization
that reuses the extracted strips. Auto-scroll reached the bottom of a shorter
generated page. Done saved two frames as a 920×1292 PNG, opened the preview and
added a history entry. Against the rendered reference, mean RGB channel error
was 0.410 in the first frame and 0.456 in the appended rows. Over the full image,
99.79% of channels were within two levels and the maximum difference was five.
Foreground control was released, the test apps closed, and the existing editor
stayed open. Cases where the same visible frames support distinct scroll histories
remain open.

### 2026-11-18 unpressed pointer entry and exit on macOS

The isolated native GPUI probe used its normal floating windows at layer 101
and the screen-only development daemon. Zero-delta foreground scroll actions
moved the system pointer between generated test backdrops and the controls.
AppKit reported zero pressed buttons. A desktop window above the first test
point caused a missed entry. The successful check used (748, 91), below that
window, where Core Graphics listed the recording bar as the top window.

In dark mode, separately entering the recording bar over Start recording and
Close displayed the named native tooltip four points below the bar. Each exit
cleared the GPUI status and removed the tooltip from the window list. Entering
the unfocused preview over Close preview and Delete screenshot displayed their
tooltips four points above the card. Its pointer flag cleared on exit and the
daemon's window list was empty.

In light mode, moving directly from Start recording to Close preview cleared
the first owner and left one native tooltip above the preview. The daemon
received the light appearance for both panels. Leaving the preview removed
the remaining panel. The helper was disabled and the isolated windows closed.
The existing editor stayed open.

The zero-delta scroll also moved the pointer between buttons inside the
recording bar while leaving the Start recording tooltip visible. A follow-up
used a temporary probe to post unpressed mouse-move events to the AppKit event
queue after the pointer entered each native floating window. In dark mode, the
recording bar switched from Start recording to Close, and the unfocused preview
switched from Close preview to Delete screenshot. For both windows the daemon
acknowledged hiding the old panel and showing the new panel for the same owner.
Leaving the preview cleared its hover state and hid its tooltip. Foreground
control was released, both test apps closed, and the existing editor stayed
open. Physical mouse or trackpad movement between buttons remains to be checked.

### 2026-11-18 macOS microphone format and live toggles

A read-only preflight inside the test daemon reported microphone access granted
and camera access undetermined. The live test used a generated area backdrop,
disabled camera and system audio, and left privacy settings unchanged. Enabling
the microphone during recording initially caused the recorder to stop. The
48 kHz float32 mono input was non-interleaved, and the AAC writer rejected a
493-frame trimmed silence buffer with `AVFoundationErrorDomain -11800` and
underlying `NSOSStatusErrorDomain -16122`.

The focused native regression reproduces that error with 512-frame planar
mono samples and partial leading silence. The microphone path now reuses the
system audio interleaving step before timing and silence padding. The focused
regression and existing static microphone timing fixture both pass. The full
timeline fixture initially stopped at a camera-range assertion on macOS 27.2.
The same assertion failed with the unchanged branch source and fixture.

After the fix, GPUI started and stopped a 4.41-second area recording and
opened its preview. RPCs sent to the same daemon enabled and disabled the
microphone twice. The saved 48 kHz stereo AAC track had active samples in both
intervals, with 1.25 seconds of leading silence and a 0.88-second silent gap.
The audio was decoded to check timing and then removed with the temporary test
project. Camera capture and microphone editor export remain open.

### 2026-11-18 macOS timeline and editor export fixtures

The camera-range assertion was caused by a fixture feeding video buffers to
the audio callback. The recorder rejected those buffers before their timestamps
could resolve the pause. With valid 48 kHz PCM at the same timestamps, the full
native timeline fixture and its CI variants passed on macOS 27.2. This includes
camera visibility across a pause and resume while the screen is static.

Three GPUI editor comparisons passed using development FFmpeg. They cover a
delayed camera toggle after edits, camera ranges and zoom across a cut, and
microphone timing after a trim. They compare the preview with decoded export
using generated media. Live camera capture and a live microphone export
comparison remain open.

### 2026-11-18 live macOS microphone editor export

The isolated GPUI test recorded a generated backdrop for 4.883 seconds with the
already-authorized microphone enabled in two short intervals. The 48 kHz stereo
AAC sidecar had 1.71 seconds of initial silence and a 0.90-second silent gap.

A temporary test harness loaded the saved project, saved and reopened a
two-segment edit, built the editor's microphone preview mix, and ran the GPUI
MP4 exporter. It checked 358,288 interleaved samples against the source after
the cut to confirm the exact trim position. The preview and decoded export
retained the silent gap. Their RMS energy envelopes had a cosine agreement of
0.99999 over 43 windows of 100 milliseconds each. The comparison reported
timing and levels, and the temporary project and export were removed.

### 2026-11-18 native macOS pending screen freeze handoff

An isolated GPUI development app used the authorized Swift daemon on the
built-in Retina display. A temporary test bridge delayed successful freeze
and release replies while the daemon performed real screen captures and
AppKit window operations. Core Graphics window enumeration tracked the
daemon’s full-screen frozen overlay and GPUI’s selector independently.

- Replacing a pending area capture with Scroll Capture left GPUI without a
  selector while the original freeze reply was held. The native frozen window
  disappeared on release. GPUI opened one live selector after the release
  reply arrived, and cancellation cleared it.
- Cancelling the pending area capture cleared the native frozen window after
  the reply was delivered. The cancelled selector did not reopen.
- Replacing a pending area capture with Window Capture released the original
  frozen window before creating a new one. GPUI then queried the window list
  and showed one picker. Closing the picker released the second frozen window.

The test used a generated full-screen backdrop and a temporary profile. The
test windows closed, the existing editor stayed open, and no captures were
saved. The native run covers successful freeze and release on one display.
Failure handling has fixture coverage. Native Linux and multiple displays
remain open.

### 2026-11-18 macOS frozen and live area pixels

The real GPUI area selector was opened twice over a generated AppKit
background on the Retina display. After each selector opened, the target
changed color and moved a marker. Both pointer drags used the same
300×210-point area and saved a 600×420 PNG. Each capture opened a
bottom-right preview and added one history entry.

With Freeze Screen enabled, GPUI requested cached pixels and retained the
original marker and colors. GPUI saved sRGB and the direct reference used
Display P3. After ColorSync conversion to sRGB, the saved pixels differed
from the original by 0.560 levels per RGB channel on average, with a maximum
of eight. The changed reference differed by 126.004 levels on average. The
native frozen window and GPUI selector cleared after save. With Freeze Screen
disabled, GPUI requested live pixels and the saved result matched the changed
reference pixel for pixel after the same conversion. In both cases the
changed background was visible after capture.

A setup that kept a second reference daemon open timed out in the freeze
request. A process sample found the Swift daemon waiting in the legacy window
list capture through ScreenCaptureKit. The successful pair closed each
reference daemon before starting GPUI. The Electron area-capture and
freeze-screen suites passed all 35 tests. The generated backdrop and temporary
apps closed, and the existing editor stayed open.

### 2026-11-18 macOS Settings accessibility at minimum size

The native Settings window measured 880×700 points. The original accessibility
tree exposed the toggle, dropdown and slider controls without names. The
history slider advertised increment and decrement, but both left its value at
100 because the component's continuous increment was smaller than the setting's
10-item step. Electron already passes that step to its slider.

GPUI now names the standard controls from their visible Settings labels and
uses the configured step for Settings sliders. Native AXPress opened and closed
the named Appearance dropdown. Keyboard selection changed Dark to Light, and
AXPress on the Dark option switched it back. The named Enable history switch
hid and restored the Maximum items slider. Native increment and decrement moved
it from 100 to 110 and back. Setting the numeric AX value to 137 snapped it to
140 and persisted that setting. The GPUI render tests also passed, including
both keyboard directions on the history slider.

These checks used a temporary profile and the native macOS accessibility
adapter. A screen reader pass and equivalent checks on other operating systems
remain open.

### 2026-11-18 native macOS History contrast and spacing

The 400×500-point History window was captured on the Retina display with
three generated screenshots. In dark mode over a white test backdrop, the
original control surface measured RGB (255, 255, 255), making the light heading
and filter text barely visible. The three-item grid also stretched its
second row to leave a 65.5-point gap. Electron's grid specifies 12 points.

The window now paints a tint from the active theme over the blurred surface
and keeps grid rows at the top of the scroll area. The same native screenshot
measured RGB (8, 8, 10) in the dark surface and a 12-point row gap. A temporary
test forced the native popover to 140×500 points. Its light compact list
wrapped the toolbar and kept the thumbnails and labels inside the window
over a dark backdrop. The opaque fallback for reduced transparency was
retained and the operating system setting stayed unchanged.

Clicking the first teal capture opened that image in the editor. Its folder
button opened the generated captures folder in Finder and selected the teal
PNG. The test windows closed and the existing editor stayed open. A live
History tooltip and multi-display placement remain to be verified. All seven
History tests passed, including the new 2× grid gap assertion.

### 2026-11-18 native macOS screenshot editor attachments

An isolated GPUI editor started with a generated 600×420 PNG. The right
and top attachment selectors each captured a 300×210-point area at 2× with
Freeze Screen enabled. The window list showed the same editor hiding before
both selections and returning after both freezes were released. The test
used a different generated pattern for each edge. An earlier attempt with
an overlapping system window was discarded before export. The successful
run used an independently checked capture region.

The native Save Screenshot sheet saved a 1200×840 PNG. The base pixels
matched exactly and the unused quadrant was fully transparent. With the
native references converted to sRGB, mean RGB channel differences were
0.605 on the right and 0.566 on the top. Maximum differences were seven
and five levels. Both attachment temporary files were removed. A fresh
native editor opened the saved PNG. The file was decoded for pixel
comparisons and the composition was inspected in the editor before saving.

The third selector stayed open after the desktop helper attempted Escape.
The helper could not target that temporary selector directly.
The isolated processes closed and the existing editor stayed open.

A follow-up opened the exact saved PNG in the offscreen macOS Metal
renderer. In light and dark mode, every rendered image pixel matched the
PNG composited over the corresponding editor background at 1× and 2×.
The viewports were 1440×1060 and 950×700 points, respectively, so the zoom
controls stayed clear of the image. Re-exporting the reopened document
preserved all 1,008,000 RGBA pixels, including the transparent quadrant.
The focused Escape fixture and pending-freeze checks passed. Native
keyboard cancellation is still open.

### 2026-11-18 macOS painted camera and redaction preview

A generated project used a rounded camera with shadow, 1.5× video zoom and
pixelation across the camera edge. The camera was visible from 0 to 0.75
seconds and from 1.25 to 2 seconds. The offscreen macOS editor painted the
saved project at 0.25, 0.5, 1 and 1.5 seconds in a 1280×750-point dark
window at 1× and a 720×520-point light window at 2×. The camera appeared
and disappeared at the expected times in both profiles. At one second,
the camera-region content matched the no-camera control exactly before
display scaling.

The GPUI exporter produced a 320×180 H.264 file. With the decoded frames
resized to match the preview using bilinear sampling at 2×, the mean
RGB difference over the full image was 0.899–0.951 levels at 1× and
0.608–0.651 at 2×. The pixelation region averaged 1.025–1.895 levels at
1× and 0.774–1.414 at 2×. The existing composition and decoded-export
fixture also passed. The generated camera clip required no device access.
Live camera and playback performance remain open.

### 2026-11-18 macOS 4K preview timing across camera gaps

A generated 3840×2160 H.264 recording and 1920×1080 camera clip at 30 fps
exercised zoom, a rounded camera with shadow, and overlapping pixelation.
With the camera visible, the original fixed 640×360 preview pipeline had a
43.2 ms median. Sequential seeks through the offscreen editor, including
Metal rendering and readback at 2×, had a 59.1 ms median and selected a
384×216 preview by the end of the run. These measurements cover the local
offscreen path. Interactive frame pacing remains open.

The decoder now stays warm while a later camera range exists, and the
preview builds camera pixmaps only for visible frames. The paired tests
measured 52 frames inside a camera gap. The decode, composition, and GPUI
image median went from 14.2–17.4 ms to 12.3–12.4 ms. A trial that skipped
all hidden camera decoding delayed the first visible frame to 263–265 ms.
Keeping the decoder warm returned the transition to 44–45 ms. The before
and after runs produced identical checksums across the hidden and visible
frames. Decode is skipped completely when the camera is disabled or its
last visible range has ended. All 92 runnable composition tests passed,
along with the FFmpeg preview visibility and rounded-camera export tests.

### 2026-11-18 macOS 4K camera shadow reuse

The visible-camera version of the 4K fixture spent a median 34.3 ms in
composition at 640×360. Disabling the shadow reduced that stage to
16.0 ms. The full-resolution redaction crop was rasterizing and blurring
the same rounded shadow on every frame.

The engine now keeps up to two exact shadow masks per composition, at up
to 4 MiB each. The raster key includes the camera path, fill rule,
dimensions, transform, blur and tint. Placement, clip and opacity are
applied when each frame is painted. Oversized masks still use the normal
rasterizer.

In the run with steadier decode timings, the composition median was 22.0 ms
with reuse. Decode, composition and GPUI image preparation together took
32.4 ms versus 44.0–44.1 ms in the uncached runs. Four alternating cached
and uncached runs also produced identical frame checksums. Decode and
Metal readback times fluctuated substantially in those runs. Interactive
4K frame pacing remains unverified. All 11 canvas tests, 92 runnable
composition tests and the rounded-camera decoded-export test passed.

### 2026-11-18 macOS recording reopen and regression checks

An offscreen 4K trial overlapped the screen and camera decode requests.
The frame checksums matched through continuous camera visibility and a
visibility gap. In three alternating runs, median preparation time was
28.9–29.5 ms with the existing decoder flow and 28.8–28.9 ms with overlap.
The first visible frame after the gap took 42.4 and 43.1 ms, respectively.
The overlap experiment was reverted because the runs showed little gain.

The full dark Metal workflow then failed when reopening the generated area
recording. An isolated reproduction reported that music was being cleaned
up, even though that project had no retention records. The background
close worker briefly held the collector marker while checking for work.
GPUI now skips that worker and collector lock when the records directory
is missing or empty. The next open still recovers crashed session markers.
Regression coverage checks both directory states and immediate reopening.

The local macOS runs passed 1,150 GPUI tests and 2,414 repository tests.
The Swift daemon type-checked for arm64 and x86_64 against the local SDK.
After the reopen fix, the dark Metal run reached two stale selection
expectations in the wallpaper fixture. The fixture now expects the
upper annotation at the overlapping point, following the established
paint order. The focused Metal check passed with both corrected cases.
The full offscreen dark workflow then completed at 1280×738 and 1×,
producing 457 GPUI PNG artifacts. It exercised recording reopen and
export, timer, analysis, scroll, attachments, pin, and screenshot
save/reopen workflows with generated media and the fixture daemon.

### 2026-11-18 macOS light Retina layout verification

Both light offscreen Metal profiles passed at 2×. The 950×700-point
profile produced 194 GPUI PNGs, including History grid and compact list
checks at 140, 200 and 240 points. The 720×520-point profile produced
169 GPUI PNGs and verified access to the narrow video editor's horizontally
scrollable controls by keyboard.

Visual inspection confirmed that the 140-point History list showed the
complete Show in folder tooltip and that the focused Scrub Audio control
and its focus ring fit inside the 720-point video editor. Both profiles
also checked capture-preview placement, image pixels and editor handoff.
Physical display transitions and live pointer delivery remain open.

### 2026-11-18 Intel macOS verification under Rosetta

The GPUI shell passed Cargo's offline `x86_64-apple-darwin` compile check.
The `x86_64` GPUI test executable ran under Rosetta on macOS 27.2 on
Apple Silicon. It passed 1,151 tests, with 23 marked ignored and no failures.
The `x86_64` Swift recording timeline executable passed 24 translated runs,
covering the baseline, every macOS CI timeline variant and the planar
microphone regression. The fixtures used generated samples to check
microphone timing and silence padding, system-audio continuity, static video
and writer finalization. Physical Intel Macs still need runtime verification.

### 2026-11-18 GPUI Metal rendering under Rosetta

An `x86_64` development build ran the offscreen Metal profile on the same
Apple Silicon Mac as the arm64 reference. At 720×520 points and 2× in light
mode, all fixture assertions passed. Of 169 paired PNGs, 149 matched byte for
byte. Six screenshot toolbar states differed by one 8-bit channel level in
one or two pixels each. Fourteen loaded video editor states differed by at
most one level per channel inside the generated video image. The arm64
reference repeated byte for byte across two runs. Visual inspection of the
translated video editor confirmed the focused Scrub Audio control and its
focus ring fit inside the narrow viewport.

The 1280×738-point dark profile at 1× completed on both builds. It exercised
recording reopen and export, timer, OCR and QR analysis, scrolling, attachments,
pin and screenshot save/reopen using generated media. Of 457 paired PNGs,
442 matched byte for byte. The 12 loaded video editor states differed by no
more than one channel level inside the video image. The three edited recording
views differed in their generated timestamp titles, one pixel per view outside
the title and preview, and up to two levels per channel inside the preview.
The translated area, window and screen recording previews had mean channel
errors of 1.533–1.535 against their decoded exports at the sampled times.
This checks the translated build on Apple Silicon Metal. Physical Intel
hardware, mixed-DPI display transitions and live input still need verification.

### 2026-11-18 macOS video timeline label contrast

The rendered light Retina video editor showed minimum measured contrast of
3.59:1 for the video duration, 2.72:1 for the rectangle label and 2.55:1 for the
audio duration. GPUI painted that audio duration with translucent window
foreground, which is dark in light mode. The isolated Electron fixture
confirmed the orange clip gradient and white duration with a drop shadow.

On macOS the GPUI labels now use white text over a compact, translucent dark
backdrop. The clip gradients remain visible around each label and retain their
existing roles. In the final 720×520 light profile at 2× and the 950×700 dark
profile at 1×, the sampled video, drawing and audio labels measured 11.92:1 or
higher. Visual inspection verified space above and below the label backdrops.
Both Metal profiles passed the rendered contrast assertion for video and audio
durations, and all 48 focused timeline tests passed.

### 2026-11-18 GPUI split audio track mute on macOS

The Audio Tracks panel showed one switch for each group but its handler
changed only the first clip. Muting a split track could leave later clips on
the timeline and in the audio mix. The pointer regression reproduced the issue
with a saved project where the first clip was already disabled. Electron's
switch updates every clip in the group and its timeline shows enabled clips.

The GPUI switch now reflects whether any clip in the saved group is enabled
and changes the entire group in one undoable edit. Fully muted lanes disappear
and the other tracks keep their current state. The regression verifies pointer
toggles, undo, redo, the persisted mute state, and the shared audio stem plan
used by preview and export. A clip without a group ID uses its own ID as before.
The arm64 GPUI suite passed 1,151 tests with 23 marked ignored. A separate
macOS development-FFmpeg run passed the generated sidecar mute comparison
between preview audio and decoded export.

The 950×700 dark Metal profile at 1× passed. All 194 PNGs matched the prior
profile byte for byte, including the Audio Tracks panel with one enabled track.
The grouped pointer regression covers the state change that this visual profile
does not exercise. Audio-panel volume and speed parity for split groups remains
open.

### 2026-11-18 macOS popup mouse-move delivery

An isolated hidden AppKit panel used a tracking area with MouseMoved and
ActiveAlways. Adding that area and removing the window frame left the native
window's acceptsMouseMovedEvents setting disabled. A dispatch probe sent two
unpressed moves through NSApplication. The panel received neither with the
setting disabled and received both with it enabled. The probe observed
window-level dispatch without moving the system pointer. [AppKit event guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/HandlingMouseEvents/HandlingMouseEvents.html)

The GPUI popup probe measured the same disabled flag after removing the frame
on the old build. GPUI now enables the native setting in its shared popup
constructor and asserts it in the hidden probe. The arm64 and translated x86_64
probes produced identical results and passed the frame, click-through, scroll
window exclusion-ID and selector lease checks. The arm64 suite passed 1,151
GPUI tests with 23 marked ignored. Physical mouse or trackpad movement between
the floating controls remains open.

### 2026-11-18 macOS recording toolbar desktop coordinates

GPUI's native macOS window bounds are relative to the window's display. The
recording toolbar compared them with the desktop bounds it calculates for a
selection when deciding whether to reuse or resize its window. Those origins
differ on a display offset from the primary display. The hidden popup probe
also reproduced the fallback when a window has no assigned display. AppKit
retained a 168×52-point frame at desktop position (−384, −144), while GPUI's
old bounds getter reported its default origin and size. The raw AppKit origin
was (−384, 1209) with a 1117-point primary display. A separate AppKit panel
confirmed the conversion using its native frame's top edge.

The macOS comparisons now read the actual AppKit frame using the same primary
screen conversion as the setter. The hidden probe round-tripped positive and
negative desktop positions on arm64 and under Rosetta. Both recording-control
Metal fixtures passed at 1× dark and 2× light, and all 36 PNGs matched the
preceding captures byte for byte. The arm64 suite passed 1,151 tests, including
the simulated cross-display selection cases. Actual placement and reattachment
on mixed-DPI physical displays remain open.

### 2026-11-18 macOS capture preview display changes

GPUI recreated the capture preview window when the corner or display changed.
Pending operations still held the previous window or view, including animated
dismissal and video export completion. A simulated display disconnect reproduced
a stale dismissing card. The macOS preview now moves the existing window using
its native frame setter. Moving the stack closes its display menu and clears the
old hover and tooltip state. Selecting a display defers the move until the menu
callback releases the window.

The regression disconnects the preferred display while a card is dismissing
and another has a pending action. It verifies the same window falls back to the
primary display, the dismissal and pending completion finish, the capture file
remains, and the saved display preference is retained. The arm64 suite passed
1,152 tests, including the display menu and fallback cases. The 720×520 light
Metal profile at 2× passed and matched all 169 reference PNGs byte for byte,
including 26 capture preview states. The hidden native popup probe also passed.

Both GPUI and Electron currently use fixed card dimensions and margins. With
four previews and a 600-point working height, their position calculations can
put 20 points of a card outside the working area. The macOS GPUI follow-up
below addresses the viewport. Electron placement and physical mixed-DPI
transitions remain open.

### 2026-11-18 macOS capture previews on short work areas

The GPUI window now fits inside the selected work area. It reduces the margin
when the normal stack would extend past an edge and scrolls when the viewport
is smaller than the stack. The cards keep their 200×140-point size. The scroll
viewport starts at the chosen corner and maintains that edge during a resize
while the user is still at the edge. A user who scrolls away keeps their
position within the available range.

The geometry regression failed on the old build with a 610-point work area.
It now passes for all four corners at offset and negative desktop coordinates.
The interaction regression uses a short viewport, resizes it while anchored,
scrolls to the initially clipped card and closes it. The capture file remains.
All 1,154 arm64 GPUI tests passed with 23 marked ignored.

The 480-point Metal viewport passed at 1× dark and 2× light. After scrolling,
the entire revealed card matched the full-height reference pixel for pixel.
Across both profiles, all 363 existing PNGs matched byte for byte. Physical
short displays and Electron placement remain open. The follow-up below
checks the auto-dismiss delay for cards outside an already-scrolling viewport.

### 2026-11-18 macOS recording outline tracking updates

The window outline sampled its target at 30 Hz and reset its AppKit frame and
front order even when the target was stationary. The native fixture measured
those calls directly and failed on the previous build. The outline now updates
its frame when geometry changes and brings itself forward when it is revealed
or restored. A temporarily hidden target retains the same capture-exclusion ID.

The isolated generated-window runs passed on arm64 and under Rosetta with
identical output. They checked stationary updates, movement, resize, hiding,
restoration at the same and at a new position, target closure, and explicit
cleanup. The full fixture and production Swift daemon also type-checked. The
runs used window-server metadata with a generated target. Live captured pixels
and physical multi-display movement remain outside this check.

### 2026-11-18 macOS camera preview work-area placement

Both GPUI and Electron forward camera preview positions to the Swift daemon.
A requested position close to the bottom-right edge placed the native panel
outside the work area on the old build. Showing and updating the panel now
share a display-aware calculation. It uses the work area of the display
containing the requested point, or the current or main display when the
requested display is unavailable.

The native generated-panel check passed on arm64 and under Rosetta with
identical output. It verified the fitted frame, the retained capture-exclusion
ID through repositioning, and an absent or transparent panel on hide. Geometry
fixtures covered dock insets, a display above and left of the
primary, and a saved position for a disconnected display. The Swift daemon and
full fixture type-checked, and all six module-parity tests passed. The test used
a generated stand-in for the camera feed. Live camera access and physical
mixed-DPI placement remain open.

### 2026-11-18 macOS camera preview movement observers

The camera preview registered a window move observer without keeping its
removal token. A notification from a retired panel could read the replacement
panel's frame and emit a duplicate position update. The generated-panel fixture
reproduced two reports of the replacement coordinates before the fix.

The daemon now owns one move observer per visible panel and removes it during
hide and cleanup. A queued callback verifies the identity of its panel before
emitting. Revealing an existing panel also ensures tracking is installed. The
arm64 and Rosetta fixtures verified one position report for each current panel,
including after repeated observer setup and a retired-panel notification. The
production daemon and full fixture type-checked, and all six module-parity tests
passed. This run used generated AppKit panels. Live camera capture and physical
dragging remain open.

### 2026-11-18 macOS camera preview startup

The first camera preview show could start its source twice. The view started
capture at construction and the show handler started it again when the request
changed device settings. The generated fixture recorded two startup calls for
the same device name on the prior build.

A newly created preview now starts its source only after the capture-exclusion
and screenshot checks let it reveal. A reused panel compares its applied source
with the latest request. With a simulated screenshot barrier held, the fixture
saw no startup calls for the hidden panel. After release the latest request
succeeded, both superseded requests returned false, and one source started.
An identical repeat show also kept the startup count at one.

The generated fixture passed on arm64 and under Rosetta. The Swift daemon and
full native fixture type-checked. All 21 camera-preview IPC and daemon-parity
tests passed. The native fixture records startup requests without opening a
camera. Device output, first-frame latency and live switching remain open.

### 2026-11-18 macOS camera preview capture-filter failure and retry

A generated-panel fixture held and then failed the capture-exclusion filter
refresh while two camera preview shows were pending. The staged panel was
transparent, ignored input, and was registered for exclusion. The superseded
show returned invisible and the current show returned `CAPTURE_ERROR`. The
failed panel stayed hidden, had no startup calls, and left an empty exclusion
registry. A retry created a different panel and queued one startup.

The camera-only fixture passed on arm64 and under Rosetta. The production Swift
daemon and full fixture type-checked, and all 21 camera-preview IPC and daemon
parity tests passed. The filter error and camera startup were generated by the
fixture. Live camera capture and actual ScreenCaptureKit filter failure remain
open.

### 2026-11-18 GPUI playback cadence after late wakes

The playback loop waited a full frame interval after processing each wake.
A simulated 9 ms delay at 30 fps left the playhead at 42.333333 ms when the
66.666666 ms frame boundary arrived. The next delay now uses the remainder to
the next boundary measured from the original playback start. The playhead uses
the elapsed time, so the loop can skip missed ticks after a long stall.

The test exercises the actual playback loop with a controlled scheduler clock.
It verified recovery after one late wake and after a multi-frame stall. All
1,155 arm64 GPUI tests passed with 23 marked ignored. The offscreen Metal
profiles at 720×520 light and 2×, and 950×700 dark and 1× passed, with all
367 PNGs matching their references byte for byte. This verifies the scheduler
and static rendering. Interactive 4K presentation and frame pacing on a
physical display remain open.

### 2026-11-18 GPUI preview frames after discrete changes

With an earlier frame in flight, the editor used to publish that frame after a
seek and then start work on the requested position. The generated-movie
regression reproduced the old frame appearing first. Discrete playhead and
transport changes now invalidate older frame requests. Commits outside an edit
gesture also defer the next frame until the composition has applied the state.

The macOS tests verified that a seek discards the earlier frame, that scrubbing
still publishes intermediate feedback, and that the first frame after a
wallpaper commit has the edited square dimensions. Continuous playback, hover
and drag updates can show intermediate frames while the latest one is queued.
The arm64 suite passed 1,157 tests with 23 marked ignored. The light 720×520
Metal profile at 2× and the dark 950×700 profile at 1× both passed. All 367
PNGs matched the reference captures byte for byte. Interactive 4K latency on a
physical display remains open.

### 2026-11-18 GPUI playback from the hovered timeline frame

Electron starts playback from the hover position and clears the hover preview.
A GPUI test moved the pointer over the two-second position and pressed Space
without moving it away. On the old build, the playback clock started from
0.5 seconds while the hover override continued to select the two-second frame.

GPUI now adopts the hover position and clears the override before starting
its playback clock and audio transport. The regression verified playback from
two seconds, advancement with the pointer stationary, and playback from zero
when starting at the exact end. The arm64 suite passed 1,158 tests with 23
marked ignored. Both offscreen Metal profiles passed, with all 367 PNGs
matching their references byte for byte. If the hovered frame is still being
composed when playback starts, the displayed image can wait for the first
playback tick and compose. Physical 4K first-frame latency remains open.

### 2026-11-18 macOS scroll capture stop and cursor prompt

When auto-scroll stopped at an ambiguous seam while the cursor was outside the
selection, the orange outline and “Move cursor here to continue” prompt stayed
visible. The Swift daemon now restores the blue outline and sends the cursor
state change when auto-scroll stops. GPUI shows the prompt only during active
auto-scroll and hides it when finishing the capture.

The focused Swift fixture reproduced the stale state before the fix. On arm64
and x86_64 under Rosetta, the fixed fixture cleared the cursor state, restored
the hidden AppKit boundary's color and saved the last verified pixels. The arm64
GPUI suite passed 1,158 tests with 23 ignored, and 62 Electron scroll capture and
daemon contract tests passed. The dark offscreen Metal check at width 1280 and
1× passed selection, save, history, editor handoff and retry.

Live cursor interaction remains open. Repeated scroll histories can still
produce identical frames and leave the seam ambiguous.

### 2026-11-18 macOS scroll capture placement at the menu bar and Dock

The native popup probe reproduced a scroll preview starting at y=0 on the
1728×1117-point Retina display, where AppKit's work area started at y=33.
Electron's scroll control placement also uses the full display bounds. GPUI now
places the scroll controls and preview inside the selected display's work area.
The resume prompt remains centered over the selected capture region. When space
allows, the controls still go below the selection and the preview to its right.

An inset-display fixture covers a negative desktop origin, a full-display
selection, and a narrow selection in the area reserved for a side Dock. It
checks the two popup origins and preserves the prompt's selection bounds.
The hidden AppKit probe verified that both popups fit the actual work area
and kept their capture exclusion IDs. The macOS GPUI suite passed 1,159 tests
with 23 ignored, and 14 Electron reference tests passed. Light and dark
offscreen Metal workflows passed at 1× and width 1280, with all 15 workflow
PNGs matching the preceding runs byte for byte. Live placement on a second
display and next to a visible Dock remains open.

### 2026-11-18 macOS screenshot countdown placement

For a selection at the top of the primary display, the old GPUI countdown
started at y=20. A hidden AppKit timer panel exceeded the work area's top
edge by 13 points. Electron also requests y=20 in this case. GPUI now uses
the selected display's work area for the countdown while retaining its
20-point top margin and its preferred position above the selection.

The inset-display regression covers both sides of a negative-origin display
and a selection near its bottom edge. The hidden GPUI popup probe returned
(0, 53) and (1588, 53) at the top corners of the 1728×1117-point display.
Hidden AppKit panels at those exact coordinates fit inside the work area on
arm64 and x86_64 under Rosetta. The macOS GPUI suite passed 1,160 tests with
23 ignored, and 21 Electron timer reference tests passed.
The light and dark Metal workflows verified the new RPC placement and
completion, cancellation, timeout, failure and retry. All six workflow
PNGs matched the preceding runs byte for byte. Placement next to a
visible Dock and on a second physical display remains unverified.

### 2026-11-18 macOS screenshot countdown timing

The native screenshot countdown previously subtracted one second for every
AppKit timer callback. A two-second countdown stayed open after a 2.2-second
main-loop stall. Two queued show requests could also display the second request's
duration in the first panel. GPUI and Electron both use this native timer.

Each show now supplies its own duration on the main queue. The AppKit timer
reads the remaining time from a monotonic deadline, runs in common run-loop
modes and dismisses an overdue panel before emitting completion. With real
AppKit timers and hidden panels, both cases failed before the fix and passed
after it on arm64 and on x86_64 under Rosetta. The macOS CI fixture now covers
both cases alongside the existing window lifecycle checks. The full production
Swift daemon typechecked on arm64, and 27 Electron timer and daemon contract
tests passed.

### 2026-11-18 macOS screenshot countdown accessibility

The native screenshot countdown is a custom AppKit view shared by GPUI and
Electron. The hidden AppKit fixture found that the view did not expose an
accessible cancel button. It now exposes a single button named “Cancel
screenshot countdown” with the current time remaining. Its press action
uses the same callback as a click. Dismissing the countdown clears that
callback so a retained accessibility element cannot cancel a later capture.

The regression checks the button role and label, singular and plural time
values, cancellation and a second press on the retired view. It runs in
the macOS timer CI fixture. The hidden accessibility and timing cases
passed on arm64 and x86_64 under Rosetta. The production daemon typechecked,
and 27 Electron timer and daemon contract tests passed. VoiceOver
announcement and navigation on a visible countdown remain unverified.

### 2026-11-18 macOS tooltip placement beside tall preview stacks

When a protected tooltip could fit neither below nor above its owner, the
pointer-relative fallback could cover a tall capture preview stack despite
available space beside it. The native tooltip now checks both sides and
uses a four-point gap when a side fits within the display's visible frame.
The bubble stays vertically aligned near the hovered control.

A hidden AppKit regression reproduced the overlap before the change.
The light single-line and dark long-label panels fit on opposite sides
after the fix on arm64 and x86_64 under Rosetta. The fixture checks the
native frame and gap and runs as part of the scroll preview fixture.
Live hover on a tall preview stack and across displays remains unverified.

### 2026-11-18 macOS auto-dismiss for clipped capture previews

A generated four-card stack with a 300-point viewport reproduced a newly
clipped preview starting to dismiss before the user scrolled to it. The
macOS timer now checks the card's position in the scroll viewport. A fully
clipped card waits until it is visible and then gets the full delay. The
regression scrolls the card into view, verifies it survives the initial
delay, and confirms the later dismissal preserves the capture file.

The GPUI test backend changes window bounds silently on resize. Updating
the test viewport after resizing made the existing normal-stack and
hidden-window reveal tests exercise the same drawable area as production.
The arm64 GPUI suite passed 1,161 tests with 23 ignored. All 52 Electron
capture preview tests passed. Light and dark Metal profiles matched all
367 baseline PNGs byte for byte, including 56 capture preview images.
Partially visible cards still count as visible. The delay restarts after
the timer observes complete clipping. Physical short displays remain
unverified.

### 2026-11-18 native macOS attachment cancellation

An isolated GPUI development app opened a generated PNG, started a right
attachment using the normal editor action, and verified that AppKit hid
the editor while the selector was visible. The probe delivered one Escape
event to the selector's native AppKit view. The selector window and pending
attachment both cleared, the same editor became visible, and its exported
composition matched the pre-selection PNG byte for byte. The source file
remained on disk.

The light and dark runs passed on arm64, and dark also passed on x86_64 under
Rosetta. The focused GPUI Escape test and 38 Electron overlay tests passed.
The native runs used Freeze Screen off and disabled the capture daemon.
The event was delivered in process. The follow-up below checks native freeze
release during cancellation. Reproduce with the parity development binary
using `--native-ui-probe --attachment-cancel`
and `PORATAKE_DAEMON_PATH=/usr/bin/false`.

### 2026-11-18 native macOS attachment cancellation with Freeze Screen

The same attachment probe also passed in light and dark with Freeze Screen
enabled on the 1728×1117-point Retina display. The authorized screen-only
daemon froze a generated backdrop. A temporary bridge observed one successful
freeze and one successful release in each run. WindowServer showed a native
frozen window after the freeze and confirmed it was gone before daemon
shutdown. The selector and attachment state cleared, and the restored
editor exported the same PNG byte for byte. The bridge forwarded no
screenshot, audio or camera requests. All 30 focused GPUI overlay tests
passed, and the isolated Freeze Screen off probe passed again.

The [run record](tests/native/evidence/macos-ui/native-attachment-freeze-cancel-20261118.txt)
covers Apple Silicon and an AppKit Escape event delivered to the isolated
selector. Physical Escape delivery, translated execution with Freeze Screen
enabled, and additional displays remain unverified. The development probe
enables this path with `PORATAKE_NATIVE_UI_ATTACHMENT_FREEZE=1` and an authorized
`PORATAKE_DAEMON_PATH`.

### 2026-11-18 native macOS scroll windows over the selected region

The development app painted the actual GPUI stitch preview, resume prompt
and toolbar over a generated backdrop. All three native windows intersected
the selected region. The authorized Swift daemon captured that region with
its production ScreenCaptureKit exclusion helper, using the three IDs from
the GPUI scroll session. Including each window individually changed pixels
in a separate part of the captured area.

With all three excluded, light mode matched the bare backdrop across all
7,492,608 pixels. Dark mode matched exactly across the three painted
regions. Outside those regions, 2,362 pixels differed by at most one
8-bit level per channel. All 15 focused GPUI scroll tests passed.
The [native run record](tests/native/evidence/macos-ui/native-scroll-overlap-exclusion-20261118.txt)
reports individual positive controls and verification scope. This check
uses the shared screenshot filter on a single display and leaves live
auto-scroll with deliberate panel overlap and mixed-DPI displays open.

### 2026-11-18 native macOS strict scroll capture and saved pixels

The opt-in Swift fixture ran the production scroll start, first-frame
capture and finish handlers with the three real GPUI window IDs. It opened
the native selection boundary and used the strict ScreenCaptureKit exclusion
path. The fixture intercepted scheduling to stop after one frame and
disabled pointer input. A Screen Recording preflight confirmed existing
authorization before each capture.

The saved 3456×2168 PNG matched the bare backdrop exactly in dark mode.
In light mode the preview, prompt, toolbar and boundary regions matched
exactly. The 2,990 other differences were at most one RGB channel level.
The default native scroll fixture also passed. The
[native run record](tests/native/evidence/macos-ui/native-scroll-strict-frame-20261118.txt)
details the capture and comparison. Scrolling through multiple live frames
with deliberately overlapping panels remains open.

### 2026-11-18 native macOS multi-frame scroll capture with overlapping controls

An AppKit fixture moved a generated page by 758 points between captures.
The actual GPUI preview, resume prompt and toolbar stayed visible inside
the selected 600×1084-point region. The test placed the preview over the
page explicitly and used the left side of the display after a full-width
attempt encountered unrelated pixels in the overlap. AppKit advanced only
the generated page. The fixture called the production post-scroll capture
and finish paths without posting pointer events.

ScreenCaptureKit supplied three 1200×2168 frames. Both seams matched the
expected 652-pixel overlap and the daemon saved a 1200×5200 PNG. The
independent reference used three unobstructed captures converted from
Display P3 to sRGB with ColorSync. In each theme, 6,239,946 of 6,240,000
pixels matched exactly. The other 54 differed by one RGB level in two
rows away from the seams. The default Swift fixture passed.

The [native run record](tests/native/evidence/macos-ui/native-scroll-multiframe-overlap-20261118.txt)
describes the test. Pointer-driven movement with deliberately overlapping
controls, ambiguous repeated content and physical mixed-DPI displays
remain open.

### 2026-11-18 macOS scroll capture wide-gamut color

The old multi-frame stitch and its resized preview used a DeviceRGB bitmap
context and encoded Display P3 input as sRGB. Converting an old stitched
PNG back to Display P3 with ColorSync changed 121,280 of 130,560 pixels in
the generated gamut fixture. One source sample (231, 0, 249) came back as
(232, 51, 247). Scroll capture now uses the first source frame's RGB
color space for the stitch and keeps that profile when resizing its preview.
Frame alignment uses the same matching path as before.

Generated Display P3 and sRGB fixtures passed on arm64 and x86_64 under
Rosetta. Both checked the profiles of the stitched and scaled preview
PNGs and exact stitched pixels in their source color spaces. The default
native Swift fixture passed. Real three-frame ScreenCaptureKit runs with
overlapping controls also retained Display P3 in light and dark. Direct
RGB comparisons against the corresponding Display P3 references matched
6,239,946 of 6,240,000 pixels. The remaining 54 differed by one channel
level away from the seams. Four GPUI image color tests and 12 Electron
preview and daemon contract tests passed. Mixed physical displays remain
unverified.

### 2026-11-18 macOS scroll-resume prompt on thin selections

The GPUI layout fixture measured a 284×38-point resume prompt inside a
24×11-point selection window, clipping the message. Electron renders its
prompt inside a display-sized overlay. GPUI now expands its prompt window
only when the selection needs more room, clamping the expanded dimensions
to the display's work area. On a narrow viewport the text wraps. The
toolbar clears the expanded prompt when the two would intersect. The
stitch preview's anchor still follows the selected area.

The 200-point viewport regression verifies the full prompt, wrapped label
and clearance from the toolbar. The hidden AppKit probe rendered the
prompt for a thin selection at the lower-left edge and verified its
painted pixels fit the window. The prompt stayed click-through and kept its
capture exclusion ID. The arm64 suite passed 1,162 tests with 23 ignored,
and 14 Electron reference tests passed. All 15 light and dark Metal
workflow images matched their baselines. The expanded prompt can cover
part of the stitch preview at a thin edge selection while the cursor is
outside. Physical interaction on a narrow or secondary display remains
unverified.

### 2026-11-18 macOS tooltips after the pointer leaves their owner

The protected tooltip previously held the pointer position from the start of
its request. If the pointer left while the daemon waited for capture startup,
the old panel could still appear. With known owner bounds, the daemon now
checks the current pointer before staging the panel and again before revealing
it. A panel already registered for exclusion is retired if the pointer leaves
before reveal.

The AppKit fixture advanced a synthetic pointer outside the owner while
holding recording startup and then while holding an in-flight screenshot.
In both cases the fixed tooltip stayed hidden and its exclusion entry was
removed. The focused and full native fixtures passed on arm64 and x86_64
under Rosetta. The Swift daemon typechecked, and eight Electron preview and
daemon contract tests passed. Physical hover transitions between controls
remain unverified.

### 2026-11-18 macOS camera preview error labels

The native camera preview uses a black background. A generated AppKit
fixture reproduced low-contrast error text in Aqua. The old text field
filled the panel and successive errors added more fields. The error now
uses one centered, light label in both appearances. A new attempt clears
the label, and a successful capture removes any label still present.
Failure callbacks check that their source still matches the current request.

The generated error and full native recording outline fixtures passed
on arm64 and x86_64 under Rosetta. They verified the rendered text,
placement, reuse and retry cleanup. The Swift daemon typechecked and all
21 camera preview and daemon contract tests passed. The authorization
check returned not determined. Live camera output and recovery remain
unverified.

### 2026-11-18 macOS camera device identity

The macOS preview and recorder previously matched camera names before
checking the selected device ID. A generated input with two cameras named
USB Camera reproduced selection of the first device when the second
device's native ID was requested. Both paths now prefer a discovered
native ID. An unmatched saved ID still falls back to an exact or partial
name match and then the default camera.

The generated fixture passed on arm64 and x86_64 under Rosetta. It checked
duplicate names, a stale name paired with a working ID, and saved browser
IDs that need name fallback. Live multi-camera hardware remains unverified.

### 2026-11-18 macOS selector and capture handoff latency

The native timing probe ran six cycles per profile over a generated backdrop
on the Apple M5 Pro Retina display. It checked when WindowServer saw selectors,
previews and editors appear or disappear. The first action in each process is
reported separately from the five warm samples. The probe drove the app
directly and used the previously authorized screen-only daemon.

GPUI now disables AppKit's automatic selector animation and hides the selector
before deferred teardown. Warm median cancel-to-hidden fell from 164–168 ms
to 12–15 ms across small live, large live and large frozen captures. Warm
area-to-window switching fell from 167–183 ms to 26–79 ms. Confirm-to-hidden
fell from 164–171 ms to 15–17 ms. The isolated dark repeat measured 20 ms for
a warm live area selector and 44 ms for a warm frozen selector.

One large frozen run still reached 884 ms from confirmation to a ready
thumbnail with the selector gone. In the dark repeat the warm median was
94 ms and the maximum was 111 ms. The probe records window visibility and
thumbnail readiness. Physical input latency and time to the first composited
editor frame remain unverified.

### 2026-11-18 macOS preview-to-editor opening latency

The editor previously decoded each screenshot once for window sizing and
again when constructing the editor. It now uses the existing color-managed
decoder once and passes the decoded image into the editor. The open error
toast and history-state restoration are preserved.

For a 3000×1800 screenshot in dark mode, the warm median from preview
double-click to editor visibility fell from 62.6 to 54.1 ms with live capture
and from 48.0 to 41.5 ms with Freeze Screen. The first editor openings fell
from 92.9 to 71.2 ms live and from 81.4 to 64.8 ms frozen. A 600×440 live
screenshot opened in a warm median of 26.5 ms on the final build. The full
arm64 GPUI suite passed 1,162 tests with 23 ignored.
The light 2× Metal workflow and native attachment cancellation also passed.

The [native timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the final selector, preview and editor numbers and the observed
tails. It measures one Apple Silicon system with generated images.
Performance across hardware, mixed displays and physical input remains open.

### 2026-11-18 macOS screenshot editor handoff during image decode

The preview's Edit and double-click actions previously held the UI thread
while decoding the screenshot. They now share one guarded handoff that
starts dismissal and decodes on the background executor. Capture directly
to the editor uses the same background path. The decoded image still goes
through the same color-managed editor constructor, and an error still
shows the existing toast.

In six native dark-mode cycles per mode on the Apple M5 Pro, the warm
median acknowledgment of the preview editor command fell from 25.3 to
0.1 ms for a 3000×1800 live screenshot and from 19.2 to 0.1 ms for a frozen
screenshot. Native window visibility moved from 40.4 to 44.9 ms live and
from 33.0 to 41.8 ms frozen in these runs. This moved work off the UI
thread while retaining the cost of preparing the editor.

The Metal workflow sends repeated edit requests in one UI turn and checks
that no editor opens inline, exactly one opens after decoding, and its
export matches the selected screenshot. It passed in light at 2× and
dark at 1×. The arm64 suite passed 1,162 tests with 23 ignored. Physical
input and final compositor paint timing remain open.

### 2026-11-18 macOS screenshot editor bitmap preparation

The background editor worker now prepares the color-managed base image,
the GPUI display bitmap and any required tiles before handing the result
to the UI thread. Synchronous editor entry points reuse the same image
preparation. The editor's pixel format, tile geometry and history loading
remain consistent with the previous path.

An initial unpaired timing run was inconclusive, so the native probe
alternated old and new builds in opposite order. It recorded eight warm
samples per build for each case on the Retina Apple M5 Pro. During the
editor handoff, a lightweight app command measured the longest UI response
in each trial. The median of those longest responses fell from 25.8 to
16.7 ms for live 3400×2060 captures and from 21.0 to 16.3 ms for a generated
17000×400 tiled PNG. The median time to a visible editor stayed at 61 ms
for the live case and moved from 56.9 to 59.3 ms for the tiled case. The
highest live probe response was 39.2 ms on the new build versus 35.7 ms
on the old build. Physical event and final compositor timing remain open.

All 1,162 arm64 GPUI tests passed with 23 ignored. The light Metal
preview-to-editor workflow and the 2× Metal 17000-pixel texture tests
passed, including panning across horizontal and vertical tile seams and
reopening the saved images.

### 2026-11-18 macOS scroll preview decoding latency

GPUI decoded, color-converted, cropped and resized scroll preview PNGs
inside its UI progress handler. The scroll session now uses one background
decoder and keeps only the newest waiting frame. An invalid payload leaves
the last displayed image intact, and cancellation discards stale results.
The decoder uses the same image and color path as before.

The focused regression sends a burst before the worker starts, verifies
that the newest frame renders, and verifies that cancelling pending work
preserves the already displayed image. The generated 480×4096 preview took
2.54 ms to decode and resize in the test. Dispatching it took 0.07 ms.
The full arm64 GPUI suite passed 1,163 tests with 23 ignored, and 14
Electron preview tests passed. The native hidden-window probe and both
light and dark Metal scroll workflows passed. Physical scroll motion
and compositor timing remain open.

### 2026-11-18 macOS recording overlay dismissal latency

The daemon previously waited for ScreenCaptureKit's capture-filter refresh
before returning the hide reply, even after the recording overlay was
transparent, ordered out and unregistered. GPUI waited for that reply.
The daemon now acknowledges the hide once the window cleanup is complete.
The remaining filter refresh still runs through the same registry.

The native fixture held the filter refresh after ordering out the overlay.
The old build failed the reply check. The fixed build replied with
`visible: false` while the refresh was still held and showed no remaining
registered overlay. The focused and full fixtures passed on arm64 and
x86_64 under Rosetta. The production Swift daemon typechecked, and the
light and dark recording toolbar Metal workflows and 82 Electron and
daemon contract tests passed. The held refresh is simulated. Live filter
and compositor timings remain open.

### 2026-11-18 macOS camera preview dismissal and replacement

Camera preview dismissal also waited for the capture-filter refresh after
the panel was already hidden and unregistered. The native fixture reproduced
the delayed reply. The daemon now acknowledges the hide once panel cleanup
is complete, then refreshes the filter asynchronously. The queued camera
stop retains its view until cleanup runs.

With the old filter refresh held, the fixed fixture received the hide
reply and staged a replacement panel. That panel stayed transparent and
deferred camera startup until the filter contained its new window ID.
Only the replacement source started. The focused and full generated
fixtures passed on arm64 and x86_64 under Rosetta. The production daemon
typechecked and 79 Electron and daemon contract tests passed. The fixture
used generated camera sources. Live camera switching remains unverified.

### 2026-11-18 macOS recording toolbar cancellation latency

The native probe observed the cancelled recording toolbar lingering for a
warm median of 161.5 ms in light and 164.1 ms in dark. The toolbar now shares
the selector's AppKit transition policy and orders out before deferred
GPUI removal. The warm cancel-to-hidden medians fell to 11.7 and 13.3 ms.

The updated bar reopened in 16.5–18.6 ms in the warm runs. Microphone menu
expansion stayed near 17 ms. Collapse measured 26.1–27.5 ms after the
change, compared with 12.9–21.1 ms before. The probe measures native window
geometry with generated device data. The
[timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the sample counts and tails.

All 1,163 arm64 GPUI tests passed with 23 ignored. The hidden AppKit
probe passed. All 36 light and dark Metal toolbar images matched the
baseline byte for byte. Physical clicks and time to the first composited
menu pixels remain unverified.

### 2026-11-18 macOS recording toolbar menu resize latency

Toolbar bounds changes previously waited for GPUI's next frame callback.
The toolbar now schedules the native resize on the foreground executor
after the current input callback and applies the latest pending bounds.
The existing window observer still opens the device menu after the
expanded bounds arrive. Escape behavior during menu dismissal is unchanged.

Paired native runs measured six warm cycles per build and theme using
generated microphone data. The warm median to the collapsed frame fell
from 30.6 to 11.5 ms in light and from 30.7 to 9.1 ms in dark. Expansion
moved from 17.0 to 16.4 ms in light and from 16.4 to 14.1 ms in dark.
The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes warm maxima and the control window timings. These are
WindowServer bounds measurements. Physical input and composited pixels
were outside the latency probe. All 1,163 GPUI tests passed with 23
ignored, and all 36 Metal toolbar PNGs matched their baselines.

### 2026-11-18 macOS recording toolbar target switching

When the target label changed the toolbar's centered position, GPUI could
tear down the pre-recording window and open a replacement. The macOS
toolbar now reuses its existing native window for target changes in the
same preference scope. It measures the new label width and keeps the
current input choices. A changed target clears its old menu and tooltip
state and invalidates the previous countdown.

In the native probe, all 28 switches between an area and a generated
window target changed window IDs before the fix. All 28 reused the same
ID afterward. Warm median switching fell from 26–33 ms to 8–11 ms.
The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
has the light and dark results. The GPUI countdown regression kept the
microphone setting and verified that the old countdown could no longer
start a recording. All 1,164 GPUI tests passed with 23 ignored, and all
36 light and dark Metal toolbar images matched their baselines. Live
switching across physical displays remains open.

### 2026-11-18 macOS recording countdown after a stalled UI

The GPUI recording countdown previously subtracted one second for each
timer wake. When a native fixture held the toolbar's foreground thread for
2.25 seconds, the old five-second countdown still displayed five after
resuming. The countdown now computes the remaining seconds from its
monotonic deadline and schedules updates on the number boundaries.
Existing cancellation and target-generation checks still apply.

Both light and dark native runs displayed three seconds after the stall.
With a warmed stub daemon, the old dark run attempted recording setup
after 7.49 seconds. The fixed light and dark runs attempted setup after
5.05 seconds. The stub declined the overlay request before an actual
recording could start. The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the measurements. All 1,165 GPUI tests and 76 Electron reference
tests passed, and all 36 light and dark Metal toolbar images matched the
baseline.

### 2026-11-18 macOS live recording startup, preview and editor latency

The current GPUI development app recorded a 300 × 220 point area on the
built-in Retina display with the previously authorized screen-only Swift
daemon. With a generated changing backdrop, the no-countdown Start command
returned in 21–35 ms and ScreenCaptureKit startup was confirmed in
333–410 ms. After the same 2.25-second foreground stall, the real recorder
received Start at 5.080–5.087 seconds and confirmed it at 5.455–5.496 seconds.
The four clips decoded as 600 × 440 video-only recordings with 9–10 frames
and the expected backdrop on the first decoded frame.

Stop to visible recording preview took 59–66 ms. Opening the video editor
from the preview took 81–84 ms to a visible window. These timings end at
WindowServer visibility. Editor frame readiness and physical input were
outside this probe. The editor's background wallpaper lookup was stubbed.

Four successful runs against a static backdrop had recorder startup
responses in 445–561 ms. One additional static run took 4.19 seconds at
the daemon and another returned an asset-writer error on Stop after a short
recording. The exact source of the startup tail and the failure on the
production daemon remain unverified. The
[timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes both conditions and the measurement details.

### 2026-11-18 macOS first frame during recording startup

ScreenCaptureKit could deliver the only useful frame of a static scene
before stream startup and the exclusion refresh had returned. The screen
recorder discarded frames in that interval. It now accepts frames as
soon as the stream with its initial exclusion filter is ready to start.
The recorder still reports startup completion after the refresh. New
protected panels continue to wait for their filter update before showing.

The generated-frame regression failed with the previous sample gate.
The fixed recorder decoded the first frame and held the static image to
the requested stop position, including under simulated writer backpressure.
The default timeline suite and all seven static timeline variants passed.

In four paired native light and dark trials with the same static scene,
the previous screen-only daemon wrote 0.438–0.725 seconds of video.
With the startup change, it wrote 0.835–0.923 seconds. Stop was requested
about 0.8 seconds after startup confirmation. All eight recordings
decoded, and the 4.19-second startup outlier did not recur. The production
Swift daemon typechecked. Live production audio and the origin of the
earlier startup outlier remain open. The
[timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the per-run durations.

### 2026-11-18 macOS recording startup filter reuse

Area and display recording startup now reuses the initial exclusion filter
when it covers the current protected windows. With the previous logic the
daemon always enumerated ScreenCaptureKit content again and replaced the
filter before confirming startup. Registration changes during startup
still require a refresh. Staged panels still wait for that refresh before
appearing.

Across six paired, fresh-process runs with the authorized screen-only
daemon and a static generated backdrop, median Start-to-confirmation
fell from 370 to 285 ms. The daemon's share fell from 333 to 259 ms.
All twelve 600 × 440 video-only recordings decoded. The earlier
multi-second startup outlier did not recur. The
[timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the individual results.

The startup regression detected the redundant refresh with the old
registry behavior. The fixed fixture skipped the unchanged refresh and
waited for the updated filter after the window list changed. The native
outline and scroll coordination tests passed. The authorized opt-in
ScreenCaptureKit test verified the protected window's exclusion against
a positive control. Its test-owned windows now close without releasing
the Swift-owned reference. The production Swift daemon typechecked.
Live production audio and mixed-display paths remain open.

### 2026-11-18 macOS first recording preview and editor frame

The GPUI recording preview waited for its 125 ms playback tick before
requesting the first thumbnail. It now decodes immediately after attaching
the decoder and uses the same interval for subsequent frames. The native
regression verifies that the first image is ready before advancing the
test clock and that playback updates it on the next tick.

Across three paired screen-only runs per size, median Stop to thumbnail
ready fell from 225 to 79 ms for a 600 × 440 recording in light mode and
from 283 to 153 ms for a 3000 × 1800 recording in dark mode. The video
editor's first composed frame was ready after 72 ms and 128 ms,
respectively. GPUI image readiness and WindowServer window visibility
were measured separately. In a separate run per size without the
backdrop over the editor, native window captures verified the actual
recording pixels in both the thumbnail and the editor. The
[timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the scope and timing endpoints.

### 2026-11-18 macOS recording preview texture retention

The looping preview generated a new GPUI image for each frame. Replacing
the image dropped its CPU reference and left its texture in the atlas.
The preview now releases displaced sharp and blurred textures through
GPUI. Dismissing a card releases its textures even if another card
keeps the preview window open.

The regression verified that the first and blurred frames were actually
painted into the atlas. The old build failed when the test checked
whether the first frame had been removed. The fixed build removed the
old frame after the next tick and the last blurred frame after dismissal.
All 1,169 runnable GPUI tests passed.

A matched light-mode native run used a looping 600 × 440 screen-only
recording. From the second to the forty-second second, the temporary app's
physical footprint grew from 145 to 203 MiB in the previous build and
from 146 to 155 MiB with the fix. This compares the whole process.
The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the sampled values.

### 2026-11-18 live macOS recorded-window movement and closure

GPUI recorded a disposable AppKit window through the authorized screen-only
daemon in light and dark mode. The target changed color as it moved and
resized. The same outline followed it, disappeared while it was hidden,
and returned at the same bounds when it was shown again. WindowServer
and ScreenCaptureKit still listed the hidden target. Recording continued.

Closing and releasing the target window while its app remained running
produced one `TARGET_CLOSED` event in each run. GPUI requested Stop after
285 ms and 291 ms, cleared the toolbar and outline, and produced a
preview that opened in the video editor. Both video-only takes decoded
at the initial 720 × 480 resolution and contained the three expected
color stages. These runs used an app-local window selection fixture and
one physical display.

An initial closure attempt retained the target's AppKit object and tested
a hidden window instead. A visible-window lookup reported it missing,
and a complete WindowServer lookup still contained the target. The
current recorder handled actual closure once the fixture released the
window. No polling change was kept.

Before the follow-up below, changing the target from 360 × 240 points to
220 × 315 points put the decoded blue image in columns 0–335 of the
720-pixel frame. Black padding occupied the right side. The ScreenCaptureKit
frame attachments reported the same origin before encoding. The expected
centered fit leaves padding on both sides. The
[timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes both native sample results.

### 2026-11-18 macOS resized-window recording alignment

The recorder now uses the ScreenCaptureKit frame's content rectangle and
scale to move already-fitted window pixels into the center of the fixed
output. Core Image fills the remaining area with opaque black in a writer
pool buffer. Full-sized and already-centered frames reuse the input buffer.
That centered geometry matches the cursor tracker's existing fit.

Two live screen-only window takes in light and dark kept their 720 × 480
resolution. The portrait target occupied columns 192–527, with 192 pixels
of black on each side. The wide target occupied rows 132–347, with 132
pixels above and below. The target also moved, hid, returned and closed,
and both takes opened in the video editor. The drawing, reopen and export
fixture passed on the original take and on a copy starting at a
post-resize portrait frame. It checks the composed preview against the
decoded export within codec tolerance.

The focused frame test checked exact pixels and opaque padding on arm64
and x86_64 under Rosetta. At 4K, a generated-buffer measurement gave a
0.94 ms warm median and 4.69 ms warm p95, with a 59.60 ms first render.
Live recording throughput at 4K and physical mixed-display transitions
remain open. The [native record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the measurements.

### 2026-11-18 macOS high-resolution window recording throughput

The screen-only native benchmark recorded a continuously changing
3200 × 1800 window with a 60 fps request. It recorded five seconds at the
original size and five seconds after a portrait resize. Four matched
takes alternated centering off and on in the same daemon binary.
The fixture's 60 Hz timer ran at 59.9–60.0 Hz.

The centered build decoded 51.2–57.5 fps in the portrait phase, compared
with 53.0–57.9 fps with centering bypassed. The full-size phase varied
between 51.5 and 57.8 fps in the same runs. Each take's two phases were
within 1.3 fps of one another. The corrected portrait footage remained
centered, and every take kept its original 3200 × 1800 output size.
In a separate pair with frame counters, the writer appended every
delivered image in both runs. The results cover the built-in display.
Sustained 60 fps at UHD, more complex content and mixed displays
remain open. The
[measurement record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the individual results.

### 2026-11-18 macOS scroll duplicate comparison responsiveness

The scroll daemon now computes the frame fingerprint and verifies
matching fingerprints on a background queue. AppKit still owns frame
acceptance and checks the capture generation at delivery. Stop and
restart can proceed while a comparison is running, and a late result
cannot change the verified prefix or schedule another scroll step.

A generated 3400 × 2060 duplicate-frame probe measured a warm median
of 8.28 ms in the AppKit callback before the change and under 0.01 ms
afterward. The total time to the decision increased from 8.28 to
13.61 ms in that probe. The focused test held the worker, stopped and
restarted on the main thread, and verified the saved prefix.

The native light and dark overlapping-control runs saved three frames
and produced pixel-identical output to the previous build at
1200 × 5200. Each stayed within one RGB channel level of the
unobstructed Display P3 reference. The arm64 and Rosetta fixtures
passed, and the production Swift daemon typechecked. Actual scroll
displacement is still unavailable. The matcher and duplicate
criteria are unchanged. The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the endpoints and scope.

### 2026-11-18 macOS scroll preview bitmap lifetime

The scroll preview kept its full stitched bitmap in the published
snapshot after encoding the smaller preview. The snapshot now keeps
the encoded image, dimensions and capture progress. The full bitmap
is released after encoding. Preview composition and the final PNG
still use the existing color and stitch paths.

In two fresh native fixture runs per build, a 3400 × 13740 preview's
steady process footprint fell from 364.3 MiB to 185.5–186.3 MiB.
A focused regression verified that the encoder received the stitched
bitmap and the published preview released it. The light and dark
overlapping-control runs each saved three filtered frames. The
1200 × 5200 Display P3 PNGs stayed within one RGB level of their
unobstructed references. The follow-up below measures peak allocation
during preview composition. The [memory record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the individual samples.

### 2026-11-18 macOS scroll previews render at thumbnail resolution

The daemon now draws the aligned scroll strips directly into the
preview-sized canvas. It uses adjacent integer row bounds at each seam
and publishes the same original dimensions and scroll height. The final
PNG uses the shared strip layout at full resolution.

In two generated runs per build, nine sequential previews ending at
3400 × 13740 took 296–301 ms before the change and 166–167 ms after it.
Peak process footprint went from 391.2 MiB to 212.4–212.5 MiB. The
benchmark includes source-frame generation, composition and PNG encoding.
The full-resolution save still requires a full-size canvas.

A fractional-seam test checked every pixel for continuous opaque bands.
The high-frequency Display P3 page produced 240 × 1040 thumbnails in
both versions, with small per-strip resampling differences. Its
full-resolution output was pixel-identical. Both native light and dark
overlapping-control saves again matched the unobstructed references
within one RGB level. The native fixtures passed on arm64 and under
Rosetta. The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the sample sizes and thumbnail comparison.

### 2026-11-18 macOS scroll save releases source frames as it stitches

The finish job takes ownership of the captured images and releases the
preview session's source references after taking its alignment snapshot.
The compositor discards unaccepted frames and releases each accepted
image after drawing its strip. The full-resolution canvas still goes
through ImageIO for PNG encoding.

In two pairs of generated 3400 × 13740 saves, peak process footprint fell
from 365.6–365.8 MiB to 232.4–232.8 MiB. The inputs and saved PNG bytes
matched. Save times varied across the runs. The comparison began after
all preview work completed. The follow-up below measures Finish with
preview encoding held. An earlier direct-provider probe showed ImageIO requesting all
186,864,000 bytes at once and offered no peak-memory improvement.

The native regression verifies that the captured images are released
after a successful finish. Cancellation and verified-prefix tests still
pass. The [measurement record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the sample timings and the live light and dark verification.

### 2026-11-18 macOS scroll save while a preview is encoding

A composed preview previously held all its input images while encoding
its thumbnail. Its worker now releases those references before encoding.
The save worker can release the input frames as it uses them, even when
the preview encoder is busy.

The native regression held the thumbnail encoder and finished a capture
during that hold. The input image was released, and the saved result
contained both frames. Preview cancellation and publication tests passed.

In two paired 3400 × 13740 saves, with the encoder held after composing
the last preview, peak process footprint fell from 363.4–364.5 MiB to
230.4–230.8 MiB. The saved PNGs were byte-identical. The measurement
covers overlap with the encoding stage. The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains individual results.

### 2026-11-18 macOS scroll save while a preview is drawing

The preview worker now transfers its frame references to the compositor
and releases them strip by strip. When Finish overlaps an active preview,
already-drawn strips can leave memory as the save uses them. The preview
checks cancellation before drawing the next strip.

The native regression held drawing before the third strip, finished
saving all three frames, and verified that a previously drawn image had
been released. After the preview resumed, it published no stale update.
The fixtures passed on arm64 and under Rosetta, and the production Swift
daemon typechecked.

In two paired 3400 × 13740 saves held after five of nine preview strips,
peak memory fell from 364.0 MiB to 288.1 MiB. Finish took 233–239 ms
across the measured runs, and the saved PNGs were byte-identical. The
held preview still owned its undrawn frames in this probe. The
[timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the measurements.

### 2026-11-18 macOS recording preview media readiness at the branch head

The Electron comparison reproduced a loading placeholder over a decoded
recording in dark mode. The video was playing at ready state 4, and the
fixture had received no `loadeddata` event. When the listener missed
`loadeddata`, the renderer stayed in its loading state. While loading,
the preview now accepts a playback time update once the video has
current frame data. Normal `loadeddata` and media error events still
work through their established handlers.

The local Electron fixture deliberately suppressed `loadeddata`. Both
appearances failed to replace the placeholder before the fix. Afterward,
both appearances passed with normal and suppressed events, without
injecting a synthetic ready event. Hover captures showed the recording
under the controls. The GPUI offscreen renderer independently passed
idle, hover and exit for the same generated clip in light and dark.

The macOS Metal harness passed a full 1280-point light profile at
1× with 217 PNGs and both 950-point profiles at 2× with 199 PNGs
each. The focused dark 1280-point scroll, timer, recording, and
recording-control workflows also passed. These runs use generated
frames and a fixture daemon. The broader dark 1280-point run was
stopped after its capture-preview checks in favor of the focused
workflows. The macOS snapshot baseline comparison remains open.

The repository checks passed with the renderer fix, with 2,414 tests
passing and three skipped. Two unrelated licensing tests timed out
during an earlier concurrent broad render and passed when the checks
ran by themselves. The GPUI test suite at `86af4ee3` passed all
1,169 runnable tests with 23 ignored. GitHub showed no pull request or
hosted run for this branch at the time of the check. The local camera
authorization probe reported undetermined, and System Information
listed one built-in Retina display.

### 2026-11-18 macOS toolbar keyboard and pointer tooltip handoff

The native recording toolbar previously kept a keyboard-focus label
visible when the pointer opened a native label over another control.
The camera hover now suppresses the inline keyboard label, and pointer
exit restores it to the focused control. The vendored HeroGPUI focus
tooltip keeps its keyed focus scope mounted while hiding the label. The recording control retains keyboard focus and focus
indication through the handoff.

The native development probe dispatched app-local input in light and
dark mode. After Tab, one inline label painted. Hover over Camera
previously reported that label alongside the native Select camera
label. The fixed build painted zero inline labels during the native
hover. Pointer exit restored one inline label, and the next Tab
showed a single label for the next control.

The offscreen Metal regression also asserted that the focused handle
stayed the same during pointer hover. All 36 existing toolbar PNGs
matched before and after byte for byte. The probe verified AppKit
windows and GPUI dispatch with generated input. Physical pointer
delivery and physical mixed-display focus remain open.

### 2026-11-18 macOS scroll toolbar keyboard and pointer handoff

The native AppKit probe checked Tab and Shift-Tab on the isolated
scroll toolbar in both appearances. Hover displayed Start auto-scroll.
Tab cleared the label. A further app-local pointer move displayed it
again, Shift-Tab cleared it, and pointer exit left it cleared. The
headless regression now checks the Tab-to-hover transition alongside
the existing auto-scroll state and pointer-exit checks. The current
toolbar handled the transition with its existing tooltip code.

The first capture-preview probe did not establish an inline keyboard
label in its headless fixture. The follow-up below verifies that
keyboard path. Physical input, a second display, and camera authorization
remain outside this comparison.

### 2026-11-18 macOS capture preview keyboard access without hover

Capture preview actions were absent from the focus order while the
pointer was away. The first Tab now reveals the controls and moves focus
to an action after the control layout updates. Auto-dismiss waits while
the active preview is in keyboard navigation. The keyboard label keeps
priority during pointer movement. A pointer press returns the preview to
pointer interaction.

The native fixture activated its own preview and dispatched input directly
to that window. In light and dark, the initial idle preview painted no
tooltip. The first Tab focused an action and painted one inline label,
the next Tab focused another action, and Shift-Tab returned to the first.
The automated regression verifies pointer-free Tab navigation, the
auto-dismiss gate, label priority, and the return to pointer interaction.

All 1,170 runnable GPUI tests passed. The dark and light offscreen video
fixtures also passed idle, hover, and exit at 2×. The native keyboard
probe used generated input on the built-in display. Physical input and
mixed-display focus remain open.

### 2026-11-18 macOS keyboard reveal in a clipped capture preview stack

The short-preview regression focused a Close button at y=469–493 in a
360-point viewport. The scroll position stayed at the top, leaving the
focused action outside the visible area. The preview now measures the
focused button and reveals its card when the card fits, so the label has
space too. In smaller viewports it reveals the focused button. Deferred
movement checks that the same focus scope still owns focus.

In light and dark AppKit runs, a 360-point preview of four generated
cards started at the top. Tab moved focus to the clipped card, scrolled
about 240 points, and painted one keyboard tooltip. The regression also
returns to pointer interaction, scrolls to the top, and closes the
expected card. All 1,170 runnable GPUI tests passed. The probe used
app-local input. Physical mixed-display navigation remains open.

### 2026-11-18 macOS keyboard reveal when the preview card cannot fit

The smaller fallback now has explicit coverage. A 128 × 100-point
viewport clipped each 200 × 140-point card on both axes. In both
AppKit appearances, Tab to Close moved the viewport from
`(-80, 0)` to `(-13, -393)` points. Tab to Delete moved it to
`(-67, -393)`, revealing the other side of the card.

The headless regression verifies that the focused Close control stays
inside the viewport after shrinking it and that Delete becomes fully
visible on the next Tab. The AppKit probe used a generated four-card
stack and app-local input. Physical mixed-display focus remains open.

### 2026-11-18 macOS timer startup responsiveness and capture handoff

GPUI previously waited for the timer daemon's show response on the UI
thread. In the native probe, holding a successful reply for 650 ms
also blocked a status command. Timer startup now waits on a background
worker. With the same reply held, the status command returned in
about 0.1 ms in both appearances.

An app-local cancel while the reply was held removed the selector.
After the reply was released, GPUI hid the native panel and saved no
image. The native panel deliberately overlapped the selected region.
Both completed light and dark takes had uniform backdrop pixels across
the 600 × 320 crop. Both showed a preview and opened the image editor.
The countdown completed about five seconds after native show. The
[measurement record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
contains the callback and preview timings. All 1,170 runnable GPUI tests
passed, and all six timer workflow PNGs matched the previous build byte
for byte. The native probe used one physical display, the authorized
screen-only daemon and generated app-local input.

### 2026-11-18 macOS selector handoff from an idle recording toolbar

Changing from the pre-recording area toolbar to a screenshot selector
previously waited on a native recording-overlay Hide call. GPUI now
tracks whether it requested a native overlay. The idle toolbar path
skips the call. A Show attempt dirties the state before the reply,
and a failed Hide keeps cleanup pending for the next attempt.

With the old idle Hide reply held for 650 ms, the area selector
appeared after 790.8 ms. The revised build issued no Hide in the same
idle scenario and showed the selector in 82.5–83.3 ms across light and
dark. Both generated-window checks still issued Hide and removed the
existing outline before showing the selector. The capture driver
cancelled each selector without saving an image. The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the individual samples. The light and dark recording workflows
and all 1,170 runnable GPUI tests passed. The native measurements use
app-local interactions on one Retina display.

### 2026-11-18 macOS responsiveness while switching from a window outline

Switching capture mode from a pre-recording window now closes the old
toolbar immediately and waits for the daemon's outline removal on a
background task. The new selector opens after the Hide acknowledgement.
Escape cancels the queued capture while cleanup finishes. A different
capture intent received during the wait replaces the queued intent.
If Hide fails, the selector stays closed and a later capture retries.

The previous build could not answer an independent app-local status command
during a 650 ms hold on the Hide reply. The revised build answered within
0.1 ms in light and dark. WindowServer showed the outgoing toolbar gone
within 30 ms, and it reported the destination selector only after the
acknowledgement. The same native probe covered cancellation, replacement
with timer capture, and retry after an injected acknowledgement error.
With the reply delivered normally, five of six switches took 58.5–69.7 ms
to show the selector. One dark trial took 372.7 ms.
The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the raw samples from the built-in Retina display. The light and
dark recording workflows and all 1,171 runnable GPUI tests passed.

On the pushed build, six screenshot cycles per profile also covered
area-to-window switching, cancellation, preview and editor opening.
Warm median selector appearance was 20.0–33.5 ms, switching was
13.4–34.4 ms, and cancel-to-hidden was 6.7–13.2 ms. Warm median
confirm-to-visible-preview was 55.2–100.8 ms and preview-to-visible-editor
was 20.3–40.1 ms. The first large live screenshot preview took 154.5 ms.
The timing record includes cold and warm samples and a follow-up
unheld recording-outline comparison. The four follow-up trials did not
reproduce the 372.7 ms observation.

### 2026-11-18 macOS pre-recording cancellation responsiveness

The native Escape event and the toolbar's own Escape action now share
the background outline cleanup used when switching capture modes.
With a 650 ms delay injected into the successful Hide reply, both
baseline paths blocked the app for the duration of the hold. The
revised build answered an independent status command within 4.2 ms
in all four trials. WindowServer showed the toolbar and outline gone
within 71 ms, while the acknowledgement was still held.

A timer requested during that wait became visible after the reply.
Cancelling the queued timer kept the overlay closed. The same probe
also verified that direct cancellation left no selector and captured
no pixels. The [timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
covers both appearances and both cancellation entry points on one
Retina display.

### 2026-11-18 macOS recording preview and editor during outline cleanup

Leaving the recording toolbar after Stop, Discard or terminal failure
now schedules outline cleanup off the UI thread, using the same queue
as pre-recording cancellation. An already-finalized recording can open
in the preview or editor during that wait. A new capture intent still
waits for the daemon to acknowledge removal before opening a selector.

In the native area recording probe, holding the successful Hide reply
for 650 ms delayed the old preview to 709.8 ms after Stop and blocked
the app through the hold. The revised preview appeared in 58.7–63.0 ms,
and its first decoded frame was ready in 76.8–84.8 ms. The video editor
opened from the preview during the hold in both appearances. A timer
selector requested during the hold opened after release. Both recordings
decoded as video-only with the generated backdrop and no toolbar or
recording outline in the sampled frame. The recording parity workflow
also holds the Hide reply and asserts that opening directly in the
editor works while cleanup is pending. The [native timing record](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)
includes the individual samples for one Retina display.

### 2026-11-18 macOS recording toolbar tooltip replacement

The native GPUI probe moved between all six pre-recording toolbar actions
with app-local unpressed pointer events in light and dark. The requested
native label followed the selected control in both directions, including
back-to-back moves. During pointer hover the keyboard label stayed hidden.
Pointer exit cleared the native request and restored the focused keyboard
label.

The AppKit regression separately holds an in-flight screenshot while the
pointer moves from Start recording to Camera to Microphone. It verifies
that only the last label appears after release, that a delayed hide from
the earlier control is ignored, and that superseded panels stay excluded
until retirement. The replacement fits the display work area and clears
the toolbar. [Native tooltip handoff evidence](tests/native/evidence/macos-ui/native-tooltip-control-handoff-20261118.txt)
records the cases. Physical unpressed pointer delivery and mixed-DPI
placement remain open.

### 2026-11-18 native macOS single-display screenshots

The real GPUI screen capture saved 3456×2234 PNGs on the built-in
Retina display in both appearances. The single-display command opened
the preview without a picker, and the editor opened each saved image.
Replaying the Electron display capture request against the same
authorized daemon produced the same dimensions and matched
7,720,464 of 7,720,704 GPUI pixels exactly after ColorSync conversion.

Switching from a real frozen area selector closed the selector and
native freeze window. The app answered a status command during an
injected 650 ms release hold and sent its capture request only after
the acknowledgement. The PNG contained the newer backdrop color.
A timer requested during the same hold replaced the pending display
capture, and cancelling that timer left no output files. The 32
focused GPUI tests and 53 Electron reference tests passed. [Native
display evidence](tests/native/evidence/macos-ui/native-full-display-capture-20261118.txt)
records the timings and pixel checks. A physical multi-display picker
remains unverified.

### 2026-11-18 native macOS window screenshot after target changes

The window picker selected a disposable AppKit target on the Retina
display. After the picker cleared, the fixture moved and resized the
target before forwarding the screenshot request. Live capture followed
its window ID and saved the new green 560×640 frame. Frozen capture
retained the original red 720×480 frame, including when the target
closed. All saved frames matched their corresponding solid backdrop
across the entire image after ColorSync conversion to sRGB. Preview
and editor opening passed.

A live capture of the closed window returned a definite failure
and previously left an empty staging PNG. GPUI now preserves the
daemon error code through the screenshot client and removes an empty
stage after a definitive failure, matching Electron's cleanup rule.
Nonempty stages and ambiguous transport failures remain recoverable.
The shared area and window capture paths use this rule. The
[native window evidence](tests/native/evidence/macos-ui/native-window-screenshot-target-change-20261118.txt)
records both appearances and the focused regression.

### 2026-11-18 macOS scroll finish that reports success without an image

A parity fixture reported successful Finish while leaving the GPUI staging
file empty. The old build promoted it to a zero-byte PNG. GPUI now checks
that the reserved output is a nonempty regular file before publishing it.
An empty stage takes the existing no-image error path. The selector and
controls close, and history, preview and clipboard stay clear of the
failed result.

The [scroll finish evidence](tests/native/evidence/macos-ui/scroll-empty-finish-20261118.txt)
records the failure and the light and dark regression cases. Those
workflows also cover valid output, partial-file retention, retries and
cancellation during Finish.

### 2026-11-18 macOS screenshot publication after an empty success

The GPUI timer fixture completed its countdown and received a
successful Screenshot response while its staged PNG was still empty.
The old build published the zero-byte image. The new build removes
the stage, shows the no-image message, and leaves history unchanged.
The same publication check now serves area, display, window, timer,
and scroll captures. A nonempty capture held after a publication
failure remains recoverable, and a timeout with an uncertain result
still retains its staging file.

The [screenshot publication evidence](tests/native/evidence/macos-ui/screenshot-empty-success-20261118.txt)
records the injected macOS timer reply and the ordinary timer and
scroll save checks in both appearances.

### 2026-11-18 macOS recording menu frames during native resize

The native recording toolbar occasionally stretched the compact
Metal frame across the expanded window as a device menu opened.
The toolbar now aligns the layer content to the top edge at its
original scale while AppKit resizes. The change applies to the
recording toolbar's backing layer. [Apple layer documentation](https://developer.apple.com/documentation/quartzcore/calayer/contentsgravity)

The [old expanded frame](tests/native/evidence/macos-ui/recording-menu-resize-stretched-20261118.png)
and [revised waiting frame](tests/native/evidence/macos-ui/recording-menu-resize-pinned-20261118.png)
show the difference. The [painted Camera menu](tests/native/evidence/macos-ui/recording-camera-menu-composited-dark-20261118.png)
shows the final layout. Across 42 openings per build, two of 148
baseline samples stretched and zero of 157 revised samples stretched.
The baseline issue was intermittent. The window captured the expected
microphone, camera, and empty iOS menus in both appearances using
generated device data. [Native timing and pixel evidence](tests/native/evidence/macos-ui/recording-menu-compositing-20261118.txt)

### 2026-11-18 macOS capture preview frames during native resize

Growing the preview stack could stretch the previous preview frame
to the height of the new AppKit window. A 280-pixel Retina card
measured 566 pixels in the [old two-card transition](tests/native/evidence/macos-ui/preview-stack-resize-stretched-20261118.png).
The capture preview now shares the recording toolbar's native
layer alignment. It pins the previous frame to the selected top or
bottom edge while the resized frame is drawn. The [revised transition](tests/native/evidence/macos-ui/preview-stack-resize-pinned-20261118.png)
shows the retained card at its original scale and bottom edge.

In the baseline bottom-right probe, stretching appeared in eight of
12 growth transitions. The revised probe saw no stretched cards in
24 growth and shrink transitions covering top-right and bottom-right
in light and dark. The window stayed anchored and returned to one
card after dismissal. [Native preview pixel evidence](tests/native/evidence/macos-ui/preview-stack-resize-20261118.txt)
records the generated input and capture method.

### 2026-11-18 macOS native status-bar menu dismissal

Sampling the live GPUI status-bar menu with synthetic left and right
anchors showed fitted 304×564-point frames and no stretched menu pixels.
Dismissal exposed an additional AppKit window transition after the menu's
own animation. The tray now disables the native transition at the point
of dismissal. Escape still plays the menu's 100 ms fade.

Warm median toggle-to-hidden fell from 162.0 to 9.2 ms in light and from
161.1 to 13.9 ms in dark. Warm Escape-to-hidden fell from 266.1 to
119.8 ms in light and from 270.1 to 118.2 ms in dark. A rapid
Escape, toggle, and reopen sequence kept the replacement registered
after the previous menu's delayed callback, so the new menu still
closed on Escape. [Native tray evidence](tests/native/evidence/macos-ui/native-tray-dismissal-20261118.txt)

### 2026-11-18 macOS capture actions from the status-bar menu

Selecting Capture Area previously dispatched while the tray menu was
still visible. The live selector appeared at about 22 ms and shared the
screen with the menu. With Freeze Screen on, the menu was visible when
the native freeze request reached the daemon.

Tray capture actions now hide and remove their menu before dispatch.
The revised live selector appeared in 33.5–39.9 ms with the menu gone.
The frozen runs had no menu at the freeze request or when the selector
appeared. Both appearances saved a 380×320 crop covering the former
menu position in live and frozen modes. Every pixel matched the
generated backdrop. [Native handoff evidence](tests/native/evidence/macos-ui/native-tray-capture-handoff-20261118.txt)

### 2026-11-18 macOS capture shortcuts with the status-bar menu open

The native Capture Area event could still start capture while the tray
menu was open. In the two baseline frozen runs, the menu remained
visible at the freeze request and throughout the seven-second probe.
The dark live selector also appeared while the menu was visible.

The shared macOS capture dispatcher now closes any open tray menu
before entering the capture flow. Both revised live runs had a clear
selector in 26.0–31.6 ms. Both frozen runs and a run during Escape
dismissal had no tray window at the freeze request or selector
appearance. Selecting the Capture Area row still passed. Every
pixel in each native 380×320 crop of the former menu region matched
the generated backdrop. [Native shortcut evidence](tests/native/evidence/macos-ui/native-tray-shortcut-handoff-20261118.txt)

### 2026-11-18 macOS toolbar recovery after a failed recording start

A definite area-recording startup error left the UI waiting for the
native overlay Hide reply. With that successful reply held for 650 ms,
the baseline could not answer an app-local status request. The revised
toolbar processes events while cleanup runs off the UI thread.

Start stays disabled until cleanup completes, so a second click cannot
start a recording over a pending Hide. The pre-recording toolbar can
still be cancelled. App-local retries in light and dark saved a
screen-only recording and opened its preview. Cancelling during the
hold left no recording project. [Native startup recovery evidence](tests/native/evidence/macos-ui/native-recording-start-rollback-20261118.txt)

### 2026-11-18 macOS recording outline Show during startup

The area recording toolbar previously blocked while the daemon showed
the recording overlay. A 650 ms hold on the successful Show reply
blocked an independent app-local status command throughout the hold.
GPUI now does that wait off the UI thread. Start stays disabled and
the recorder waits for the overlay acknowledgement.

The revised native status replies took 0.08–2.56 ms during the hold.
After release, the light and dark runs recorded 600×440 screen-only
video and opened the preview. Cancel before release delayed Hide until
Show completed and left no recording project. A capture switch waited
for Show and Hide before opening its selector. An injected failed
overlay response cleaned up and re-enabled Start. [Native startup evidence](tests/native/evidence/macos-ui/native-recording-overlay-show-20261118.txt)

### 2026-11-18 macOS All-in-One frozen recording preparation

Starting an area recording from a frozen All-in-One selection used
to block the toolbar while the daemon released the frozen screen.
A held Release response stalled the baseline parity fixture until
the response timed out. GPUI now waits off the UI thread and
checks that the selection and toolbar still own the attempt before
continuing to the recording overlay.

The native toolbar answered app-local status requests in 0.04–0.86 ms
during a 650 ms hold. Both appearances recorded screen-only video
and opened the preview after Release, Show and Start. Cancel closed
the toolbar and selector without a project. Switching to a screenshot
opened a new selector after release, with no stale recorder request.
[Native evidence](tests/native/evidence/macos-ui/native-frozen-recording-release-20261118.txt)

### 2026-11-18 macOS All-in-One frozen window recording selection

Choosing a window from a frozen All-in-One recording selector blocked
the UI on Release before it could show the window outline and toolbar.
A held reply timed out in the baseline parity fixture. The release
now runs off the UI thread. The selected window is handed off only
if the selector and its window list still own the pick. Escape can
dismiss the selector during the wait.

In native light and dark runs the app answered in under 1 ms during
a held 650 ms release. The outline and controls opened after release.
Each selected window recorded a 720×480 video and opened a preview.
Cancelling during the hold left no controls, outline, or project.
[Native window handoff evidence](tests/native/evidence/macos-ui/native-frozen-window-recording-release-20261118.txt)

### 2026-11-18 macOS frozen timer selection handoff

Confirming a frozen timer selection blocked the UI while the daemon
released the frozen screen. With the successful Release reply held,
the native baseline status command waited 652 ms. GPUI now releases
off the UI thread while keeping the selector available for cancellation.
The countdown waits for Release and for the current selection to
still own the request.

The revised native command answered in 0.10–21.94 ms during the hold.
Both light and dark counted down normally and saved clean 600×320
PNGs over a generated backdrop. Cancel and switching to a screenshot
left no stale countdown or timer output. [Native timer evidence](tests/native/evidence/macos-ui/native-timer-freeze-release-20261118.txt)

### 2026-11-18 capture reservations during freeze release

A deterministic test held Release and then reserved a capture against
the same generation. The old code chose frozen pixels immediately.
The revised capture waits for the release result, uses live pixels
after success, and can still use the retained freeze after failure.
The color picker now uses the same cached-or-live decision as the
screenshot paths. The regression isolates the cache decision from
native pixel output.

### 2026-11-18 redundant freeze release during capture replacement

A native timer-to-screenshot switch sent a second Release after the
daemon had acknowledged release of the old generation. GPUI now records
the acknowledgement under the freeze lock. The replacement can skip
that Release and begin its own freeze. A failed release remains
eligible for retry, and newer generations still need their own release.

The native probe held the first successful release reply for 650 ms
and switched to a screenshot while it was held. The baseline sent
two Release requests before showing the replacement. The revised
light and dark runs each sent one, displayed the replacement after
release, and left no stale countdown or saved output. A focused unit
test covers the successful, failed, and newer-generation cases. All
1,175 GPUI tests and 48 Electron reference tests passed, along with
the light and dark timer and recording workflows.

### 2026-11-18 native macOS fallback after a failed freeze

The native development probe on one Retina display let the authorized
daemon install a real freeze over a generated backdrop. The bridge
then substituted an error for the successful freeze acknowledgement.
GPUI issued Release, and the probe held its reply for 650 ms. The
app-local status round trip was 0.27 ms in light and 0.16 ms in dark.
The selector opened after cleanup. With the backdrop changed from
blue to orange, both saved one 600×320 PNG using the live path.

A separate dark run rejected the first Release before forwarding it
to the daemon. The real frozen surface stayed present, the selector
stayed closed, and the app-local status reply took 0.29 ms during
the hold. The next capture retried Release, made a new freeze, and
saved the selected pixels. Every pixel in each of the three saved
PNGs matched the new backdrop at (194, 77, 30) after ColorSync
conversion. Each opened a preview, and the native freeze surface
cleared after the capture.

The failure replies were injected at the app boundary. The underlying
AppKit freeze, release, and captures ran through the screen-only
daemon. The probe used application-local input and 48 Electron
freeze and capture reference tests passed.

### 2026-11-18 macOS freeze deadline and late acknowledgement

An isolated native GUI probe withheld the first Freeze acknowledgement
at the daemon bridge. The unchanged GPUI client reached its 30-second
deadline and sent Release at 30.006 seconds in dark and 30.016 seconds
in light. Delivering the expired request's success while Release was
pending left the selector closed.

The dark fallback opened after Release and sent its screenshot
request with cached=false. It saved one generated 600×320 PNG and opened
a preview. A subsequent freeze waited for its own acknowledgement.
The light run requested its replacement during the original hold.
Even after a second delivery of the old success, the replacement
stayed closed until its own Freeze reply. Cancelling it left no
screenshot or redundant Release request.

This probe ran the real GPUI request deadline and native selector
lifecycle with a synthetic daemon. The fixture supplied screenshot
pixels, and the probe used app-local events on one Retina display.
All 48 Electron freeze and capture reference tests passed.

### 2026-11-18 macOS window recording outline acknowledgement

After a window was picked, GPUI blocked the toolbar on the daemon's
ShowWindow reply. Holding that reply for 650 ms blocked an app-local
status request for the duration. The outline request now runs off the
UI thread. The existing preparation gate keeps Start disabled until
the reply and orders Hide after Show for a cancelled or replaced pick.

Revised native runs answered status in 0.05–4.89 ms during the hold.
Both appearances recorded 720×480 video and opened the preview.
Cancellation and switching to a screenshot waited for Show and Hide
without starting a recording. Each run issued one window-outline
request. The light and dark parity workflows also hold ShowWindow
and verify that Start is disabled and the recorder idle. [Native outline
evidence](tests/native/evidence/macos-ui/native-window-outline-acknowledgement-20261118.txt)

### 2026-11-18 macOS startup freeze prewarm and Screen Recording access

The GPUI and Electron startup paths each issue one prewarm call to
the shared macOS daemon. The daemon now checks for existing Screen
Recording access before reading a screen frame. A denied preflight
returns prewarmed=false, as does an unavailable image. The call
uses Apple's permission status API. [Apple permission guidance](https://developer.apple.com/forums/thread/839069?answerId=898801022)

The native lifecycle fixture injected a denied status and verified
zero capture calls. It also checked successful and unavailable frame
results with access granted by the fixture. Both the focused check
and the generated AppKit lifecycle passed on arm64 and under Rosetta.
The Swift daemon typechecked and 41 Electron and daemon contract
reference tests passed. The test uses simulated permission outcomes.

### 2026-11-18 macOS History popover dismissal and editor handoff

History now disables the AppKit window transition at dismissal. A second
toggle and the popover's keyboard actions share the same close path.
In the GPUI dev app on the built-in Retina display, a generated screenshot
appeared as a History thumbnail in both themes. The native popover measured
400 × 500 points. [Light capture](tests/native/evidence/macos-ui/history-popover-light-20261118.png)
[Dark capture](tests/native/evidence/macos-ui/history-popover-dark-20261118.png)

| WindowServer observation                                   | Light before → after | Dark before → after |
| ---------------------------------------------------------- | -------------------: | ------------------: |
| Toggle to History dismissal, warm median                   |      165.2 → 20.2 ms |     167.0 → 13.7 ms |
| Escape to History dismissal, warm median                   |       166.6 → 9.7 ms |      168.4 → 9.6 ms |
| Enter on the screenshot to editor visibility, one handoff  |       46.4 → 42.6 ms |      49.3 → 50.9 ms |
| Enter on the screenshot to History dismissal, same handoff |      208.4 → 51.8 ms |     206.2 → 52.2 ms |

The revised warm open samples ranged from 23.2 to 32.0 ms. Each dismissal
median uses three samples after the first attempt for that action. The
probe measured WindowServer visibility with app-local events and an
isolated History profile. The editor timing ends when its window is
visible. All 53 Electron History and popover reference tests passed.

### 2026-11-18 macOS onboarding pages and exit paths

The GPUI dev probe opened onboarding with a fresh profile four times on
the built-in Retina display. The 500 × 650 point window showed the Welcome,
Disable macOS Screenshot Shortcuts, Set Your Shortcuts and Setup Permissions
pages in both appearances. Next reached each page, and Back returned from
permissions to shortcut setup. Both permission rows showed existing grants.
[Light pages](tests/native/evidence/macos-ui/onboarding-light-20261118.png)
[Dark pages](tests/native/evidence/macos-ui/onboarding-dark-20261118.png)

Skip for now and Get Started each closed the window and persisted their
respective status in separate profiles. A repeat open request honored that
status. First WindowServer visibility took 181–215 ms, and dismissal took
17–25 ms across the four trials. The probe raised only its onboarding
window above occluding windows and routed clicks inside that window.
Permission request and Settings actions were outside the run. All 15
Electron onboarding reference tests passed.

### 2026-11-18 macOS recording startup phase timings

Sixteen fresh GPUI area takes on the built-in Retina display recorded
generated static or changing pixels through a preflight-gated screen-only
daemon built from the current Swift sources. All sixteen decoded at
600 × 440, and the daemon confirmed Start in 195–263 ms. The previous
4.19-second outlier did not recur. Stop to visible preview took 56–136 ms.

Four detailed trials attributed 76–78 ms to configuring and attaching
the video input, 50–57 ms to creating the ScreenCaptureKit stream, and
69–75 ms to starting it. The protected-window filter check took under
0.06 ms across all sixteen takes. The measurements used the existing
screen grant. Live audio, camera and the cause of the earlier outlier
remain open. [Per-run timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-18 macOS video writer and stream startup overlap

The daemon now configures the video writer in a child task while the
main actor constructs the ScreenCaptureKit stream. It joins the task
and takes ownership of the writer before starting capture. The existing
protected-window filter and first-frame tests still exercise startup ordering.
The generated static timeline cases now use the production writer setup.

Daemon Start confirmation was faster in all six paired native runs, with
a median paired improvement of 44 ms. All twelve video-only clips decoded
at the expected area or window size, and the moving takes opened in the
editor. The tests used generated AppKit panels on one Retina display and
the preflight-gated screen-only daemon. The arm64 recording timeline suite,
two static timeline cases under Rosetta, and all 53 Electron reference
tests passed. Live system audio, microphone and camera were outside this
comparison. [Per-run timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-18 macOS recording stopped before the first frame

Start could confirm when ScreenCaptureKit had opened its stream without
delivering a usable frame. An immediate Stop then finalized an empty
video writer. Start now confirms after the first frame is either written
or retained for finalization under writer backpressure. A stream failure
before the first frame ends startup with the existing error classification.

With app-local input and a preflight-gated screen-only daemon, all eight
area trials previously returned an asset-writer error when Stop arrived
during startup or immediately after confirmation. With the change, all
eight area trials and four window trials decoded two frames of the
generated panel. After Start confirmation, Stop to visible preview took
72–129 ms. When Stop was queued during startup, it took 235–324 ms from
the request to a visible preview, including the remaining startup time.
These runs used one Retina display and disabled keystroke capture.
Live audio and camera were outside the runs. [Short take timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-18 macOS live system-sound control responsiveness

The recording toolbar previously waited on the system-sound daemon
reply on the UI thread. The macOS path now sends the request in order
and waits off the UI thread. The button retains its confirmed state
and blocks repeat clicks until the reply. A rejection leaves the
original state intact and allows a retry.

In the native app with an isolated simulated daemon, a status check
and repeat toggle returned within 0.09–9.45 ms while the reply was held.
Stop reached the fixture before the hold was released in 5.77 ms in
light and 8.36 ms in dark. The fixture observed one audio request
before Stop and exercised both accepted and rejected replies. The Metal
toolbar workflow separately checked the actual button against a held
reply, and the wider recording workflow exercised a terminal audio
failure. All 1,175 runnable GPUI tests and 111 Electron reference tests
passed. The native probe used app-local input and simulated system audio.

### 2026-11-18 macOS microphone changes during recording

The GPUI microphone menu now sends changes in order and waits for the
daemon off the UI thread. While a switch is pending, it displays the
confirmed device and keeps the microphone menu closed. A rejected switch
preserves the previous device and allows a retry. Live changes stay
within the recording session.

In light and dark native runs against a simulated daemon, microphone
change commands returned in 0.12–1.53 ms. Stop reached the fixture in
7.67–8.21 ms while a microphone reply was held. Accepted switches
selected the replacement device, while rejected switches kept the
original. The Metal toolbar workflow separately exercised the real
microphone menu during a held disable acknowledgement. These checks
used an isolated profile and did not access the microphone.

### 2026-11-18 macOS live camera toggle and preview responsiveness

During recording, GPUI now waits for camera and preview replies off the
UI thread. Enabling waits for the protected preview before enabling the
camera track. Disabling keeps the confirmed state and preview until the
recorder accepts the change. A rejected disable leaves the preview open.
Stop sends preview cleanup even if the preview open is still pending.
A late open reply cannot re-enable the camera after Stop.

Ten native runs used isolated light and dark profiles and a simulated
daemon. Stop reached the daemon in 0.62–11.05 ms while a recorder toggle,
preview open or preview close reply was held. The fixture verified that
each preview was closed once and that a late open never sent a camera
enable request. The Metal toolbar workflow also holds the camera disable
reply and checks the confirmed state before allowing the hide. All
1,175 runnable GPUI tests and 111 Electron reference tests passed. The
native probe used app-local input and simulated camera responses.

### 2026-11-18 macOS camera preview before recording

Opening the pre-recording camera preview now waits for the daemon off
the UI thread. Start stays disabled until the preview request resolves.
The camera selection is saved after a successful reply. Closing or
replacing the toolbar invalidates its pending reply and sends Hide for
the preview, including a preview that has yet to appear.

Ten native runs with a simulated daemon covered accepted and rejected
device selections, restoring an enabled camera, cancellation, and
switching to an area screenshot in light and dark. While Show was held,
cancellation completed in 21–23 ms and the screenshot selector became
active in 53–56 ms. Both paths sent Hide before the delayed reply and
sent no Start request. The Metal toolbar workflow also checks that
Start is disabled during a pending Show. The fixture simulated camera
replies with no device access.

### 2026-11-18 macOS recording Pause and Resume responsiveness

The GPUI toolbar waits for Pause and Resume acknowledgements off the UI
thread. It holds its confirmed state and disables duplicate commands
during the wait. Stop can proceed, and a late reply leaves a successful
Stop in effect. If Stop fails, the toolbar uses the most recent confirmed
pause state regardless of which reply arrives first.

Ten native runs with simulated replies covered light and dark, accepted
Pause and Resume, a rejected Pause and retry, and Stop with a held Pause.
Stop reached the fixture in 1.35–8.07 ms. Both reply orders recovered to
the confirmed paused state after a failed Stop. The Metal toolbar
workflow also checked the rendered Pause and Resume controls with both
replies held. The native probes used app-local input and a fixture daemon.

### 2026-11-18 macOS camera protection cleanup after Stop

After a successful Stop, GPUI sends camera protection cleanup before
returning to Idle and waits for the reply off the UI thread. The macOS
daemon applies protection changes in request order on the main queue
so a subsequent recording can enable protection safely.

With the cleanup reply held, the native GPUI test decoded the preview
frame in 93–113 ms after Stop and the editor frame 77–108 ms after opening
it from the preview. Both appearances used a generated recording and a
simulated daemon. The Metal recording workflow checks direct editor
opening while the same reply is held. The AppKit lifecycle fixture
checks protection request order and the panel's final sharing state.

### 2026-11-18 macOS camera protection before recording

GPUI now waits for camera protection off the UI thread before reserving
a project or sending Start. The toolbar prevents duplicate starts while
the request is pending. Escape and capture switches release protection
and invalidate the waiting start. A rejection keeps the toolbar available
for a retry.

Eight native runs used a simulated daemon in light and dark. With the
protection reply held, cancellation took 14–29 ms, and switching to an
active area selector took 50–53 ms. Neither path sent Start or reserved
a project when the late reply arrived. The acceptance and rejection
runs verified that Start followed a successful protection reply. The
Metal recording workflow holds both startup and cleanup protection
replies to check the start gate and editor handoff.

### 2026-11-18 macOS resumed recording of a static scene

When the scene changed during Pause and stayed static after Resume,
ScreenCaptureKit supplied no new image and the movie held the pre-pause
frame. The recorder now keeps the latest paused screen frame in memory
and writes it at the resume boundary. Stopping while paused releases it
without writing it.

Live light and dark area takes used a generated panel that was red before
Pause, green during it, and static blue on Resume. Both previously decoded
as red only. The revised clips decoded red followed by blue, with no green.
The clips were 0.803 and 0.860 seconds after 1.641 and 1.657 seconds had
elapsed. Stopping while paused retained only the red segment. All four
takes decoded in the preview and editor. The tests used the existing
Screen Recording grant with audio, camera and keystroke capture disabled.
[Timing and frame evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-18 macOS paused take through edit, reopen and export

Two live 600 × 440 area takes used a generated panel with fixed shapes
over the red, green and blue pause-test colors. The recorded sources
decoded red followed by blue. GPUI's Metal editor workflow imported each
real source, added a zoom segment and a filled rectangle, reopened the
saved project and exported at the original canvas size.

The reopened previews showed the saved edits. Preview and decoded export
mean channel error was 0.87 at 0.5 and 1.0 seconds and 1.10 at 1.5 seconds
in both appearances. The rendered preview at 1.0 seconds averaged
1.19 RGB levels of difference from the decoded export. Both exports
retained the red-to-blue order and rounded the 2.250–2.252-second
source duration to 2.267 seconds on the export frame grid.
Live capture used the existing screen grant. Editor gestures were
generated by the parity workflow. [Capture and export measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-18 macOS paused window movement, hiding and closure

Six live window recordings covered movement, hiding and restoration,
and native window release while paused, in both appearances. Moving or
temporarily hiding the target preserved the recording and its window ID.
After Resume, each clip decoded red followed by blue at its original
640 × 480 size. Green was visible only while paused and was absent
from the decoded frames.

Closing and releasing the target removed its ID from the full
WindowServer list. The daemon reported `TARGET_CLOSED` once in
each run, and GPUI stopped automatically. The decoded preview appeared
121–164 ms after closure, and the output held only the red segment.
The outline hide acknowledged completion and all six takes opened
in the editor. The first fixture retained its native window and tested
hiding. Releasing it after the AppKit run loop started verified actual
closure. The recorder passed with its existing close handling.
[Native timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-18 macOS scroll preview corner clipping

The native GPUI scroll fixture rendered a solid generated preview in
light and dark. Both the regular placement and the 240-point-wide
selection displayed a 240 × 180-point image at 2×. The four extreme
corner pixels contained only the shadow, with alpha 6 at the top and
16 at the bottom. Four device pixels inward, all corners matched
the fully opaque interior. [Native preview capture](tests/native/evidence/macos-ui/scroll-preview-retina-corners-20261118.png)

The 2-point preview radius follows the Electron renderer's
`rounded-lg` token. The macOS Metal scroll workflow now runs the
existing corner pixel and Finish/Cancel assertions. The native
window captures used one physical Retina display.

### 2026-11-18 macOS scroll preview growth at the bottom edges

The native scroll fixture delivered 60-, 200-, and 960-point generated
frames to the same preview, beside a 96-point selection at each bottom
edge of the built-in Retina display. In both appearances the preview
stayed 240 points wide and on the side with available space. Its painted
image heights were 60, 200 and 360 points.

The short image aligned with the selection and ended 12 points above
the work-area bottom. The taller images moved up to fit, ending exactly
at the work-area bottom. The capped preview showed only the bottom
360 points of the tall fixture. No pixels from its contrasting upper
strip were visible. The check sampled the native preview window with
app-local frame delivery. Physical narrow and mixed-DPI displays remain
outside this run.

### 2026-11-18 macOS scroll controls during an auto-scroll reply

The GPUI scroll toolbar now waits for Start and Stop Auto-scroll replies
off the UI thread. It disables repeat toggles while the reply is pending.
Cancel can still send its signal. After cancellation, a late error
does not present an auto-scroll failure.

Eight native runs used a simulated daemon and generated preview frames
in light and dark. The preview settled on the newest frame in a burst.
With either command reply held, app-local Cancel dispatched in
0.07–0.18 ms. A frame submitted after cancellation left the preview
unchanged. Rejected replies cleared the pending state and allowed a
retry. These measurements end at the GPUI signal, before native scroll
cancellation completes.

### 2026-11-18 macOS camera preview Hide when leaving recording setup

Closing the pre-recording toolbar sends camera Hide and closes the
toolbar while the daemon reply is pending. Capture switches wait for
camera and outline cleanup. Escape can cancel a queued switch during
the wait. The camera and outline requests can complete independently.

Eight native runs used simulated replies in light and dark. Cancel and
capture switch commands returned in 15–18 ms while Hide was held.
WindowServer confirmed that the toolbar closed during the hold.
The area selector opened only after release, and cancelling a queued
switch kept it closed. The Metal recording-controls workflow also
holds Hide and asserts that the toolbar has closed before the
new selector opens. These tests used no live camera feed.

### 2026-11-18 macOS camera preview repositioning during recording setup

Retargeting the pre-recording toolbar now sends the camera preview
position and waits for the reply off the UI thread. GPUI keeps the
position reported by AppKit, including any work-area clamping. The
daemon applies updates in request order on the main queue.

In four native runs with simulated replies, a second reposition reached
the daemon in 5.49–9.69 ms while the first reply was held. Releasing the
replies in reverse did not change the last reported position. Close
reached Hide in 16–20 ms while both replies were pending. The AppKit
lifecycle fixture also sent two position requests and checked the
last clamped position on its generated panel. No camera feed was used.

### 2026-11-18 macOS camera-off toggle during recording setup

The pre-recording camera-off toggle now waits for Hide off the UI
thread. It keeps the confirmed state and preference until Hide
succeeds. A rejected reply keeps the camera enabled. Start waits until
the toggle resolves.

Eight native runs used simulated replies in light and dark. A camera-off
command reached Hide in 6.05–9.42 ms. Closing the toolbar or switching to
an area screenshot reused the pending Hide instead of sending another.
The area selector waited for cleanup, and a late reply after leaving
setup did not change the saved preference. The Metal workflow holds
Hide during the camera-off toggle and checks the selector handoff.
No live camera feed was used.

### 2026-11-18 macOS live OCR and QR area selections

The native GPUI probe selected a 300 × 220-point area of a generated
AppKit panel on the Retina display. The Swift daemon captured a
600 × 440 PNG and recognized all three English and Chinese lines
or decoded the exact generated QR payload. The blank panel returned
empty results for both methods in light and dark.

With the analysis reply held, the selector cleared in 1.0–8.4 ms
from confirmation. The completed trial set measured 162–184 ms
for text and 40–43 ms for the QR code. Two initial OCR attempts
exceeded the probe's 12-second observation limit, and one earlier QR
call took 2.06 seconds. The cause of those slow observations remains
open.

The bridge checked recognition before returning an empty result to
GPUI. This exercised its cleanup path and kept the system clipboard
untouched. Temporary PNGs were removed and the screenshot output
folder stayed empty. Clipboard delivery remains unverified.
The probe used the existing screen grant and app-local input, and
all 34 Electron reference tests passed.

### 2026-11-18 OCR and QR completion order on macOS

Only the newest confirmed OCR or QR selection can publish a clipboard
result or completion notification. Recognition and temporary-file
cleanup still finish for an older request. The newer selection can
complete while the older daemon reply is pending.

The light and dark Metal analysis workflows hold the OCR reply, confirm
a later QR selection and verify that the QR result survives release
of the older reply. Both temporary capture files are removed and the
older reply posts no notification. The workflow uses GPUI's isolated
clipboard. Live system clipboard delivery and the cause of the
intermittent OCR delay remain open.

### 2026-11-19 macOS OCR and QR first-request timing

A standalone timing fixture built the current Swift OCR and QR modules
and ran 60 requests across 12 fresh processes. It used generated
600 × 440 PNGs and no capture selector or clipboard. The first OCR
call reproduced the prior delay at 14.44 seconds. Vision's request
handler accounted for 14.42 seconds. Image decoding took 6.15 ms,
request configuration took 10.04 ms, and reply dispatch took 0.16 ms.

The first QR call in that process took 1.09 seconds, nearly all inside
Vision. Each request recognized the expected bilingual text, exact
QR payload or empty result. Median round trips by input were
44.53–44.75 ms for text, 36.23 ms for QR and 32.80 ms for blanks.
Fresh processes shared the existing macOS service state. The cause
inside Vision and performance after an OS restart remain unverified.
[Stage timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS visible countdown accessibility

The AppKit timer fixture now checks the visible countdown as well as
the hidden test view. Its accessibility frame matches the 140 × 52-point
panel in screen coordinates. A center hit test locates the countdown
button, and the real timer updates its value from “2 seconds remaining”
to “1 second remaining”. [AppKit accessibility frame](https://developer.apple.com/documentation/appkit/nsaccessibility-c.protocol/accessibilityframe)

The fixture presses the button through AppKit and observes one
cancellation, with the panel removed or transparent in WindowServer.
A second press on the retired element is rejected. The macOS native
timer workflow runs this case in CI. VoiceOver navigation and speech
remain unverified.

### 2026-11-19 macOS first frame with a 4K video source

The native GPUI probe opened a recording preview and editor from
generated 1080p and 4K H.264 clips on the built-in Retina display.
Six runs per size covered light and dark. Median Stop-to-thumbnail
time was 173 ms at 1080p and 325 ms at 4K. Median preview-to-editor
frame time was 170 ms and 448 ms, respectively.

The stage trace placed the largest thumbnail delay in the FFmpeg
frame request, which includes decode and BGRA transport. It took
156–647 ms at 4K, and thumbnail scaling and conversion took
35–38 ms. The measured editor composition stage includes decoding.
A standalone single-thread FFmpeg comparison produced identical
4K software-decoded bytes, but the native app comparison had slower
median readiness with that setting. The decoder settings were kept.
[Readiness and decoder measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 FFmpeg decoder checksum work on macOS

GPUI uses FFmpeg's `showinfo` filter for frame timestamps. The decoder
now disables the filter's per-frame checksum and pixel statistics.
Paired tests of a generated 4K H.264 clip produced identical BGRA bytes
and timestamps across all 60 frames. [FFmpeg showinfo filter](https://ffmpeg.org/doxygen/9.0/vf__showinfo_8c_source.html)

In four paired decode-only runs using FFmpeg's null output, median CPU
time fell from 1,159 to 740 ms. The native app's first-frame timings
varied across three pairs. Median Stop-to-thumbnail time was 179 ms
before and 189 ms after. Median preview-to-editor time was 217 ms
before and 227 ms after. These results establish reduced decoder CPU
work for the fixture. They leave first-frame latency open.

### 2026-11-19 macOS FFmpeg raw-frame output queue

The GPUI FFmpeg decoder now uses one thread for writing raw output
frames. The flag applies to the output stream and leaves input
decoding on the existing thread policy. [FFmpeg option scoping](https://ffmpeg.org/ffmpeg.html) [Raw video encoder](https://ffmpeg.org/doxygen/9.0/libavcodec_2rawenc_8c_source.html)

Four paired 4K H.264 runs produced identical BGRA bytes and timestamps
for all 60 frames. Median time to the first two frames fell from
119 to 105 ms, and peak FFmpeg memory fell by 225 MiB. Four paired
native launches with the same generated clip reduced median thumbnail
readiness from 197 to 160 ms and median preview-to-editor frame time
from 224 to 206 ms. The native launches covered light and dark on the
built-in Retina display. [Per-run measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS video preview dismissal

A dismissing video preview now stops scheduling thumbnail decoding as
soon as its exit animation starts. It holds its last displayed image
and blur through the fade. If a decode is already running, it can finish
without replacing those images.

Four paired native GPUI runs used a generated 4K clip in light and dark
on the built-in display. From the Open Editor action, the thumbnail
decoder exited in a median 338 ms before the change and 356 ms after.
Median editor first-frame readiness was 429 ms before and 381 ms after.
Individual results varied, including a two-second decoder exit before
the change. This sample does not establish a consistent timing gain.
The regression test checks the ownership change directly and confirms
that dismissal retains the displayed images.
[Per-run measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS video thumbnail color conversion

The recording thumbnail now resizes the decoded BGRA pixels before
swapping channels for the existing thumbnail and blur pipeline.
The offscreen macOS Metal runs produced byte-identical PNGs before and
after for 4K video at idle, on hover and after pointer exit, in light and
dark at 2×. The focused pixel test compares video and screenshot
thumbnails and hover blur in landscape and portrait.

A resize-only benchmark cut the median from 25.59 to 22.55 ms for a
3840×2160 frame and from 27.07 to 23.41 ms for 2160×3840.
These timings measure the thumbnail conversion stage.
[Per-size measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS all-in-one toolbar and instruction spacing

The Capture text tooltip overlapped the selection instruction pill in
both GPUI and the Electron reference. Both macOS shells now place the
pill at 128 points from the top, below the toolbar tooltips. The
Electron pill also uses the width of its contents up to the viewport
width. At 400×300 it fits the instruction on one line. The capture
target icon now uses the appearance's foreground color, restoring
contrast on the light toolbar.

At 400×300 the Electron keyboard tooltip bottom was 121 points and
the instruction began at 128 points in both appearances. The GPUI
Metal fixture requires at least four points of clearance for Capture
text, Pick color and Close, including the narrow layout and the
selected Capture text toolbar. Both 950×520 editor profiles completed
at 2× in light and dark, with 200 rendered PNGs each. The five
Electron capture and all-in-one test files passed 130 tests. These
runs use generated surfaces and app-local input.
[GPUI dark capture](tests/native/evidence/macos-ui/all-in-one-tooltip-clearance-gpui-dark-20261119.png)
[Electron light capture](tests/native/evidence/macos-ui/all-in-one-tooltip-clearance-electron-light-20261119.png)

### 2026-11-19 macOS native editor playback and raw-frame copies

Decoded frames now share the byte buffer that FFmpeg fills. This
removes a second allocation and pixel copy for each received frame.
The decoder retains the same BGRA bytes and timestamp selection.

At 30 fps, the native editor published 90 updates over three media
seconds for both 1080p and 4K in each appearance. In three paired 4K60
runs, both builds published 180 updates in that interval. The revised
build held a 640×360 interactive preview through the sample. The earlier
build stepped down to 432×243 or 480×270. A fourth pair stalled on both
builds, even at the smallest preview size. The bottleneck during those
stalls remains open. These counts measure preview updates delivered to
the editor, and do not measure display presentation.
[Per-run measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 Unix video decoder replacement during seeks

A large or backward seek can restart the GPUI FFmpeg decoder. The
old process used to stay alive until the replacement returned a
frame. GPUI now closes and reaps it before launching the next
process. The regression leaves a stand-in process running and
has the replacement verify the original PID has exited. It
also checks the replacement's exact BGRA output. All nine decoder
tests passed, including the development FFmpeg cases. The editor
suite passed 164 tests with five marked ignored.

In an isolated follow-up, reading 180 4K60 frames from FFmpeg
through a raw BGRA pipe took 3.55 to 21.56 seconds during periods
of host contention. This probe excludes GPUI's compositor and
display. It shows that raw frame delivery can fall below real time
before the editor receives frames. It does not identify the cause
of the intermittent playback stalls or establish that the seek fix
changes continuous playback.
[Raw pipe measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS video thumbnails outside the preview viewport

A recording preview continued requesting decoded frames while it
was completely outside a short, scrollable preview window. The
playback loop now uses the same card bounds as auto-dismiss.
A clipped card keeps its last image and skips frame requests.
Playback resumes when the card scrolls into view, and the card
still receives its full visible auto-dismiss delay.

The macOS regression renders a four-card stack in a 208×300
viewport. It reproduced image updates on the hidden video card
before the change, and checks that scrolling restores updates.
All 46 preview tests passed. The native Metal fixture also
rendered idle, hover and exit states for a 4K video at 2× in
light and dark.

A separate six-round 4K60 H.264 pipe benchmark measured median
time of 1,170 ms for 180 full-resolution frames. Scaling to
1920×1080 before transfer took 1,259 ms. FFmpeg direct buffering
took 1,157 ms and matched all output bytes and timestamps.
The decoder settings were kept. [Raw pipe benchmark](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS recording preview dismissal after loading

A recording preview used to start its auto-dismiss countdown before
the first frame arrived. The card now uses the shared loading
indicator while waiting. The countdown starts when the first frame
is applied or the initial decode attempt fails. A failed decoder
open clears loading and restores normal dismissal. Clipped cards
still receive their full countdown after becoming visible.

The first-frame regression failed on the old build before decoding
finished. It now verifies that the countdown has not started during
loading and that it is armed after the first frame, before the first
autoplay tick. The missing-project case verifies that a failed open
clears loading and arms the countdown. The existing image-atlas
checks cover retiring old images after playback and dismissal. All
46 preview tests passed. Before-and-after Metal renders of the 4K
thumbnail matched byte for byte at 2× in light and dark for idle,
hover and pointer exit.

### 2026-11-19 macOS keyboard focus in short video export settings

Tab moved focus from Compression to Frame Rate while the latter
remained hidden behind the export footer. The macOS video editor
now reuses its focus-scrolling behavior on the vertical Export
Settings pane. The pane brings the newly focused field into view
and the footer stays in place.

The offscreen Metal regression reproduced the clipping at 950×520.
The fix passed the keyboard transition and footer checks at 950×520
in dark mode at 1× and 720×520 in light mode at 2×. Those profiles
rendered 201 and 176 PNGs. The initial Export Settings render at
950×520 was byte-identical.
In the Electron reference at 950×460, Tab scrolled the settings pane
21 pixels and brought the full Frame Rate row into view.
[Focused GPUI export field](tests/native/evidence/macos-ui/video-export-frame-rate-focused-20261119.png)

### 2026-11-19 macOS keyboard focus in scrollable video settings

In a 950×520 editor, Tab moved from the last wallpaper aspect-ratio
button to the Padding slider below the visible panel. The shared
video settings pane now uses the existing vertical focus reveal.
Focus is tracked separately for each tab. Keyboard focus can bring a
row into view. Mouse clicks keep the pane's current scroll position.

The regression failed on the old build with the Padding row's bottom
edge 67.5 points below the viewport. It now verifies the entire row
is visible and the Right key changes the Padding value. The existing
export regression still checks the fixed footer and focused
frame-rate row. The editor suite passed 165 tests with five marked
ignored. At 720×520 and 2×, all 176 PNGs per appearance matched the
previous build byte for byte.

### 2026-11-19 macOS video settings scroll when switching tabs

In a 950×520 editor, scrolling Wallpaper and then opening Audio left
the Audio pane 188.5 points below its top. The video editor now resets
the settings and export scroll offsets when the active tab changes.
Reselecting the current tab preserves its position.

The regression switches between two overflowing panes and also scrolls
Export, leaves it, and returns. The Electron fixture at 950×460 measured
a 37-pixel Export scroll and confirmed that returning from Wallpaper
opened Export at the top. The GPUI check uses a simulated viewport.
The editor suite passed 166 tests with five marked ignored. The 950×520
dark Metal profile matched all 201 previous PNGs byte for byte.
The 720×520 light Metal profile passed at 2× with 176 PNGs.

### 2026-11-19 Electron video sidebar accessible names on macOS

The ten icon-only video settings buttons now use their sidebar labels
as accessible names, matching GPUI. In a macOS Electron accessibility
tree built from the rendered component, all ten button names were
empty before the change and correctly exposed afterward. The rendered
markup differs only by the ten labels.

The focused renderer and sidebar tests passed all 50 cases, and
TypeScript passed. The full editor fixture timed out during navigation.
The accessibility-tree evidence covers the isolated component.

### 2026-11-19 macOS scroll capture progress while finishing and canceling

The scroll control bar showed a dimmed checkmark while the daemon
was saving. The pending Done or Cancel control now shows a spinner
in the same space. A hovered tooltip updates to the pending label
without requiring pointer movement. The controls keep their existing
disabled and cancellation behavior.

The offscreen Metal fixture holds the Finish and Cancel replies
while rendering the toolbar. It checks for animation within the
active control and checks that the toolbar retains its dimensions.
The old finishing frames matched byte for byte across two phases.

Both light and dark Metal scroll workflows passed on the built-in
Retina display's 2× scale, including selection, save, retry and
cancellation cases. Comparing the old and new dark finishing
frames found pixel changes only in the Done icon region. All
17 focused tests passed, including tooltip updates while hovered.
[Dark finishing control](tests/native/evidence/macos-ui/scroll-control-finishing-dark-20261119.png)
[Light canceling control](tests/native/evidence/macos-ui/scroll-control-canceling-light-20261119.png)

### 2026-11-19 macOS Auto-scroll icon contrast

The scroll control bar hard-coded white play and stop glyphs. At 1×
in the light Metal fixture, Stop Auto-scroll had 1.14:1 contrast
against the toolbar. The macOS glyphs now use the theme foreground
at the same opacity, matching the Electron toolbar treatment.

The regression samples the painted glyph and nearby toolbar surface
in each state, with a minimum contrast of 3:1. The light fixture
measured 10.13:1 for Start and 9.99:1 for Stop. The dark fixture
measured 13.56:1 and 13.43:1. The light comparison changed only
the Auto-scroll icon pixels. Both full scroll workflows passed.
[Light Auto-scroll control](tests/native/evidence/macos-ui/scroll-control-auto-scroll-contrast-light-20261119.png)

### 2026-11-19 macOS primary recording control contrast

The recording-control Metal fixture now measures the colored Start
and Stop glyphs against the surrounding button pixels. It checks
Start in area setup and Stop during active and paused window
recording. The device icons retain their existing contrast and
clipping checks. Device menus use generated data from the fixture
daemon.

Four macOS profiles passed at 1× and 2×. Both scales measured
3.87:1 for Start and 3.10:1 for Stop in light mode, and 6.88:1
for Start and 5.38:1 for Stop in dark mode. Stop retained the
same contrast while paused. All 36 before-and-after PNGs at 2×
matched byte for byte. These runs cover fixture device menus and
toolbar rendering. Physical input and media permissions were
outside the run.

### 2026-11-19 macOS full regression checks

The full GPUI run on `b0fefc2f` found a stale assertion for the
all-in-one selection hint. The Rust and Electron implementations
already place the macOS hint 128 points from the top. The routing
assertion now uses the same platform constant whose value the test
checks separately. The rerun passed 1,178 tests with 23 ignored.

The repository checks passed on the same source, including 2,414
Vitest cases with three skipped, contract generation, TypeScript,
lint, formatting and React Compiler. The Windows-native check
skipped on macOS. The arm64 AppKit fixtures passed for scroll
preview and protected tooltip lifecycle, timer lifecycle and
accessibility, recording outline cleanup, and recording target
closure. Tall-tooltip placement and pointer exit while a filter
update is held also passed under Rosetta.

Only the built-in 1728×1117 display was online. The optional
ScreenCaptureKit exclusion probe was outside this run. The camera
fixture uses a stand-in panel. Physical mixed-DPI transitions,
live camera and physical input remain separate checks.

### 2026-11-19 live macOS area capture at `42ff7eab`

The current GPUI development app used the previously authorized
screen-only daemon fixture on the built-in Retina display. The
probe used a bounded generated AppKit panel and app-local input.
All six 600×440 PNGs in the final timing run matched the generated
reference in every pixel. Each capture preview opened an editor.

The warm median area-to-window switch was 129 ms in light and
54 ms in dark. Cancel-to-hidden was 36 and 23 ms. Confirmation to
thumbnail readiness was 217 and 249 ms, and opening the editor
from the preview was 132 and 66 ms. Window timings come from
WindowServer visibility. The check covers live area confirmation
and cancellation of the window selector at this revision.

The first attempt used a location partly covered by another
window. WindowServer confirmed that obstruction. The repeat used
an unobstructed location and a persistent stacking check. The
per-capture guard took 4–20 ms on that repeat. An intermediate
run reached 2.36 seconds to thumbnail readiness with a fresh
guard process for each capture. The cause of that tail remains
unclassified.
[Per-run timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS screenshot history persistence and responsiveness

Screenshot finalization read, wrote and pruned history on the app
thread. The native smoke probe held a history read in an isolated
profile. The previous build stopped answering preview status
requests until that read was released.

History persistence now runs on the background executor. Screenshot
finalizations enter the history writer in order, and each preview
opens after its entry has been processed. The regression accepts
another capture while the first history read is held, then verifies
the history order and preview readiness after release.

The probe uses generated pixels from a fixture daemon and app-local
commands. Three revised runs answered preview status in 2.1, 21.1
and 4.7 ms while history remained held. The older build missed the
regression's four-second response window. This reproduces a UI
stall during storage I/O. The earlier 2.36-second live thumbnail
result remains unclassified. The full GPUI suite passed 1,178
tests with 23 ignored.

```sh
cargo build --offline --locked --manifest-path src/main/Cargo.toml -p poratake-gpui --features parity
python3 tests/native/macos-capture-history-probe.py
```

### 2026-11-19 macOS recording history and project handoff

Recording Stop could block the app thread after the recorder
confirmed completion. The native regression held a history read
with a generated clip and reproduced the missing status reply.

Initial recording project state and cursor-based auto-zoom
preparation now run on the background executor. Recordings and
screenshots share one ordered history queue. The recording preview
or editor opens after project preparation and history persistence.

The fixture starts another screenshot while the recording history
read is held. It checks that the recording state file is already
valid, that both paths persist in newest-first order, and that the
recording preview becomes ready after release. It uses a fixture
daemon and app-local commands. In three runs the app answered
during the hold in 0.3, 0.6 and 4.8 ms. The screenshot-only
regression passed with the shared queue.

The arm64 GPUI suite passed 1,178 tests with 23 ignored. Both
light and dark Metal recording workflows passed the area, screen
and window preview-to-decoded-export checks.

```sh
python3 tests/native/macos-capture-history-probe.py --recording
```

### 2026-11-19 macOS recording discard cleanup

Discard closed the recording toolbar, then recursively removed the
project on the app thread. A removal failure left the recording
in place without feedback. Cleanup now runs on the background
executor after the toolbar closes. If removal fails, the app reports
the project path and the filesystem error.

The regression makes an isolated recording project read-only before
the fixture daemon releases a held Stop request. It reproduced the
missing warning before the fix. The revised case confirms the toolbar
closes, the warning names the retained folder, and the source movie
still contains the original bytes. Existing cases verify successful
discard removes the project and the retry path keeps its recording.
Both light and dark Metal recording workflows passed, including
preview-to-decoded-export for screen, area and window. The arm64
GPUI suite passed 1,178 tests with 23 ignored.

### 2026-11-19 macOS recording project setup

Recording Start created the project directory on the app thread.
A creation failure was only logged. On macOS the start preparation
now creates the project on a background worker while the toolbar
retains its pending state. If the toolbar closes during preparation,
the worker attempts to remove its unused empty project. A creation failure
reports the destination and filesystem error and returns the toolbar
to its retryable state.

The fixture points Start at a read-only directory. The original
build reported no warning. The revised test checks the warning,
the absence of a recorder Start request, and a successful retry
from the same toolbar after restoring write access. Both Metal
recording workflows passed in light and dark, including screen,
area and window preview-to-decoded-export checks. The arm64 GPUI
suite passed 1,178 tests with 23 ignored.

### 2026-11-19 macOS clipboard image editor opening

Open Clipboard in Editor previously decoded the clipboard image,
published the PNG and prepared the editor bitmap within the app
command. GPUI now reads the clipboard on the app thread, then
does the file and image work on the background executor. The editor
opens through the shared prepared-image path after publication.

The existing collision fixture now checks that the editor stays
closed in the command's UI turn. It failed on the old code. The
same fixture checks that the previous file survives, the clipboard
image receives an exclusive filename, and the saved PNG and
editor export retain the generated pixels. The fixture clipboard
is isolated from the system clipboard. Both full 1280-point
macOS Metal profiles passed at 1×, with 221 PNGs per appearance.
The arm64 GPUI suite passed 1,178 tests with 23 ignored.

### 2026-11-19 macOS screenshot destination preparation

The area, window and scroll paths checked the configured screenshot
directory on the app thread. They now snapshot the naming settings
and filename at confirmation, then resolve the directory on the
capture worker. Clipboard opening uses the same destination helper.
The existing fallback to Pictures is retained.

The fixture points an area capture at a read-only directory. The
old error omitted the destination. The revised regression checks
that the warning includes the folder, that preparation failure
sends no capture request, and that retry saves the expected pixels
after write access is restored.

The focused macOS selection workflow is available with
`PORATAKE_PARITY_SELECTION_ONLY=1`. Its light and dark runs
passed the area, window, display, preview, editor and clipboard
checks. The dark scroll workflow passed, including preview,
save, cancellation and retry. The arm64 GPUI suite passed
1,178 tests with 23 ignored.

### 2026-11-19 macOS screenshot editor history loading

Opening a screenshot preview prepared its bitmap off the app thread
but still read saved edits during window construction. The native
probe held the history read and reproduced an unresponsive app.
GPUI now prepares the bitmap and loads saved edits together on the
background executor. The preview, file picker and History openings
use that preparation path.

History keeps its window available while preparation runs. It closes
after a successful open, leaves the row available after failure, and
ignores a repeat open while one is pending. A focused regression
checks the failed-open retry, exactly one editor, and matching
export pixels.

The native held-history probe uses the isolated fixture daemon.
It checks that the app responds while editor history is held and
that the editor opens after the read is released. Three revised runs
answered in about 0.1 ms while history was held. The original build
missed the four-second response window. Both selection profiles
passed in light and dark. The arm64 GPUI suite passed 1,179 tests
with 23 ignored.

```sh
python3 tests/native/macos-capture-history-probe.py --editor-open
```

### 2026-11-19 macOS Recent Captures loading

Opening Recent Captures used to read the history file on the app
thread before showing the panel. The read now runs on the background
executor. The panel shows a loading indicator until the entries
arrive, then switches to the existing grid, list or empty state.

The native probe holds the history read in an isolated profile.
The old build missed the status reply. The regression checks that
the app responds, the panel is present while history is pending,
and the entries appear after release. The light and dark runs
answered in 0.3 and 0.2 ms during the hold. The screenshot,
recording and editor history probes also passed. The arm64 GPUI
suite passed 1,179 tests with 23 ignored, including the narrow
loading-state check. The light and dark Metal profiles passed the
140-, 200- and 240-point History grid, list, tooltip and isolated
deletion checks at 2×.

```sh
python3 tests/native/macos-capture-history-probe.py --history-open
```

### 2026-11-19 macOS History deletion and clearing

Delete and Clear History previously read history, removed files and
updated history on the app thread. Both now run on the background
executor. The panel retains its rows and shows progress until it
receives the result. It updates the rows on completion and reports
the existing filesystem and history errors.

The isolated native probes hold the history read for each action.
The old build missed both four-second status deadlines. The revised
app replied in 0.4 ms for Delete and 0.7 ms for Clear during the hold.
Both actions removed the file and its entry after release. The arm64
GPUI suite passed 1,179 tests with 23 ignored. Light and dark Metal
History checks passed at 2× for the 140-, 200- and 240-point panels.

```sh
python3 tests/native/macos-capture-history-probe.py --history-delete
python3 tests/native/macos-capture-history-probe.py --history-clear
```

### 2026-11-19 macOS capture preview deletion responsiveness

Deleting a screenshot or recording from its preview now performs the
history read, file removal and history update on the background executor.
The preview shows a spinner while deletion is pending. If source removal
fails, the preview remains available and its controls allow a retry.

The isolated screenshot probe held the history read open. The old build
missed the four-second status deadline. Across four light and dark trials, the fixed build replied in 0.1–2.3 ms
while the read was still held. Releasing the
read removed the file and its history entry. Making the capture directory
read-only kept both the source and its preview in place. Restoring write
access let the same preview delete the file.

The arm64 GPUI suite passed 1,179 tests with 23 ignored. Both light and
dark 950-point Metal profiles passed at 2×, covering preview placement,
stacking, editor handoff, thumbnail pixels and automatic dismissal.

```sh
python3 tests/native/macos-capture-history-probe.py --preview-delete
```

### 2026-11-19 macOS screenshot preview copy responsiveness

Copy and Polish now read and decode the capture, compose the wallpaper
when requested, and encode the sRGB PNG on the background executor.
The preview shows a spinner while it prepares the image. The UI publishes
the finished PNG to the pasteboard and dismisses the preview. If preparation
fails, the preview remains available for retry.

The isolated macOS probe answered status in 0.2 ms in light mode and
1.0 ms in dark mode while the source read was held. Supplying invalid
image bytes to that read left the original file and preview available.
The native probe used a failed copy and left the system pasteboard
untouched. A GPUI test published a generated image to the test clipboard,
checked every pixel, and verified that the preview started dismissing.
The arm64 GPUI suite passed 1,180 tests with 23 ignored.

```sh
python3 tests/native/macos-capture-history-probe.py --preview-copy
```

### 2026-11-19 macOS screenshot preview handoff before history persistence

GPUI now starts thumbnail preparation as soon as screenshot output is
saved, matching Electron's earlier preview handoff. The preview shows a
saving label on hover while finalization is in progress. Actions that
depend on the history entry or automatic clipboard publication become
available when finalization completes. The card can be closed meanwhile.
Automatic clipboard reads also run in the background. A superseded
automatic copy cannot overwrite the newest requested capture.

With the history read held, the old build reached the probe's 10-second
thumbnail deadline. Across the light and dark repeats, the revised app
reported a ready thumbnail within 16.7 ms of starting the hold check.
The probe confirmed that Delete remained gated during the hold, both
files reached history in newest-first order after release, and deleting
the newest card then removed only its own file and history entry. This
measures thumbnail readiness after the capture file was written. The earlier 2.36-second live capture tail remains unclassified.

```sh
python3 tests/native/macos-capture-history-probe.py
```

### 2026-11-19 macOS recording preview handoff before project finalization

After the recorder stops, GPUI starts decoding the preview directly
from the finalized video while it prepares the editor state and writes
the history entry. This matches Electron's earlier recording preview.
Edit, Delete and Copy become available once both project tasks finish.

The generated-recording macOS probe held the history read. The old
build reached its 10-second first-frame deadline. The revised build
decoded the first preview frame 141.4 ms after Stop in light mode and
118.5 ms in dark mode, with history still held. The probe confirmed
that Delete stayed gated, the generated two-second project state
contained a single segment, and a screenshot taken during the hold
was saved after the recording in history. The screenshot-only variant
also passed.

```sh
python3 tests/native/macos-capture-history-probe.py --recording
```

### 2026-11-19 macOS screenshot editor Save responsiveness

Save now opens the native destination panel before it composes and
encodes the screenshot. The editor snapshots the current image,
annotations, wallpaper and layers when Save is requested. After a
destination is confirmed, the background task composes and writes the
PNG or JPEG. Cancel exits before rendering the output. JPEG encoding
uses the same pixels and quality as the previous PNG round trip.

The native probe opened the panel in 660 ms on this trial and
canceled it without creating a file. It then used the production
background writer directly and reopened the 640×360 PNG. The saved
and reopened pixels matched the annotated editor, including all
1,790 changed pixels. A physical click on the native Save button
remains unverified. The GPUI suite passed 1,181 tests with 23 ignored,
including byte equality for JPEG output against the former encode path.

The direct-to-editor capture route still waits for the history entry.
Editor annotation persistence writes to that entry on debounce and
close, so opening before history completes needs coordination with
those writes.

```sh
python3 tests/native/macos-save-panel-probe.py
```

### 2026-11-19 macOS direct-to-editor image preparation

When Capture Preview is disabled, GPUI now decodes the newly saved
screenshot and prepares its editor image in parallel with history
persistence. After the entry is written, it opens the prepared image
with fresh annotation state. Reopening existing screenshots through
Preview or Recent Captures still restores their saved state.

The native probe held the history read while capturing a generated
3840×2160 PNG. Image preparation finished during the hold in light
and dark mode. The editor opened 52.7 and 54.4 ms after release, and
the matching history entry was present. These timings start at
history release. End-to-end timing on a real 4K capture remains open.

```sh
python3 tests/native/macos-capture-history-probe.py --direct-editor
```

### 2026-11-19 macOS screenshot annotation autosave responsiveness

The screenshot editor now writes debounced annotation state to Recent
Captures on the background executor. A later edit supersedes a queued
write, and a write already in progress finishes before the next revision.
The editor still flushes the most recent state synchronously on close
so a subsequent open can restore it immediately.

The native probe held the first history write with a FIFO. App status
replied in 0.2 ms in light mode and 0.1 ms in dark mode. The editor
accepted a second annotation while the write was held, saved both
after release, and flushed a third on immediate close. The screenshot
editor opening probe passed. The GPUI suite passed 1,181 tests with
23 ignored.

```sh
python3 tests/native/macos-capture-history-probe.py --annotation-save
```

### 2026-11-19 macOS screenshot editor Copy responsiveness

Copy snapshots the current editor composition and renders and encodes
it on the background executor. The toolbar shows progress until the
app publishes the PNG to the clipboard. A later Copy from another
screenshot editor supersedes an earlier request that has yet to publish.
The close-on-copy setting still closes the window after the PNG is
published.

The regression uses GPUI's test clipboard. It checks that Copy returns
before publication, that a later wallpaper edit does not affect the
copied PNG, and that close-on-copy only closes after publication.

### 2026-11-19 macOS screenshot editor Print responsiveness

Print snapshots the current editor composition. The background
executor now renders the image, encodes the PNG and base64 payload,
and calls the daemon. The existing Print shortcut and daemon contract
are preserved.

The native probe opened a generated 3840×2160 screenshot and added
an annotation. The Print command returned in 1.1 ms in light mode and
0.4 ms in dark mode. In both cases the fixture daemon received a PNG
that matched the editor export byte for byte. The real macOS print
dialog and printer output remain unverified. The 4K direct-to-editor
probe and all 1,182 GPUI tests passed.

```sh
python3 tests/native/macos-capture-history-probe.py --editor-print
```

### 2026-11-19 macOS screenshot editor Cloud Upload responsiveness

Cloud Upload captures the editor state at invocation and composes and
encodes the PNG on the background executor. It passes the rendered
bytes directly to the existing provider code, eliminating the temporary
file and keeping the filename, MIME type and clipboard behavior.

The loopback REST regression verified the exact PNG from the requested
composition after a later editor change. It also verified the returned
URL on GPUI's test clipboard. The test used a local endpoint and did
not exercise the user's cloud configuration. The GPUI suite passed
1,183 tests with 23 ignored.

### 2026-11-19 macOS screenshot editor Pin responsiveness

Pin prepares the selected composition on the background executor and
passes the pixels directly to GPUI. The toolbar shows progress during
preparation. The editor closes after the pin opens successfully and
passes its latest annotation, wallpaper and layer state to the pin.
Closing an editor-backed pin prepares the original screenshot off
the UI thread before restoring that state. Closing a standalone pin
dismisses it.

The focused light and dark Metal profiles verified standalone pin
layout, hover, resizing and close. In both themes, the annotated
pin matched the editor export within two channel levels. Closing
an editor-backed pin restored the annotations and the exact PNG
export, with and without a history entry. All 1,183 GPUI tests passed.

```sh
PORATAKE_PARITY_PIN_ONLY=1 PORATAKE_PARITY_WIDTH=1280 PORATAKE_PARITY_SCALE=1 src/main/target/debug/poratake-gpui --parity
```

### 2026-11-19 macOS standalone pin file loading

The tray picker and the development pin command both use the same
background image loader. It reads the file, decodes the color-managed
image and prepares its pixels before the app opens the pin window.
Read and decode failures now display a Pin failed notification.

With the selected PNG read deliberately held, the native probe
received app status in 0.8 ms in light mode and 0.4 ms in dark mode.
The pin opened after the read completed. The focused light and dark
Metal pin workflows passed, including standalone layout and close
and the editor-backed round trip.

```sh
python3 tests/native/macos-capture-history-probe.py --standalone-pin
```

### 2026-11-19 live macOS area capture latency at `eb4c8fa3`

A preflight-granted screen-only daemon captured a generated AppKit
panel under app-local input. The probe sampled WindowServer to wait
for Poratake's selector to clear the capture region and rejected
overlap from other apps. All 24 600×440 PNGs matched the generated
panel within two channel levels. Recent Captures was enabled in
an isolated profile.

With a persistent stacking checker, warm median area selector
appearance took 22 ms in light mode and 24 ms in dark mode, and
confirmed selectors disappeared in 17 and 20 ms. Warm median
confirmation-to-thumbnail readiness took 114 and 119 ms. Of that
interval, the capture RPC took about 83 ms and thumbnail readiness
followed its reply by about 17 ms. Opening the editor from the preview
took a warm median of 18 ms in each theme. This ends at editor window
availability.

Starting a fresh checker increased the warm median guard interval
from 14–18 ms to 69–70 ms and the warm median thumbnail interval
to 167–170 ms. The slowest of the 24 captures reached thumbnail
readiness in 253 ms. The previous 2.36-second outlier did not recur.
Its cause remains unclassified in the earlier run.
[Per-trial timing and probe details](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 live macOS Retina window capture and editor latency

On the built-in Retina display, the GPUI development app selected a
generated nonactivating window using app-local input. The broker
filtered the native window list to the generated target and forwarded
capture requests only for that ID. Freeze was off. All 20 saved
2960×1740 PNGs matched the generated colors within two sRGB levels.
The direct-to-editor runs also verified the matching history entry.

With Preview enabled, warm median area-to-window switching took
54 ms in light mode and 55 ms in dark mode. Cancelling the window
picker took 18 and 17 ms. Picking the window to thumbnail readiness
took 181 and 167 ms. The capture RPC accounted for 110 and 106 ms,
and the thumbnail became ready about 71 and 65 ms after its reply.
Preview-to-editor window readiness took a warm median of 73 and
71 ms, with one light trial taking 347 ms.

With Preview disabled, warm median window pick to editor window
readiness took 188 ms in light mode and 182 ms in dark mode. The
cold runs took 364 and 298 ms. First composited editor frames,
physical input latency and 4K-resolution capture remain open.
[Per-trial timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS editor window content timing with a 4K source

The development app opened an editor for a generated 3840×2160
PNG in both light and dark mode. WindowServer identified the
editor window, then the preflight-granted screen fixture sampled
that window by ID. The first sample contained the generated image
in all 20 trials.

With Preview enabled, warm median editor availability was 64 ms in
light mode and 63 ms in dark mode. The first verified sample returned
by a warm median of 149 and 153 ms from the preview edit action.
With Preview disabled, the corresponding times from confirmation
were 68 ms in both themes for availability and 157 and 155 ms for
the verified sample. The sampling RPC accounts for about 85–87 ms
of those warm upper bounds. The exact first composited frame time
and physical input latency remain open.
[Per-trial timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K60 playback stall phase trace

The native development probe can start editor playback and trace the
decoded-frame wait and the work through each preview update. The probe
uses generated video and a fixture recorder. The trace is opt-in via
`PORATAKE_NATIVE_SMOKE_VIDEO_PLAYBACK=1` in parity builds.

The earlier stall recurred. In the first trial one update took 1,544 ms,
including 1,520 ms waiting for the decoded frame. Across a further
six trials, some three-second samples contained only one update,
while the best unmodified decoder sample contained 178. During
the third paired comparison, the unmodified decoder delivered 178
updates and a two-input-thread cap delivered 149. The cap also
stalled in other trials, so the production decoder settings were
kept. The trace identifies decoder delivery as the largest measured
part of the stalled updates. FFmpeg decode, raw pipe delivery and
host scheduling within that wait remain open. All 1,183 GPUI tests passed.
[Per-run timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K60 decoder timestamp and payload timing

The opt-in playback trace now records the FFmpeg process ID, frame
timestamp, wait for the timestamp channel and time to read the
pixel payload. Four unthrottled 4K60 runs delivered 176–180
preview updates in the three-second measurement interval. The
largest completed decoded-frame wait in that interval was 24 ms.

Two controlled runs exercised different parts of the trace.
Pacing FFmpeg input at 0.6× made timestamp wait dominate a 686 ms
request. Pacing the raw payload relay with an added 20 ms per frame
made payload acquisition dominate a 1,299 ms request. This
distinguishes timestamp and payload stalls in a future recurrence.
The unthrottled tests did not reproduce the earlier long stall.
[Per-run timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS reuse of decoded video preview frames

Nearby playhead positions can resolve to the same decoded frame. The
video editor now reuses that frame's converted pixmap for both screen
and camera tracks. The cache uses decoded buffer identity and dimensions,
while the decoder still selects the frame for each new time. A weak
reference lets the decoder release the raw bytes when it moves on.

The regression checks that nearby times sharing a decoded buffer reuse
the same pixmap, a new buffer produces distinct pixels, and the cache
holds no extra strong reference to the decoded data. The optimization
removes repeat conversion when frames are shared, including a 31.6 MiB
pixmap allocation for each reused 3840×2160 frame. All 1,184 GPUI tests
passed. The development FFmpeg camera visibility test and both narrow
2× Metal profiles passed. The intermittent 4K60 decoder stall remains
a separate open issue.

### 2026-11-19 macOS video preview during a missing screen frame

When a screen frame is unavailable, the video editor skips that
preview update and keeps the last valid image in place. This avoids
replacing the video with a background-only composition during the
dropout. Intro stills can render independently of the screen frame.
The regression covers the intro, missed updates at full and reduced
preview sizes, and recovery when screen pixels become available.
All 1,185 GPUI tests passed, along with the real-FFmpeg camera visibility
test and both narrow 2× Metal profiles. Decoder stalls can still delay
new frames.

### 2026-11-19 macOS preview quality during slow decoding

The editor now tracks decode wait separately from the rest of preview
work. If decoding alone exhausts the quality budget, the resolution
drops only when the remaining preview work also exceeds that budget.
When decoding leaves time in the budget, the existing total-time rule
still applies. Raising resolution still requires a run of fast updates.
A missed frame resets that run.

The regression covers a long decode with fast composition, a long
decode with expensive composition, and a mixed cost that can benefit
from downscaling. All 1,186 GPUI tests passed. In four native
development runs with a deliberately paced 1080p60 input, the lowest
observed preview size rose from 384×216 in each old run to 480×270
in light and 432×243 in dark. The old and new runs each delivered
23–26 updates across the three-second sample. This verifies quality
selection under controlled decode delay. The intermittent 4K60
stall cause remains open.
[Per-run timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS video editor intro-still handoff

The editor composes an enabled intro still before requesting recording
or camera frames. Both preview sizes use the same composition engine
as before. Recording decode begins when the playhead reaches the
recording. The regression verifies the intro pixels, preserves the
screen-frame cache during the intro, and checks that a real FFmpeg
source spends zero time decoding for the intro request.

A native development probe used a generated recording and deliberately
held the first FFmpeg frame request. The old build had no editor
preview while the decode was held. The revised build published the
intro preview in 120 ms in light mode and 208 ms in dark mode while
the hold remained active. Starting playback reached the decoder,
and releasing the hold produced the recording frame. Timings end
at preview-image availability. All 1,186 GPUI tests and the
separate development FFmpeg camera visibility test passed.
[Timing details](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS screen preview during a camera gap

The editor keeps the camera decoder warm during a gap by submitting
at most one asynchronous request. The camera worker reports completion
without retaining a raw frame in the preview. Screen composition
continues while that request runs, and visible camera ranges still use
the decoder and the shared composition engine.

In the native development probe, the old build had no editor preview
while the camera decode was held during a gap. The revised build
published the screen preview in 221 ms in light mode and 210 ms in
dark mode, and playback advanced while the hold was active. After
release, the decoder supplied a camera frame when the camera range
became visible. The real-FFmpeg camera visibility test and all
1,186 GPUI tests passed.
[Per-run timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS preview recovery after an empty FFmpeg stream

A decoder process that closed before delivering any frame used to
remain attached to the source, leaving subsequent requests empty.
The Unix decoder now retires that process and starts a replacement
on the next frame request. A decoder that has already delivered a valid
frame still supplies that final frame at end of stream.

The regression forces the first FFmpeg process to exit early and
verifies two launches and the replacement's exact BGRA bytes. In
native light and dark trials, a prerecorded camera track failed
its first decode. The old app advanced the screen past 0.5 seconds
with only the failed camera process. The revised app launched
a second process and decoded camera frames during preview playback.
All 1,187 GPUI tests passed.

### 2026-11-19 macOS video preview during a missing visible camera frame

After the editor has displayed a camera frame, a missing camera frame
in a visible range now leaves the last complete preview in place. The
cache retains its last valid frame through repeated misses. The screen
can appear before the first camera frame, and an intentional camera
gap still renders without a camera overlay.

The regression checks initial screen-only playback, camera pixels
on success, two consecutive misses at the reduced preview size,
camera recovery and the screen-only gap. In native light and dark
trials, a prerecorded camera decoded normally through one playback,
then raw camera decode failed on the replay. Both missed updates
left the previous preview available. Decoder delivery resumed
after release. The attempted baseline used the rebuilt binary and
was excluded. All 1,188 GPUI tests and the development FFmpeg
camera visibility test passed.

### 2026-11-19 current macOS recorded-window movement and closure

The GPUI build at `79094a68` and a screen-only fixture built from
the current Swift sources recorded a generated AppKit window in
light and dark. The same outline followed the move and resize,
disappeared while the target was hidden, and returned when the
target was restored. The hidden interval preserved the recording.

Releasing the window produced one TARGET_CLOSED event and one
automatic Stop in each run. Both video-only files decoded at the
original 720×480 size with red, green and blue stages. The resized
blue stage was centered with black padding on both sides, and
saved packet timestamps increased strictly. The toolbar and
outline disappeared, both previews decoded, and both projects
opened in the video editor.

Closure to Stop request was 284 ms in light and 281 ms in dark.
Closure to decoded preview took 390 and 386 ms, and preview-to-editor
frame readiness took 87 and 63 ms. The probe sampled window
visibility and decoded frame availability. Export and physical
pointer input were outside this run.
[Per-run evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 current live macOS window recording through export

The current GPUI app and a screen-only fixture built from current
Swift sources recorded a moving and resized generated window in
both appearances. Closing the target finalized each take and
opened a decoded recording preview. The app-local editor probe
split the timeline at 0.45 seconds and verified two saved video
segments. It then used the regular export worker with an isolated
destination to produce a 720×480 MP4.

At matching output-frame timestamps in each of the red, green
and blue stages, 99.697–99.767% of decoded export pixels were
within five RGB channel levels of the editor preview. A first
comparison sampled a different timestamp in the encoded stream
and exposed a mismatch around the moving cursor. The corrected
comparison used the exact output-frame index on both sides.
[Per-run evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 live macOS area recording excludes the toolbar and tooltip

The current GPUI app recorded an 840×194-pixel region of a generated
backdrop in both themes. The recording toolbar and the native Stop
recording tooltip were visible inside that region. The tooltip sat
four points below the toolbar. The screen-only fixture used an
owner-relative pointer position for the app-local hover. A green
marker appeared only while the tooltip was visible.

Both recordings contained seven marked frames. Every tested pixel
inside the toolbar and tooltip regions stayed within ten RGB levels
of the blue backdrop. An ordinary magenta window at the tooltip's
former bounds then appeared in seven frames per recording, verifying
that the region was sampled. Stop to decoded preview took 96 ms in
light mode and 98 ms in dark mode.

The editor and decoded MP4 export preserved the protected and
ordinary-window stages. At matched frame indices, 99.379% of all
pixels in each protected sample and 98.800% in each ordinary sample
were within five RGB levels of the preview. Every pixel outside an
eight-pixel band around the generated color edges met that tolerance.
The hover probe used app-local input.
[Native verification and sampling details](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS full-display editor and export verification

The live full-display attempt stopped before sending a capture
request. WindowServer reported two Notification Center windows in
front of the generated backdrop. The temporary backdrop and app
closed after the guard rejected the scene. Live full-display
capture remains open for the current build.

A separate app-local probe exercised the full-display request and
editor pipeline using a generated video. The recorder stub verified
an explicit 1728×1117-point display rectangle with no window ID and
provided a 3456×2234 video. The editor saved a split and exported
a video-only MP4 at the original size in both themes. For both
colors at matched output-frame indices, every decoded pixel was
within five RGB levels of the preview.

With the fixture daemon, Stop to decoded preview took 187 ms in
light and 143 ms in dark. Preview to the editor image took 228 and
229 ms. The full-resolution exports completed in 4.49 and 5.63 s.
These timings cover the editor handoff and export with fixture media.
[Per-run details](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 current live macOS timer screenshot and cancellation

The current GPUI build used the current native timer and a bounded
generated backdrop. The 140×52-point countdown was visible inside
the selected region. Its window and the selector were absent from
that region before the broker forwarded capture. All six completed
light and dark runs saved a uniform 600×200 PNG, and the previews
opened in the editor.

With a persistent WindowServer guard, the five-second countdown
completed in 4,971 ms in light mode and 5,004 ms in dark mode.
Completion-to-thumbnail readiness took 496 and 421 ms. The real
capture RPC accounted for 379 and 328 ms. The persistent guard
added 14 and 21 ms. A separate run with a new guard process per
capture measured 134–137 ms of fixture overhead.

The two cancellation runs held the native Show reply. GPUI answered
in 7.4 ms in light and 0.9 ms in dark, cleared the selector and native
countdown, and issued no capture request. The two Electron timer
test files passed 21 tests. Live timing measurements are from GPUI's
app-local input on the built-in Retina display.
[Per-run timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 current macOS fixed-header scroll stitching

The current Swift fixture captured three live frames of a generated
Retina page with a fixed 102-point header. The page accepted scripted
offsets while the fixture skipped system pointer input. The 208- and
266-point steps stitched to 920×1592 and 920×1824 PNGs. Each output
contained one header, preserved Display P3 and matched the expected
native rows within two RGB channel levels after converting both
sides to sRGB. All three magenta windows were present in the
unfiltered control capture and absent from the stitched outputs.

The native Swift fixture suite passed. The GPUI light and dark
scroll fixtures passed selection, preview, history, saved-image
reopen, cancellation and retry. The two Electron scroll test
files passed all 50 tests. Stitching remains open where different
scroll histories produce the same frames.
[Native fixture evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 current macOS tooltip typography at 2×

The current native tooltip was rendered over a bounded generated
backdrop in Aqua and Dark Aqua. The twelve composited captures
covered a single-line recording label, a wrapped English label,
explicit truncation, right-to-left Arabic, a long unbroken device
name, and emoji with combining marks. All panels maintained a
four-point gap below the generated owner.

| Text case                   | Panel size in points | Light ink margin top/bottom | Dark ink margin top/bottom |
| --------------------------- | -------------------- | --------------------------- | -------------------------- |
| Start recording             | 100×22               | 13/10 px                    | 13/10 px                   |
| Wrapped English             | 376×36               | 13/13 px                    | 13/13 px                   |
| Truncated explicit lines    | 94×64                | 14/13 px                    | 13/13 px                   |
| Arabic                      | 353×64               | 13/8 px                     | 13/8 px                    |
| Unbroken device name        | 375×64               | 13/13 px                    | 13/13 px                   |
| Emoji and combining accents | 213×22               | 8/8 px                      | 8/8 px                     |

The Arabic label wrapped right-to-left and truncated at the reading
end. The unbroken token wrapped and truncated on line four. The
emoji sequence and accents rendered intact. The measured margins
describe the visible ink within equal AppKit label insets. The probe
supplied the pointer position directly to the tooltip and checked
WindowServer isolation before capturing each panel. The Swift
fixture passed, including the rendered-label and tall-owner tests.
The five Electron recording control and preview files passed
132 tests.

### 2026-11-19 macOS video editor with a tall timeline in a short window

A saved 300-point timeline left the preview only 32 points tall in
a 640×440-point window. The two 16-point insets consumed that space.
The track pane now shrinks and scrolls to leave space for the preview
while the playback controls and ruler keep their heights. Enlarging
the window restores the saved timeline height. Dragging the resize
handle starts from the displayed height.

The regression reproduced the 32-point pane before the fix. It now
verifies at least 84 points for the preview, an in-bounds track pane,
and restoration of the saved 300-point timeline on a larger window.
Both 640×440-point Metal runs at 2× verified the rendered preview
colors and timeline label contrast with the maximum saved height,
one in light and one in dark. All 1,189 GPUI tests passed.

### 2026-11-19 macOS selector, preview and editor responsiveness

The GPUI build at `755052f4` ran six app-local trials per appearance
with a temporary profile on the built-in Retina display. WindowServer
sampling followed the selector replacement, cancellation, preview,
editor opening and cleanup. A fixture supplied a 3840×2160 PNG to
isolate the UI and image preparation cost from native capture time.
All 12 saved PNGs matched the fixture byte for byte.

| Transition                        | Warm median light | Warm median dark | Slowest of 12 |
| --------------------------------- | ----------------- | ---------------- | ------------- |
| Area selector appears             | 42 ms             | 50 ms            | 199 ms        |
| Area selector switches to window  | 45 ms             | 52 ms            | 144 ms        |
| Cancel hides the window selector  | 25 ms             | 26 ms            | 103 ms        |
| Confirm hides the area selector   | 28 ms             | 44 ms            | 78 ms         |
| Confirm to preview window         | 45 ms             | 63 ms            | 93 ms         |
| Confirm to 4K thumbnail readiness | 185 ms            | 167 ms           | 198 ms        |
| Preview action to editor window   | 165 ms            | 173 ms           | 234 ms        |

Earlier live runs measured warm thumbnail readiness at 114–119 ms
for a 600×440 area and 167–181 ms for a 2960×1740 window, including
native capture. The separate 4K editor sampling run found the expected
pixels by a warm median of 149–153 ms after the preview Edit action,
including the sampling RPC. These runs measure different stages and
use separate fixtures. First display presentation and physical input
latency remain open. The earlier intermittent 4K60 decoder stall
also remains open.
[Per-trial evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K60 frame delivery during playback stalls

GPUI played a generated 3840×2160 H.264 clip at 60 fps with the
development FFmpeg binary. Four of six initial runs delivered 180
preview updates in three seconds. The other two delivered 164 and 87. A second batch reproduced longer waits with both the software
and VideoToolbox decoders. In the slowest completed software
request, frame delivery took 1,121 ms. App-local command replies
still completed within 22 ms across both batches.

The VideoToolbox trial matched all 300 BGRA frame checksums and
timestamps from the software decoder. It reduced FFmpeg user CPU
time in the standalone comparison and still stalled in the app.
The default remains software decoding.

The opt-in trace now separates buffer allocation, the wait for
the first byte, and the rest of the payload read. Six unmodified
GPUI runs with this trace delivered 180 preview updates each.
Controlled 200 ms holds before and after the first byte appeared
in the expected trace fields. The earlier long stalls occurred
after timestamp delivery. Their cause within payload delivery
remains open. All 1,189 GPUI tests and both sparse-timeline
development FFmpeg tests passed.
[Per-run evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS video frame conversion and playback quality

Two alternative BGRA-to-RGBA conversions matched the existing
output byte for byte. In an isolated 4K test the existing conversion
took 7.08 ms, the single-pass mapping 5.09 ms, and the copy-and-swap
version 4.01 ms at the development optimization level.

The app-level results led to retaining the existing conversion.
At the same 384×216 playback preview size, the copy-and-swap
build took longer to compose, excluding measured decoder wait.
The single-pass converter showed lower composition costs in
several 4K60 runs, during a batch in which decoder stalls affected
both builds. All four 1080p60 runs applied 120 updates over two
seconds. The single-pass build spent most of that interval at a
lower preview size and its initial full-resolution update took
more work outside the decoder. Playback update counts, preview
resolution and first-frame cost are recorded separately.
[Per-run evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS recording tooltips across controls

The native GPUI probe dispatched unpressed mouse moves directly to
the recording window at 2× scale. One sequence in each appearance
visited Start recording, Select camera, Select microphone, and
Close, then exited. The fixture daemon used the owner frame sent by
GPUI and a synthetic anchor within that frame. The real AppKit
tooltip showed each requested label in the matching appearance
with a four-point gap. Exiting cleared the final panel.

The measured in-window replacements took 4.9–22.1 ms from the probe
command through observing the visible panel. The Swift fixture also
held a screenshot in flight and verified that only the newest label
appeared after release. The five Electron reference files passed 132
tests. Physical pointer delivery remains open.
[Native transition evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS frozen window screenshot corners

GPUI and Electron already sent the picked window ID in a cached
window capture. The Swift daemon used the frozen rectangle and
left desktop pixels in the rounded corners. The cached path now
uses the selected window's native alpha when the pixel dimensions
match. The saved colors still come from the frozen frame. A
missing or differently sized mask preserves the frozen crop.

A bounded AppKit fixture exercised the real GPUI picker, thumbnail,
history and editor in light and dark at 2×. It seeded the frozen
store with a red window over magenta and then changed the visible
target to green. WindowServer supplied the alpha while a
click-through cover obscured the target. The original 640×440 PNG
had opaque magenta corners. The revised PNG had transparent corners
and the same red center. The editor export preserved the alpha
exactly and differed by at most one premultiplied RGB value at
partially transparent edges. Pixels with full and zero alpha
matched exactly.

In these runs the additional mask read raised confirmation-to-thumbnail
time from 50–53 ms to 114–160 ms. The picker cleared before that
work finished. The reduced fixture did not time a full-display
freeze. Apple Silicon and Rosetta geometry tests, Swift daemon
typechecking, window-ID validation and 77 Electron reference tests
passed.
[Native frozen-window evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS frozen screenshot color and crop time

The frozen frame store used a Device RGB bitmap for every crop.
The new path uses the original image directly for an aligned crop
contained by one display. Composited crops keep a shared RGB
profile and use Display P3 when the source profiles differ. A
generated fixture verifies both mixed-scale and mixed-profile seams.

A native 2× capture of a bounded Display P3 test window verified
the change. The old crop changed the source green sample from
(31, 224, 56) to (104, 225, 67) after conversion back to the
source profile. The revised 600×400 PNG preserved Display P3 and
matched the source crop in every channel. The GPUI screenshot
workflow reopened that PNG in light and dark and exported exactly
the same sRGB pixels as ColorSync.

In an isolated 4K benchmark, median crop-plus-PNG time fell from
94 ms to 54 ms. This measures the Swift backend and in-memory
encoding. Full-display presentation and mixed physical displays
remain open. Geometry passed on Apple Silicon and Rosetta, along
with Swift typechecking, four GPUI color tests and 77 Electron
reference tests.
[Color and crop timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS frozen crops at fractional display seams

The Swift compositor drew each display at fractional destination
bounds. Two opaque source images could produce a translucent
column at the join and a translucent edge row. The compositor
now maps each intersection to the output pixel grid and rounds
shared boundaries consistently. The generated 1×/2× regression
also verifies that a gap between displays stays transparent.

The old 600×400 Display P3 fixture had 400 pixels at alpha 191.
The revised PNG contains 240,000 fully opaque pixels. In both
appearances the GPUI development app used the revised PNG in a
cached area workflow, opened the thumbnail and history entry, and
exported pixels matching the ColorSync reference exactly. The
capture request preceded release. Swift geometry checks passed
on Apple Silicon and Rosetta, and 77 Electron reference tests
passed. Physical mixed-DPI displays remain open.
[Native geometry and workflow evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Save sheet open and cancel timing

The screenshot probe opened and canceled the native Save sheet
three times per appearance. Each opening kept the isolated
directory and a separate PNG filename. Each cancel cleared Save
state and produced no file. The probe then saved an annotated
640×360 PNG through the background writer and reopened it.
All 1,790 changed pixels matched the editor composition in both
appearances. The probe removes its temporary profile on exit.

The first GPUI sheet opened in 1.04–1.09 seconds. Later opens took
511–589 ms. A standalone AppKit sheet took 1.19–1.31 seconds for
the first open and 505–659 ms on later opens. The AppKit cancel
call itself took 265–427 ms to return; GPUI cleared Save state
1–22 ms after its own cancel call returned. These are app-local
samples. The physical Save button and native file confirmation
remain outside this probe.
[Per-trial Save timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 isolated live macOS full-display recording and export

The GPUI development app requested the full 1728×1117-point
Retina display and supplied the recording control's exclusion ID.
A screen-only Swift fixture built from the current recorder used
a ScreenCaptureKit allowlist containing one generated window.
This exercised the actual display-resolution stream and video writer
without admitting other desktop windows. The fixture used the
existing Screen Recording grant.

Both 3456×2234 takes decoded with the generated red and green
phases. Samples outside the fixture bounds were black throughout.
The GPUI preview opened each take, and the editor saved a split
at 0.45 seconds before exporting an original-resolution MP4.
At exact output frame timestamps, 99.909–100% of pixels were within
five RGB levels of the editor preview. All four videos had strictly
increasing decoded timestamps.

Stop to decoded preview took 200–202 ms. The editor then reported
its first image in 196–220 ms, and exports took 3.27–4.27 seconds.
The earlier duration comparison used GPUI's busy status, which
includes startup before the first frame. The follow-up below times
from the first-frame callback and verifies the static tail.
A live full-display run with the regular exclusion filter remains
open. All 85 Electron reference tests passed.
[Live stream and export evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS recording duration after the first frame

The native recorder fixture timed from the first screen frame to
Stop using the host clock. Four 3456×2234 takes changed from red
to green and held the final color for 901–954 ms. In light and
dark, it also paused and resumed one take. After subtracting the
measured pause, the decoded durations matched elapsed recording
time within 11.2 ms, and the final decoded frame stayed green.

The first-frame callback arrived 439–476 ms after the request.
The earlier GPUI timing observed busy status, which can arrive
during that startup interval. The native smoke probe now exposes
a separate active-recording status. An isolated test held the
Start acknowledgement and verified that the two states differed
until recording became active.
[Clock and duration evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS large video frame transfer

The macOS decoder now transfers frames larger than 1 MiB over a
local socket pair with 1 MiB buffers. Small frames and buffer
setup failures use the existing pipe. The standalone 4K60
comparison verified identical bytes and timestamps for all
180 frames. Median transfer-and-decode time fell from 1,582 to
780 ms. A socket with its default buffers was substantially slower.

In eight alternating GPUI playback runs, median payload time fell
from 8.25–8.33 ms to 3.01–3.12 ms. Each run applied 120 preview
updates over two seconds. Both paths produced the same initial
editor snapshot and the same first-frame comparison against an
edited 3840×2160 export. The EOF regression, 1,190 GPUI tests and
both real-FFmpeg sparse seek tests passed. The earlier intermittent
stall remains open.
[Transfer and playback evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS playback during concurrent 4K encoding

The GPUI development builds alternated between the original pipe
and the tuned socket while a separate software encoder processed
the generated 4K60 clip. The pipe applied 2–7 preview updates
over two seconds and the socket applied 29–118. Median payload
times were 17.0–27.5 ms and 7.0–16.2 ms respectively. The longest
completed decoder request on the socket was 651 ms.

The app-local playback query answered within 19 ms across the
eight paired runs. The area selector appeared in 99–167 ms,
switched to window mode in 39–64 ms and cleared in 44–67 ms.
WindowServer observations were limited to the temporary app.

The standalone BGRA path also slowed with its output directed
to the null device, so frame production and host scheduling
remain relevant to the stall. The two- and four-filter-thread
caps varied too much across runs to select either. The playback
stall remains reproducible under concurrent encoding.
[Concurrent playback and diagnostic evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS VideoToolbox with the tuned frame socket

With the tuned socket in both GPUI runs, software decoding applied
85–120 updates per two-second interval without a competing test
job. VideoToolbox applied 7–41. With concurrent 4K encoding, both
showed stalls and neither had a consistent cadence advantage.
Both sparse-timeline seek tests passed with the hardware flag.

The generated 4K60 H.264 clip matched frame checksums and timestamps
through both decoders, including after seeking. The full-range
H.264 and 10-bit HEVC fixtures also matched. Rotated H.264 and
4:2:2 H.264 produced different RGB values with VideoToolbox.
The rotated first frame differed by more than five RGB levels in
13,141 pixels. Current GPUI software-decoded preview pixels matched
FFmpeg exactly for the rotated, 4:2:2, and 10-bit HEVC fixtures.
These results favor software as the default. The contention stall
remains open.
[Decoder and format evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS preview responds to a newer seek

An older preview request could wait on FFmpeg after a direct seek
had already queued a new playhead position. The editor now signals
cancellation when it invalidates that preview generation. The Unix
decoder wakes from its frame wait and keeps its buffered frames
available for the next request. The normal seek rules then decide
whether to reuse the stream or start from a new keyframe.

The native probe held the raw output for a 1.5-second seek, then
sought to 4.5 seconds. The original editor produced no frame in
the 1.5-second observation window. The revised editor applied the
second seek in 92 ms in light and 90 ms in dark, while the old
output was still held. Both preview images matched the FFmpeg
reference at 4.5 seconds in every RGB channel. Four ordinary 4K60
playback runs applied 120 images each over two seconds.
[Seek and playback evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS camera warmup during a distant seek

The prerecorded camera warmup could hold the camera decoder after
a seek moved to a visible interval. A request beyond the existing
decoder seek window now cancels the old warmup. A decoder awaiting
its first frame can use the start of its current stream to decide
whether to reposition. Nearby requests can continue using the
warmup.

In a native probe the original editor produced no new frame within
1.5 seconds of the distant seek while camera output was held. The
revised editor applied the camera overlay in 149 ms in light and
143 ms in dark with the hold still active. A nearby seek used the
same camera process. All 1,191 GPUI tests and four development
FFmpeg preview, seek and export tests passed. Live camera hardware
was outside this fixture.
[Camera seek evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS framed attachments at fractional zoom

The existing Metal fixture now compares the framed attachment
preview with its flattened PNG reopened in a separate editor.
It uses balanced dark frames and translucent light frames, each
with a top attachment, a right attachment, and a rectangle moved
through the editor. The original and reopened views are measured
using their own snapped image bounds.

All 24 combinations passed at 73% and 137% zoom, simulated
100%, 125%, and 200% display scale, and light and dark appearance.
The largest difference was one channel level. The 13 Electron
wallpaper, frame and annotation export tests passed. Physical
display transitions remain outside the fixture.
[Framed preview evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS narrow frame lights and attachment insets

A narrow right attachment cropped its green title-bar light in
both GPUI and Electron. Both renderers now scale the group to the
available frame width while keeping it vertically centered.
An Electron regression reproduced a light extending to x=59 in a
40-pixel frame. The revised Rust and Electron tests cover all
three lights in that width.

The attachment fixture now also covers a six-point inset and
seven-point spacing. The 24 GPUI Metal preview/reopen comparisons
at 73% and 137% zoom differed by at most one channel level.
Electron 43.4.0 emitted six offscreen PNGs through its export hook.
Their dimensions, three content anchors, right inset and clear
spacing matched GPUI. In the sampled 125% export, all changed
pixels were inside the narrow title bar.
[Frame and attachment evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS framed aspect-ratio previews at fractional zoom

The framed backdrop now fills the measured stage. The earlier
image-fit pass could resize it again after stage snapping. A
balanced framed attachment with a 9:16 canvas at 137% zoom and
simulated 200% display scale differed from a reopened PNG in
7,483 pixels by more than two channel levels. The largest channel
difference was 110.

The revised fixture adds 1:1 and 9:16 canvases to the framed
attachment inset and spacing matrix. All 48 Metal comparisons
at 73% and 137% zoom in both app appearances and simulated 100%,
125% and 200% scale matched the reopened PNG within one channel
level. The 14 Electron reference tests passed.
[Aspect-ratio preview evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS centering for framed attachment exports

Framed attachments now center against the final pixel dimensions
of the canvas. The balanced 273×273 square example previously
measured 24 and 22 pixels at its sides. It now measures 23 on
each side, matching Electron. Annotation patches use the same
composition pixel grid at half-pixel offsets.

All 48 Metal preview/reopen comparisons passed in both themes
with at most one channel level of difference. Twelve offscreen
Electron exports matched GPUI's dimensions and sampled content
positions. The six square cases matched the sampled margins.
Four portrait samples still differ by one edge pixel between
renderers.
[Centering and annotation evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS capture UI latency recheck

The GPUI development build at `9e2e19e5` ran six app-local trials per
appearance on the built-in Retina display of an Apple M5 Pro. Each
appearance started with a fresh process and temporary profile. A
daemon fixture supplied one window target and a 3840×2160 PNG with
Freeze off. All 12 saved PNGs matched the supplied bytes.

WindowServer observations include polling time. Thumbnail readiness
comes from the app. Each cold result is the first trial in that
appearance. The warm results are medians of trials two through six.

| Transition (ms)               | Light cold | Light warm | Dark cold | Dark warm | Slowest of 12 |
| ----------------------------- | ---------: | ---------: | --------: | --------: | ------------: |
| Area overlay appears          |       77.1 |       18.6 |     130.1 |      36.7 |         130.1 |
| Area switches to window       |       32.6 |       26.3 |      53.6 |      34.1 |          53.6 |
| Cancel hides overlay          |        8.2 |       16.7 |      20.2 |      15.6 |          24.8 |
| Area overlay reopens          |       20.5 |       20.8 |      39.1 |      34.2 |          75.4 |
| Confirm hides overlay         |       20.9 |       20.1 |      12.0 |      16.4 |          28.5 |
| Confirm to preview window     |       20.9 |       21.0 |      32.1 |      31.1 |          51.1 |
| Confirm to 4K thumbnail ready |       61.4 |       62.2 |      97.4 |      92.6 |         107.4 |
| Preview Edit to editor window |      107.7 |       63.3 |      99.5 |      96.8 |         125.9 |

The probe measures application and WindowServer readiness under
fixture capture. Live ScreenCaptureKit trials are recorded separately
in the area and Retina window sections. Physical input-to-display
latency and consistent 4K60 playback under encoding load remain open.
[Per-trial timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS video frame socket under encoding load

For large decoded frames, the macOS transport now requests a send
buffer up to 4 MiB, capped at the frame size. The receive buffer
uses 1 MiB. If macOS denies the larger size, the sender retries at
1 MiB before falling back to a pipe.

Eight paired trials alternated 1 and 4 MiB sends with two concurrent
4K encoders. The median of each trial's longest payload read fell
from 206 to 97 ms. Seven pairs improved on that measure. Applied
preview updates increased or held steady in all eight pairs, with
medians of two and four out of 120. Some runs still applied only
one update. In the slowest 1 MiB payload sample, the remainder after
the first byte took 382 ms and allocation took less than 1 ms.
FFmpeg's direct output mode also stalled and the default stays buffered.

The compiled app reported the expected socket sizes in both themes.
Its two loaded pairs were mixed, with 19 to three applied updates
in light and one to seven in dark. Editor preview PNGs matched byte for byte between builds. All 120
decoded export frames and their timestamps matched in each theme
after splitting the 4K recording. The raw transport also preserved
all 60 sampled frame hashes and timestamps across startup and a
seek. All 1,192 GPUI tests and three sparse timeline FFmpeg tests
passed. Sustained 4K60 playback under contention remains open.
[Per-trial timing and frame evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS reuse of raw video frame storage

The FFmpeg frame reader reuses a decoded byte buffer after the
decoder advances past the frame and releases its final strong
reference. Each stream retains at most one spare buffer. Reusing
its bytes gives the new frame a new Arc identity, so the editor's
existing weak-reference cache distinguishes the two frames.

The development app played a generated 3840×2160 H.264 clip in
light and dark. Across eight paired trials, the updated build
reused 516 of 550 payload buffers (94%).
The median of trial allocation medians was 0.46 ms
before and 0.00 ms after. With two concurrent encoders,
the baseline applied 2–3 and the updated app applied
1–4 of 120 expected preview updates per sample. Host
activity outside the test varied. Sustained 4K60 playback under
contention remains open.

First-frame and exact-seek editor snapshots matched byte for byte
in both appearances. All 120 decoded export frames and timestamps
matched the baseline in each appearance after the same split. All
1,193 GPUI tests and three sparse timeline FFmpeg tests passed.
[Per-run timing and pixel evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS exact video seeks avoid unnecessary preroll

When a new FFmpeg stream is needed for a distant time, the decoder
asks for the target position. If that stream ends or starts above
the target, it retries the previous preroll position. This preserves
the frame just before a keyframe when the first seek rounds forward.

In both appearances, a native editor test sought to 2.02 and 5.10
seconds in a generated 3840×2160 video. The first seek read five
payloads instead of 125 and the second read ten instead of 130.
The longer-seek pairs measured 2.51–3.22 seconds before and
0.28–0.46 seconds after. The reverse seek to 3.99999 seconds
required the fallback and read one more payload than the baseline.
Its latency varied. The next nearby seek read about 30 payloads
in both builds.

The startup and four seek snapshots matched byte for byte in
each appearance. All 180 decoded export frames and timestamps
matched the baseline in each appearance after the same split.
Standalone checks covered an open GOP with a five-second start
offset and the sparse screen and camera fixtures. All 1,194 GPUI
tests and three sparse FFmpeg tests passed. Sustained playback
under contention remains open.
[Per-seek evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 native macOS History tooltips and editor reopen

Recent Captures rendered in a visible AppKit window on the built-in
Retina display. App-local input exercised **Show in folder** and
**Delete** in the 400-point grid, 140-point grid, and 140-point
list at 2× in both appearances. WindowServer reported the same
window before and after the narrow resize. All 12 rendered
tooltips stayed inside the window and cleared on pointer exit.
Sampled solid text and fill colors measured contrast of 16.85:1
in light and 18.44:1 in dark.

Escape removed the History window from WindowServer. Reopening
History and pressing Enter opened the generated screenshot in
the editor with exact pixels in both themes. All 63 Electron
History and popover reference tests passed. The scene captures
show the GPUI content at native scale. Physical input and
another display remain outside this run.
[Light compact grid](tests/native/evidence/macos-ui/history-tooltip-grid-light-20261119.png)
[Dark compact list](tests/native/evidence/macos-ui/history-tooltip-list-dark-20261119.png)
[Native fixture evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS native window and display picker rendering

Both appearances passed the window and All-in-One display picker
on the built-in 2× Retina display. In the native Metal scene,
the window picker left a 720×480-pixel transparent hole over the
generated target and dimmed the surrounding display at 50% opacity.
Moving off the target or leaving the picker restored the scrim.
The display picker cleared the scrim on entry, restored it on
exit and left the retained handoff scene fully transparent
after confirmation. Cancelling the recording controls removed it.

Picking the generated window closed the selector and left one
native AppKit outline. WindowServer verified the same outline ID
at the resized target, two points outside each edge. Closing the
target hid that outline. The probe used app-local input and stopped
at the recording toolbar. Physical pointer delivery and movement
across displays remain open.
[Native selector and pixel evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS wide fixed-header scroll seam validation

A generated 1280-pixel-wide page repeated the same rows in
its center while retaining distinct content at the sides.
The old matcher chose a 120-pixel overlap and would have
omitted 46 rows. The revised matcher checks the full-width
strip when its center matches exactly. When the sides differ,
the existing whole-image matcher can recover an exact seam.
When it finds no exact seam, the full-width difference must meet
the existing confidence threshold.

The revised overlap was 74 pixels. Both the preview and saved
1280×246 PNG matched the generated document pixel for pixel,
including one fixed header. The complete native fixture passed
on arm64 and x86_64 under Rosetta, and 56 Electron scroll tests
passed. A 3456×640 ordinary seam took a median 11.98 ms before
and 12.45 ms after, across 24 samples per build. This measures
the matcher. Histories that produce identical complete frames
remain ambiguous.
[Native seam and timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS wide scroll seams with a changed center pixel

A one-pixel change in the repeated center region reproduced the
120-row false overlap after the previous exact-center fix. The
true 74-row seam remained exact across the full frame. The matcher
now compares the selected center score with the full-width score,
so disagreement from the outer columns can trigger the existing
exact match even when the center differs slightly.

The exact-center and changed-pixel fixtures each recovered the
74-row overlap and saved a 1280×246 image matching the generated
document and preview pixel for pixel. Both native Swift suites
passed on arm64 and x86_64 under Rosetta, and 56 Electron
scroll tests passed. The matcher-only median for an ordinary
3456×640 seam with noise was 12.55 ms before and 13.68 ms after.
Identical full-frame histories remain ambiguous.
[Native fixture and timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS live wide fixed-header scroll seam

A generated AppKit window displayed a fixed header and repeated
center rows with one changed pixel in the repeat. The preflight-
granted screen-only fixture captured that window by ID at 2×.
Two 1280×160 frames showed the page at offsets zero and 86.
The same window rendered and supplied a full 1280×246 reference.

With the captured frames, the previous matcher chose a 120-row
overlap and saved a 200-row image. The current matcher chose 74
and saved all 246 rows. The preview and saved PNG matched the
live reference after sRGB conversion, with zero channel difference.
The saved PNG retained Display P3 and all 1,259,520 decoded
RGBA bytes matched the live reference. The arm64 Swift fixture
suite also passed.

The fixture advanced the target through app-local commands and
fed the captured frames into the native stitch and preview code.
Physical scrolling and the capture panel lifecycle were outside
this run.
[Live capture evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS full-width retry for a noisy scroll overlap

A changed pixel inside the true overlap made exact full-image
matching unavailable. The prior matcher rejected the second frame
because the repeated center's proposed seam failed the full-width
confidence check. The revised matcher searches the full width in
that case and accepts a seam only if it meets the existing score
threshold and wins by the existing score margin.

The fixture recovered the 74-row seam and saved the same
1280×246 pixels as the preview and expected page. A separate
fixture with two plausible full-width seams preserved the accepted
prefix. Removing the uniqueness check from a test build made that
fixture fail.

A 3456×640 fixture triggered the retry. Its matcher-only median
was 173.13 ms with an unpruned scan and 44.70 ms with row-by-row
error bounds, across 24 measurements each. The native Swift
suites passed on arm64 and x86_64 under Rosetta, and 56 Electron
scroll tests passed. Identical full-frame histories remain ambiguous.
[Native recovery and timing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS scroll seam recovery for tied center matches

Two seams tied in the center strip of a wide fixed-header capture.
A changed pixel inside the real overlap prevented the exact
whole-image match. The previous matcher stopped after the first
frame. The updated matcher checks the full width when strong center
matches tie and the footer passes the existing exactness rule.
It still requires a unique full-width score within the existing
confidence limits.

The regression recovered the 72-row seam even when the requested
step favored the false 120-row seam. Its preview and saved
1280×248 PNG matched the generated page pixel for pixel. When
both candidates also tied at full width, the matcher stopped.
The arm64 and x86_64 native Swift fixtures passed, along with
56 Electron scroll tests. These tests used generated frames.
[Native seam evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Save confirmation probe limits

The isolated editor opened a Save sheet for an annotated 640×360
fixture. Cancel dismissed it and returned the editor to idle without
writing a file. The reopened sheet exposed its generated PNG name,
temporary directory and enabled Save button in the AppKit
accessibility tree.

Direct AppKit confirmation raised an unimplemented-action exception.
Poracode's accessibility invocation reported a stale snapshot. The
window-targeted background and foreground input attempts left the
sheet present and produced no PNG. This leaves native confirmation
unverified. The 55 Electron screenshot and annotation reference
tests passed. The temporary confirmation code was removed.
[Native Save probe evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS decoder reply deadline and current checks

The full repository check found that the Unix video decoder used a
timer prohibited by the GPUI check. The decoder now uses the existing
blocking reply deadline. When the caller stops waiting, it releases
the reply's lifetime signal and the FFmpeg worker can retire an
abandoned read. A superseded seek still cancels the individual read
and retains the decoder for the next request. Camera warmup uses the
same reply deadline through the blocking executor.

Repository checks passed with 2,415 Vitest cases and three skipped.
All 1,194 GPUI tests passed, including the deterministic cancellation
and abandonment check. Three sparse timeline tests and the camera
visibility test passed with development FFmpeg. The Swift daemon
typechecked for arm64 and x86_64. Light and dark Metal selection,
scroll, and recording handoff fixtures passed. The initial light
recording fixture hit its held-start state assertion and passed
on retry. The Windows-native gate skipped on this host.
[Checks and decoder evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS recording fixture startup ordering

The earlier light recording fixture observed Idle where it expected
Starting. Startup awaits an asynchronous camera protection reply,
and rendering alone cannot establish that the daemon accepted Start.
The held-start fixture now checks Idle while the protection reply
is deliberately held. It then waits for the daemon to hold the
start reply after writing the fixture recording, and checks Starting
before continuing the existing stop and target-closure assertions.
At teardown, a third barrier observes the protection disable request
before the fixture resets. Each phase has its own listener.

The rebuilt Metal recording workflow passed twice per appearance.
Each run exercised all seven held-start cases. The fixture uses
generated media and app-local input. This verifies the test ordering.
[Recording fixture evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS recording toolbar status alignment

The recording toolbar kept the same AppKit window bounds when
Starting or Stopping replaced the elapsed timer, but two separators
and the controls beside them moved two Retina pixels to the right.
The status text now fits inside the existing 64-point timer region.
All three states retain the same separator positions in both themes.

Window-scoped native captures verified the 276×44-point window and
552×88 pixel output. Both pending labels remained fully visible.
The light and dark Metal recording workflows passed the new timer
geometry assertions. All 76 Electron recording-control reference
tests passed. These runs used generated media and held RPC replies.
[Light Starting toolbar](tests/native/evidence/macos-ui/recording-starting-retina-light-20261119.png)
[Dark Stopping toolbar](tests/native/evidence/macos-ui/recording-stopping-retina-dark-20261119.png)
[Native recording status evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS long recording title keyboard tooltip

The compact recording title chip truncates long titles. Its
keyboard-focus tooltip now marks truncation in the two-line body
with an ellipsis. The old light-mode captures ended with partial
words for both the mixed Unicode title and unbroken identifier.

The live GPUI toolbar probe rendered both cases at 2× in light and
dark. Pixel checks identified the final three dots on line two
in all four updated scenes. The 445×52-point focused viewport
contained each tooltip. Tab moved from the title to the Start
control and replaced the tooltip in the checked dark Unicode case.
The light and dark 2× Metal control workflows and all 76 Electron
recording-control tests passed. The probe used generated titles
and app-local keyboard events.
[Light Unicode tooltip](tests/native/evidence/macos-ui/recording-title-focus-retina-light-20261119.png)
[Dark unbroken-title tooltip](tests/native/evidence/macos-ui/recording-title-focus-retina-dark-20261119.png)
[Native-title evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS live capture with tied center scroll seams

The wide tied-seam regression now has live Retina capture evidence.
A generated AppKit window rendered two 1280×160 frames at offsets
zero and 88, then rendered the full 1280×248 page. The captured
second frame differed from its expected header and body rows at
exactly one pixel in the true overlap.

The previous matcher kept the first frame. The current matcher
recovered the 72-row overlap and saved all 248 rows. The preview and
saved image matched the full reference after sRGB conversion.
The PNG retained Display P3, with all 1,269,760 decoded RGBA bytes
matching the reference. The fixture captured only its own window
by ID and advanced the page with app-local commands.
[Live tied-seam evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS visible tooltips beside tall preview owners

The tall-owner regression now shows the real AppKit tooltip panel
beside a visible generated owner. WindowServer reported a four-point
gap on the open side in light and dark. Both tooltips fit the
visible frame, ignored pointer events and stayed registered for
capture exclusion while shown. Dismissal removed the window and
its registry entry.

The focused probe and full arm64 and Rosetta x86_64 Swift fixtures
passed. All 56 Electron scroll tests passed. The probe supplied
app-local pointer coordinates on the built-in Retina display.
Physical hover and mixed-display placement remain open.
[Visible-tooltip evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS tooltip placement after a capture wait

A protected tooltip could appear at an earlier pointer position
after waiting for a screenshot to finish. The native panel now uses
the current pointer position when it is ready to appear on the same
display. It moves while transparent, retains its capture-exclusion ID,
and keeps the four-point gap from the owner.

A held-screenshot fixture moved the pointer 18 points within the
owner. The old panel stayed at its initial position. The updated
panel moved all 18 points. Existing pointer-exit and tooltip-owner
replacement checks passed. The arm64 and Rosetta x86_64 Swift
suites, full daemon typechecks on both architectures, and all 56
Electron scroll tests passed. Verification used
app-local pointer coordinates.
[Pointer and capture-barrier evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS tooltip anchors while owners move

WindowServer bounds now refresh before the protected tooltip is
placed and again before it appears. If the owner is hidden during
the capture wait, the tooltip retires and releases its exclusion ID.
If changed bounds move the anchor to a different display during that
wait, the pending tooltip is suppressed.

A generated AppKit owner moved 140 points horizontally and 32
points vertically while the screenshot barrier held. The previous
tooltip failed to appear at a pointer position over the moved owner.
The updated panel appeared with its original exclusion ID and a
four-point gap below the moved owner. A second trial hid the owner
while the tooltip waited and verified dismissal.

Both native Swift suites and both full daemon typechecks passed.
All 56 targeted Electron scroll tests passed. Verification used
app-local pointer coordinates on one display.
[Native tooltip owner evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS desktop hover transitions

Unpressed macOS desktop mouse events moved between Start recording
and Close on the floating recording bar, then between Close preview
and Delete screenshot on the floating screenshot preview. Eight
sequences in light and dark passed. WindowServer reported one
tooltip from the probe's daemon after every hover, a new window ID
after each replacement, and no tooltip after each exit. Each tooltip
kept a four-point gap from its owner. The probe used the physical
Retina display with OS-posted events and the production Swift
tooltip path.

Warm toolbar shows and switches took 35–62 ms. Preview tooltips took
186–250 ms including the configured 150 ms hover dwell. Exits took
26–70 ms. First shows in four fresh probe processes took 165–432 ms.
The probe launches its daemon on demand. Normal app initialization
starts the daemon ahead of hover, so the cold probe range measures a
different startup sequence. These timings stop at WindowServer
visibility and include polling. The five Electron reference files
passed all 168 tests.
[Desktop hover timings](tests/native/evidence/macos-ui/native-tooltip-physical-pointer-20261117.txt)

### 2026-11-19 macOS capture shortcuts and window readiness

Temporary area and window capture shortcuts went through the macOS
desktop keyboard event path on the built-in Retina display. Six runs
per appearance opened the area overlay, replaced it with the window
picker, then reopened area capture after dismissal. The daemon
fixture saw one window-list request per switch and returned a
generated 3840×2160 PNG on each confirmation. All 12 saved PNGs
matched the fixture bytes.

The timing starts at posting the desktop key for the three shortcut
rows. Cancellation, area confirmation and opening the editor use
app-local input or a test command. Warm results are medians of
trials two through six. Window timing includes polling and stops at
WindowServer visibility.

| Transition (ms)                       | Light cold | Light warm | Dark cold | Dark warm | Slowest |
| ------------------------------------- | ---------: | ---------: | --------: | --------: | ------: |
| Area overlay opens by shortcut        |       73.2 |       22.6 |      55.0 |      22.8 |    73.2 |
| Area switches to window by shortcut   |       27.7 |       25.3 |      33.3 |      26.7 |    34.0 |
| App-local cancel hides overlay        |        6.2 |       10.6 |      12.0 |      11.8 |    21.9 |
| Area overlay reopens by shortcut      |       19.6 |       21.5 |      21.8 |      21.9 |    25.4 |
| Area confirmation hides overlay       |       17.6 |       20.2 |      20.1 |      16.0 |    23.3 |
| Confirmation to preview window        |       18.0 |       20.6 |      20.6 |      17.9 |    27.0 |
| Confirmation to 4K thumbnail ready    |       60.1 |       59.6 |      60.7 |      60.5 |    65.6 |
| Preview double-click to editor window |       90.3 |       56.5 |      76.4 |      55.5 |    90.3 |

The desktop drag and directed Escape attempts did not complete a
capture or cancel the overlay in this session. Real button and Escape
delivery for the workflow, live capture latency under desktop
input, and first composited frames remain unverified by this run.
All 145 tests in the five related Electron reference files passed.
[Per-trial results](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS tooltip requests after the owner hides

The daemon now requires a visible WindowServer owner for each
tooltip show request. It can still use the submitted geometry if
the live owner has no readable bounds. It can also resolve a
visible owner when the request omits the geometry.

The native regression hid an AppKit owner before submitting a new
tooltip request with its last frame. The old implementation showed the
tooltip at that frame. The revised daemon suppressed it without
creating a panel or registering an exclusion window. The existing
cases for pointer movement, moving owners, and owners hidden
during a screenshot wait still pass. The complete Swift fixtures and
daemon typechecks passed on arm64 and x86_64, along with 70 targeted
Electron and daemon contract tests.
[Native tooltip results](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

The overlay input check also tried Escape on a blank AppKit window
whose PID was confirmed frontmost. The signed desktop helper
reported unverified delivery and the window's local event monitor
recorded no Escape. Physical Escape and drag behavior in the
selector still need a session with verified input delivery.

### 2026-11-19 macOS 4K playback with two active encoders

The GPUI development app played a generated 3840×2160 H.264
clip at 60 fps. Across three runs per appearance without test
encoders, it applied 110–121 images in a two-second window.
With two independent 4K encoders, the range fell to 5–61 images,
and the longest gap between applied images was 563 ms. The
slowest loaded Play command returned in 3.2 ms. These measurements
end at the app and exclude display presentation. All 12 paused
preview PNGs were identical and their RGB matched the first
frame decoded independently with bundled FFmpeg.

The slowest decoded-frame request took 551 ms. During that
request the reader completed 27 consecutive raw payloads from
one decoder while advancing to the requested timestamp. Each
of those payloads took at most 49 ms. That run spent its wait
catching up through intermediate frames.

Two standalone receiver changes had mixed results. Increasing
the raw frame reader's scheduling priority shortened four of
six loaded transfers. Waiting inside the socket read for the
complete payload shortened three of six. All sampled frame-byte
digests and 180 timestamp sequences matched. The candidate
timing results were inconclusive. The GPUI suite passed 1,194
tests with 23 ignored. Three development FFmpeg sparse timeline
tests and 50 Electron reference tests passed. Sustained 4K60
playback under comparable encoding load remains open.
[Per-trial timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K decode and Electron comparison

The same generated 4K60 H.264 pattern ran through a minimal
Electron video and 640×360 canvas fixture. Chromium reported 120
video frame callbacks and zero dropped frames in every two-second
sample, with VideoToolbox and with its FFmpeg software decoder.
The six loaded samples ran alongside two independent 4K encoders.
The fixture exercised the video and canvas paths and omitted the
full editor composition engine.

In the standalone GPUI-style BGRA transfer, caps of two or four
decoder threads and one filter thread each won at most half of
their six paired loaded runs. All four settings produced
identical BGRA hashes and timestamps for 180 frames.

A separate test reduced each raw frame from 33.2 MB of BGRA
to 12.4 MB of planar YUV. With the same encoding load, the
median for 180 frames fell from 2.61 s to 1.12 s across four
pairs. That measures frame production and transfer. Converting
the YUV back to BGRA separately reproduced every pixel and
timestamp from this source. The raw stream dropped the sample
aspect ratio header. The editor would need to carry that
metadata and preserve color and timing across other formats.
Fifty-six related Electron tests passed.
[Decoder and Electron timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K packed RGB transport

A standalone probe sent 24.9 MB of BGR per 4K frame and restored
opaque alpha at the receiver. Both direct BGR and packing after
the existing conversion to BGRA reproduced all pixels and
timestamps in the five opaque fixtures tested. The partially
transparent control lost alpha in both BGR paths.

With two encoders running, median times for 180 frames were
1.83 s for BGRA, 1.67 s for direct BGR and 1.71 s for packing
after BGRA conversion. Each BGR variant won four of six paired
runs. Restoring alpha took a median of 1.83 ms and 1.73 ms per
frame respectively. Without competing encoders, the medians were
0.85 s, 0.86 s and 0.93 s. These timings include alpha expansion
and exclude editor composition. The comparison leaves a smaller
benefit than the planar YUV result and requires a reliable
opacity check for production use. Three development FFmpeg
timeline tests and 56 Electron reference tests passed.
[Packed RGB evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS tooltips crossing displays during capture

The protected tooltip now resolves the current pointer's display
after the capture wait. If the display changes, AppKit fits the
existing panel to the destination work area before revealing it.
The panel retains its capture-exclusion ID. If the pointer no
longer has a display or a visible owner, the pending panel retires.

A native fixture simulated adjacent displays inside the built-in
Retina screen. The old panel became visible outside the destination
work area when the pointer crossed within a straddling owner.
The revised panel fit the 220-point destination after both a
pointer-only crossing and an owner move. The long label rewrapped,
the panel kept a four-point gap below the owner, and dismissal
removed its exclusion entry. The fixture used light and dark
appearances and app-local coordinates. The full Swift fixtures
and daemon typechecks passed on arm64 and Rosetta x86_64. All 70
related Electron and contract tests passed. Physical multi-display
and mixed-DPI behavior remain open.
[Display transition results](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS tooltip replacement after an exclusion refresh fails

A recording filter refresh could fail to resolve a tooltip window
that had already been replaced. The failure reached the replacement
even though its window was present. The registry now reassesses the
current window set when a refresh finishes for an older revision.
It still reports failures for the current revision.

The native fixture held a refresh for the Camera tooltip, replaced it
with the Microphone tooltip, then returned the missing-window error
for the retired panel. The previous implementation suppressed the
Microphone tooltip. The revised implementation applied the new window
ID before revealing the replacement and cleared the ID on dismissal.
The panel stayed transparent while the retry was held. The fixture uses
real AppKit panels and a simulated asynchronous filter update. The
full Swift fixtures and daemon typechecks passed on arm64 and Rosetta
x86_64, along with 70 related Electron and contract tests.
[Filter refresh results](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS protected tooltips during recording restarts

A tooltip awaiting an old recording filter could appear when the
registry invalidated that refresh during a new recording start.
Filter updates now recheck the startup barrier whenever a refresh
resumes. A pending request also waits through a second start if
it begins before the request resumes.

The AppKit fixture held the old filter refresh and began a new
recording startup. The old code revealed the panel immediately.
The revised panel stayed transparent through two simulated starts
and a held filter update for the new recording. A late error from
the old filter did not interrupt the new request. The original panel
and exclusion ID survived, and dismissing the panel released the ID.
The native fixture simulated the recorder. The full Swift fixtures
and daemon typechecks passed on arm64 and Rosetta x86_64, along with
70 related Electron and contract tests.
[Recording restart evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS protected windows during overlapping captures

A tooltip could become visible after a screenshot finished even
when a new recording had started during the screenshot wait. The
exclusion registry now checks both conditions again after waiting.
The tooltip, recording overlay and outline, and camera preview use
that shared check before becoming visible.

The native tooltip fixture began a recording start while the panel
was waiting for a screenshot. The old panel appeared as soon as
the screenshot finished. The revised panel stayed transparent
through startup, the new filter update, and a second screenshot
that began during the update. It retained its original AppKit
window and exclusion ID. The filter and capture waits were simulated.
The complete Swift fixtures and daemon typechecks passed on arm64
and Rosetta x86_64. The arm64 recording outline and generated camera
preview lifecycle fixture passed, along with 70 Electron and daemon
contract tests.
[Overlapping capture results](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 live macOS tooltip during screenshot and stream startup

With the existing Screen Recording grant, a temporary AppKit
probe took a real, window-only screenshot of a generated backdrop.
It held the screenshot's registry completion until recording
startup had begun. The production tooltip stayed transparent until
a live ScreenCaptureKit stream had started and its updated filter
included the tooltip's exclusion ID.

In light and dark, the 370×340 screenshots showed the blue backdrop
at all nine sampled positions. After the tooltip appeared, the
1728×1117 stream showed the backdrop at all 5,704 sampled pixels
around the tooltip in each appearance. A separate ordinary red
window at the same position appeared in nine of nine positive-control
samples per appearance. Filter refresh and reveal took 65.8 ms in
light and 61.5 ms in dark, measured from release of the startup
barrier. These times do not include stream startup or display paint.

The stream used an allowlist limited to generated windows. This
probe checked live frames before encoding, without a GPUI editor
handoff. A preliminary display-filter screenshot did not yield the
expected backdrop samples. The accepted screenshot check used the
window-only filter. All 70 related Electron and daemon contract
tests passed.
[Live overlap evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS capture overlap through recording and export

A preflight-granted probe used the current macOS recorder to encode
a generated area in light and dark. The screenshot callback was held
in the exclusion registry until recording startup began. The native
tooltip kept its window ID, stayed transparent during the wait, and
was visible on screen during the protected phase. The probe limited
ScreenCaptureKit to its own generated windows and disabled audio
and input tracking.

Both 700×500 MOVs preserved the backdrop in the 12,720-pixel region
around the tooltip. Every protected-phase frame matched the backdrop
at all tested pixels. An ordinary red window at the same position
appeared in both files. A test bridge supplied the recorded files
to the GPUI preview, which opened the editor. Each project saved a
split at 0.45 s and exported a 700×500 MP4.

At the matched export timestamp of 0.633333 s, every protected-frame
pixel was within five RGB levels of the editor preview. At 1.666667 s,
99.418% of the ordinary-window frame met the same tolerance.
Larger differences were confined to an eight-pixel band around the
ordinary window's edges. The probe exercised live capture, encoding,
preview, saved edits and export in sequence. The GPUI app received
the finished clip through the bridge. All 87 related Electron tests
passed.
[Encoded overlap evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS overlap through the GPUI Start and Stop handoff

GPUI sent its normal Start and Stop requests to a temporary
screen-only daemon under the existing Screen Recording grant.
The bridge held Start after the recording overlay had prepared.
A real 540×326 window screenshot held registry completion while
the daemon began startup. The tooltip request stayed pending until
startup resumed and its native panel appeared. The app passed the
recording toolbar's window ID in the Start exclusion list.

The generated-window allowlist kept the scene isolated. Both
1040×620 MOVs and the editor's 1040×620 MP4 exports preserved
all 14,040 sampled pixels around the tooltip in every protected
frame. An ordinary red window appeared at the former tooltip
position. Each project saved a split at 0.45 s. At matched
timestamps, all protected-frame pixels and 99.727% of ordinary
sample-frame pixels were within five RGB levels of the preview.

| Measured interval                           | Light |  Dark |
| ------------------------------------------- | ----: | ----: |
| Startup barrier release to Start reply (ms) | 286.7 | 217.9 |
| Stop RPC round trip (ms)                    |  10.0 |  11.1 |
| Stop command to decoded preview (ms)        | 114.7 |  79.9 |
| Preview to editor frame ready (ms)          | 107.2 | 113.1 |

The probe used app-local Start and Stop commands and an injected
owner-relative tooltip request. The bridge disabled keyboard
tracking on the test request. Microphone, camera and system audio
were off. These timings exclude physical input and first display
paint. All 87 related Electron tests passed.
[GPUI handoff and decoded frame evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS countdown hit testing

The countdown exposed a single accessible button, but AppKit hit
testing selected its image view over the timer icon. The countdown
now receives pointer input across the full control.

The lifecycle fixture lays out the native panel and checks hit targets
across the margins, icon and text. It sends the cancellation click
through the actual AppKit window at the icon position and verifies
that the panel disappears. The following countdown completes and
the last panel hides through RPC. The native fixtures passed on
arm64 and Rosetta x86_64, and 27 Electron and daemon contract tests
passed. The pointer check used app-local events.
[Timer hit-testing evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS countdown cursor after dismissal

Cancelling a hovered countdown could leave an extra arrow cursor on
the daemon's AppKit cursor stack. The timer now registers its arrow
as a window cursor rectangle. Hover still controls the Cancel label.

The native fixture pushed a sentinel cursor, sent the countdown an
app-local hover and then cancelled through the real AppKit window.
The previous implementation left the sentinel on the stack after
the fixture popped once. The revised implementation restored the
original cursor. The native lifecycle fixtures passed on arm64
and Rosetta x86_64, along with 27 Electron and daemon contract tests.
The fixture did not move the physical pointer.
[Countdown cursor evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS countdown pointer state on restart

A replacement countdown could inherit its predecessor's hover
caption and cancel when the predecessor's mouse press was released.
Each presentation now clears the old press and recalculates hover
from the pointer position. An inside release cancels only after a
press on the current countdown.

The AppKit regression pressed the visible timer, replaced it, and
sent the old mouse-up through NSWindow. The old code showed Cancel
and emitted cancellation. The revised fixture moved the panel to the
other side of the display. It showed the new countdown and stayed
visible after the old release. The CI lifecycle workflow now runs
this regression. The native lifecycle fixtures passed on arm64 and
Rosetta x86_64, along with 27 Electron and daemon contract tests.
The fixture used app-local events.
[Countdown restart evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS countdown feedback while dragging

Dragging a pressed countdown outside its panel could leave the
Cancel caption visible. The countdown now updates its hover and
pressed appearance from drag and release positions.

An AppKit mouse-down and drag outside restored the countdown
caption, and releasing there kept the panel visible. In a second
gesture, dragging back inside restored Cancel and releasing there
dismissed the timer. The default CI lifecycle fixture covers both
gestures with app-local events. The native lifecycle fixtures passed
on arm64 and Rosetta x86_64, along with 27 Electron and daemon contract
tests.
[Countdown drag evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS countdown catch-up before the deadline

A three-second AppKit countdown was sampled while its main loop
was deliberately stalled for up to 2.65 s. On arm64 and Rosetta
x86_64, the first observed caption after a 1.65 s stall was “2”.
After a 2.65 s stall, it was “1”, with the expired “2” skipped.
The corrected caption was first observed 11–32 ms after resuming.
All eight runs dismissed the timer and emitted completion at
3.002–3.007 s from the show reply.

This verifies the countdown deadline and label state under partial
stalls using a hidden AppKit panel and the production timer. The
probe sampled labels about every 10 ms. Capture and display paint
are outside these timings. All 27 related Electron and daemon
contract tests passed.
[Countdown timing measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Auto Scroll restart after duplicate frames

Auto Scroll could stop as soon as it restarted. The first snapshot
often matched the last saved image and inherited the previous run's
duplicate count. Each run now starts a fresh count, and its baseline
snapshot does not consume a post-scroll retry.

The native fixture stopped on repeated frames, restarted through the
normal Auto Scroll request, and submitted the same baseline. The old
implementation stopped before scheduling another scroll step. The
revised one scheduled the step, then stopped after the existing three
post-scroll duplicates. The saved PNG matched the original accepted
frame byte for byte. The Swift fixtures and daemon typechecks passed on
arm64 and Rosetta x86_64, along with 62 Electron and daemon contract tests.
The fixture injects frames and intercepts input. Identical-image histories
still require independent information to disambiguate a stitch.
[Auto Scroll restart evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Auto Scroll pointer boundary at dispatch

Auto Scroll could post a wheel step outside the selected area
between cursor-monitor callbacks. A step now samples the pointer,
updates the boundary prompt, and uses that same position as the
wheel event target. The next step resumes when the pointer returns.

The native fixture moved the simulated pointer outside without
running the periodic cursor check. The previous implementation
posted one step outside. The revised fixture posted zero, showed
the prompt, and sent exactly one step at the tested inside point
when the pointer returned. The native fixtures and daemon typechecks
passed on arm64 and Rosetta x86_64, along with 62 Electron and daemon
contract tests. No desktop input was posted.
[Auto Scroll boundary evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Auto Scroll input failures preserve captures

Auto Scroll now checks its existing event-posting grant when a run
starts, before moving the pointer or taking another baseline. If
Core Graphics cannot create a wheel event during a run, Auto Scroll
stops and reports an input error. GPUI and Electron show an input
failure with no Save suggestion when zero frames are available.

The native regression rejected a wheel event after a valid image.
The previous code kept scrolling. The revised code reported the
failure and saved the accepted image byte for byte. A separate
simulated preflight denial stopped before requesting another frame.
The 200-call preflight probe measured a 9.1 ms median and 9.8 ms p95
on this host. Production checks the grant once per run. Swift
fixtures and daemon typechecks passed on arm64 and Rosetta x86_64.
The 14 GPUI scroll tests and 62 Electron and contract tests passed.
[Auto Scroll input evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 GPUI save and reopen after Auto Scroll input errors

The GPUI interruption workflow now checks both input-error messages.
With zero frames the message reports the input failure. With a
retained portion it also offers Save. The fixture sends the simulated
daemon events through the normal capture coordinator before finishing.

The offscreen macOS workflow passed in light and dark. Both saves
produced byte-identical 640×900 PNGs with all 576,000 pixels matching
the generated capture. Each image entered history and reopened in
the editor. The fixture verifies notification text, while the separate
control snapshots verify light and dark icon contrast. It uses a
simulated scroll daemon and app-local input. All 62 related Electron
and contract tests passed.
[Scroll input workflow evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS scroll preview and retry after a rejected seam

A failed alignment could cancel a preview render for frames that
had already been verified. Restarting Auto Scroll also left the
matcher stopped. The daemon now preserves verified preview work
and drops the rejected suffix. On retry it starts from the last
accepted image and its stored duplicate fingerprint.

The native regression held the render for the second verified
image, rejected the next seam, then released the render. The old
preview stayed at one frame. The revised preview showed both
verified frames. After retry, a new overlapping image extended
the accepted result to three frames, and the saved 640×352 PNG
matched the expected page pixel for pixel. This fixture supplies
frames and alignment results to test lifecycle ordering. Retry
uses the existing matcher confidence checks. Image-only histories
that produce identical frames remain ambiguous. The Swift suites
and daemon typechecks passed on arm64 and Rosetta x86_64, along
with 62 Electron and daemon contract tests.
[Scroll retry evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS retry through the real scroll matcher

The native retry regression now uses the production image matcher
with a generated 2× capture. It rejects a non-overlapping frame,
publishes the verified two-frame preview, then recovers from the
accepted image. The saved 640×352 PNG matches the expected page
in every pixel.

A separate replay used three previously captured Retina images
of the generated fixed-header page. After a rejected frame, the
retry joined the last captured image to the verified prefix. The
920×1824 PNG matched the expected crop of the three native images
in every pixel. These cases verify recovery when the source data
contains an unambiguous overlap. They use injected frames and
app-local scheduling. The native suite passed on arm64 and Rosetta
x86_64, along with 62 Electron and daemon contract tests.
[Real-matcher retry evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Auto Scroll waits for the current seam

Auto Scroll could schedule its next wheel event while the previous
frame's alignment was still pending. If the seam failed, that extra
step could move the page farther away from the last verified image.

The daemon now schedules the next step after the alignment queue
has handled the frame. The native fixture held alignment and observed
one scheduled step in the revised code, compared with two before
the fix. After accepting the seam, the scheduler resumed while PNG
encoding was still held. A rejected seam then stopped Auto Scroll
with two accepted frames and no additional step. The fixture uses
injected frames and scroll scheduling. Native Swift suites and
daemon typechecks passed on arm64 and Rosetta x86_64, along with
62 Electron and daemon contract tests.
[Scroll scheduling evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Auto Scroll restart during alignment

A restart takes a baseline screenshot before sending the next
wheel step. If that screenshot matched a frame still being
aligned, the duplicate fast path could resume scrolling early.
The restart now waits for pending alignment even when the
baseline pixels match the previous capture.

The native regression stopped Auto during a held alignment, then
restarted with the same baseline image. The previous implementation
scheduled an extra step before the alignment finished. The revised
case waited, then scheduled exactly one step for the current run.
The saved three-frame PNG matched the expected page pixel for pixel.
The Swift fixtures and daemon typechecks passed on arm64 and Rosetta
x86_64, along with 62 Electron and daemon contract tests. The fixture
supplies frames and holds alignment without desktop input.
[Scroll restart scheduling evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS camera preview completion ordering

A camera startup could install its preview layer after the user
selected another device or closed the panel. Camera capture sessions
now belong to the serial camera queue, and AppKit accepts preview
layers and errors only for the current request.

The native fixture generated an A → B → A selection sequence. The
previous implementation accepted both older completions and a
callback after Stop. The revised implementation accepted only the
latest A. The test used inputless AVFoundation sessions. A read-only
authorization check returned not determined, so live camera startup
and switching remain unverified. Recording-outline fixtures and
daemon typechecks passed on arm64 and Rosetta x86_64, along with
97 Electron and daemon contract tests.
[Camera completion evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS queued camera preview starts

Rapid camera selection could queue startup work for sources that
were already superseded. The serial camera queue now checks the
current request before setting up a device and before starting it.
If a request expires while AVFoundation is starting the device,
the queue retires that session after startup returns.

The native fixture held the queue through A → B → A and through
a new selection followed by Stop. The previous code reached all
four startup callbacks. The revised code reached only the latest
A callback. The fixture simulated startup work without opening
camera devices. Native lifecycle fixtures and daemon typechecks
passed on arm64 and Rosetta x86_64, along with 97 Electron and daemon
contract tests. Live startup and switching remain unverified.
[Camera startup queue evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS camera preview retry after startup or runtime errors

A failed camera preview could leave its selected source marked as
already applied. Showing the same source again returned success
while leaving the error on screen. Camera preview now reports
startup failure when the session cannot attach its input or start.
A runtime error clears the stopped layer and releases the session.
The current source can then be selected again.

The native fixture delivered a startup error and an AVFoundation
runtime notification from an inputless session. Before the fix,
the same-source startup retry left the start count at one. The
revised fixture started the same source again after each error
and cleared the error text. Live camera startup and switching
remain unverified on this host. The native lifecycle fixtures and
daemon typechecks passed on arm64 and Rosetta x86_64, along with
97 Electron and daemon contract tests.
[Camera preview retry evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS GPUI camera preview retry while recording

Selecting the locked camera again during recording now refreshes
its preview if the camera is already enabled. The request uses the
recording's selected source and leaves the recorder's camera active.

The isolated development app sent one preview Show request and
zero recorder Set Camera requests for the selection. Before the
fix it sent zero preview requests and one recorder request. The
light and dark runs covered an accepted preview retry and a
rejected one. Both kept the camera enabled and opened the recording
preview after Stop. The native smoke regression is available as
`python3 tests/native/macos-capture-history-probe.py --camera-retry`
with a GPUI parity development build.

The fixture used a simulated daemon and bundled video, so hardware
preview recovery is still unverified. All 97 related Electron and
daemon contract tests passed.
[Toolbar camera retry evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Stop while a camera preview retry is pending

The native GPUI smoke fixture held a camera-preview Show reply
during an active recording, then sent Stop. GPUI issued the camera
preview Hide request and opened the decoded recording preview while
the retry was still pending. The fixture then released a rejected retry and
verified that the recording preview remained available.

Stop to decoded preview took 148.2 ms in light and 61.9 ms in dark.
These are app-local smoke timings with bundled video and a simulated
daemon. They exclude hardware startup and compositor presentation.
All 97 related Electron and daemon contract tests passed.
[Stop and camera retry evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS camera preview restore at display edges

A saved camera panel that overlapped a connected display could
jump to the fallback corner if its top-left point was outside the
display. When no display contains that point, the daemon now
chooses the display with the largest panel overlap and clamps the
restored panel to its work area.

The native geometry fixture covered a panel beyond the left and
top edges, a top-left point at a joined edge, and a rectangle
spanning two displays. The revised placements stayed inside the
matched work area. A rectangle wholly outside the connected
displays used the fallback. Physical mixed-display dragging and
live camera output remain unverified. Native lifecycle fixtures and
daemon typechecks passed on arm64 and Rosetta x86_64, along with
97 Electron and daemon contract tests.
[Camera placement evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS camera preview fit after a display change

The open camera preview now reapplies its work-area constraints
when AppKit reports a display configuration change. The observer
belongs to the current panel and is removed when the panel closes.
[AppKit display-change notification](https://developer.apple.com/documentation/appkit/nsapplication/didchangescreenparametersnotification)

The native fixture posted the notification locally while a generated
camera panel was 24 points outside the work area. The panel moved
inside, retained its existing WindowServer and capture-exclusion
ID, and released that ID on dismissal. The fixture used the built-in
display without changing its settings. Physical screen changes,
Dock changes and live camera output remain unverified. Native lifecycle
fixtures and daemon typechecks passed on arm64 and Rosetta x86_64,
along with 97 Electron and daemon contract tests.
[Display change evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS camera preview fit before reveal

A camera preview waiting for capture protection could retain an
off-screen location until after it became visible. The daemon now
refits the staged panel to the current work area before revealing
it. A newer Show request that reuses the hidden panel also applies
its requested position before revealing.

The AppKit fixture placed the transparent panel partly outside the
work area while a screenshot held its reveal. After release, both
a first Show request and a superseding Show request revealed the
same protected window fully inside the work area. The window and
exclusion ID survived. The fixture staged the bounds and display
notification locally. Physical display changes and live camera
output remain unverified. Native lifecycle fixtures and daemon
typechecks passed on Apple Silicon and Rosetta x86_64, along with
97 Electron and daemon contract tests.
[Deferred camera placement evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS fitted camera preview position on reveal

A camera panel could be moved into the work area just before
reveal without reporting that correction. The daemon now emits
the fitted position when the panel appears and returns it in the
Show reply. GPUI receives the event, and Electron initializes
its preview state from the reply when the position is available.

The native fixture verified one report and the same reply position
for both a first request and a superseding request. A simulated
daemon in the light and dark GPUI smoke tests sent corrected
coordinates before the Show reply. GPUI retained those coordinates
and the recording preview still opened after the held-retry Stop.
The Electron test verified that the returned position supersedes
the original requested one. Native fixtures and daemon typechecks
passed on arm64 and Rosetta x86_64, along with 98 Electron and
daemon contract tests. These cases use generated sessions and
app-local controls.
[Camera position handoff evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS fitted camera preview position after a move

The Update reply now includes the panel’s actual coordinates after
AppKit fits it to the display work area. Electron uses those coordinates
when preserving preview placement for recording. A pending move reply
is discarded if that preview has been closed or replaced.

The AppKit fixture verified two position requests in sequence on the
built-in display. On Apple Silicon and Rosetta x86_64, the final reply
matched the position event and the panel stayed within the work area.
The Electron regressions reproduced the off-screen saved coordinates
and the closed preview becoming visible in app state. Both pass with
the fix, along with 100 targeted Electron and daemon contract tests
and typechecks for both daemon architectures. The native fixture used
a generated panel without live camera imagery.
[Camera movement evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS idle wakeups and editor memory

GPUI now follows macOS appearance changes through the AppKit observer
on its hidden keepalive window. The redundant two-second polling
thread is gone. With the recording controls dismissed, four
app-local appearance changes reached the system-mode theme. Explicit
Light and Dark held across the same changes.

In two before-and-after development runs, GPUI went from 40–42 to
37 interrupt wakeups per eight seconds at idle. CPU use measured
at most 0.001% of a core in both builds. The standalone Swift daemon
was near 8 MiB of physical footprint at idle. GPUI stayed near
22 MiB before capture and 115–116 MiB after closing the 4K screenshot
editor. The app released its large allocations on close. In one
after-close sample, 87 MiB remained in empty large malloc regions.
A standalone trial of the public allocator pressure-relief call
returned zero bytes against freed 4K-sized buffers on this host.

The editor memory measurements used a generated capture and the
timings exclude physical input. The theme change measured here was
scoped to the test process.
[Idle process measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS daemon exit notifications and idle wakeups

The GPUI watchdog now waits for the macOS kernel to signal daemon
exit. The existing 250 ms process poll serves as a fallback if the
kernel wait reports an unexpected error. The watchdog reaps the
matching child, rejects pending RPCs, and keeps the restart backoff.

Across three eight-second idle samples on the M5 Pro, GPUI
registered 33–37 interrupt wakeups before the change and 2–7 after.
CPU use and idle physical footprint stayed near zero and 22 MiB,
respectively. Both builds used a fixture daemon and dismissed the
recording controls before sampling. These results measure idle
process activity. The post-editor memory finding above remains open.

The crash fixture verified that an in-flight call failed promptly,
the daemon was reaped, the restart used a new PID, and a fresh RPC
succeeded. All eight GPUI daemon tests and 112 Electron reference
tests passed. An independent kernel wait and reaping probe passed
on Apple Silicon and Rosetta. The light and dark GPUI workflows
opened the 4K screenshot editor 40–56 ms after the fixture released
its History read.
[Daemon watchdog measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K screenshot editor memory

The editor now decodes opaque 8-bit RGB PNGs into the buffer that
will hold the RGBA source for edits and export. It expands the
channels in that buffer before applying the existing ICC conversion.
This removes the separate 24 MiB buffer for a 3840×2160 RGB image.

On the M5 Pro, after three open and close cycles per appearance,
the prior GPUI build held 118–120 MiB after close. The revised build
held 91–94 MiB. Empty large malloc regions dropped from 87 MiB to
63 MiB. The editing source and GPUI image each still use a
full-size pixel buffer. A separate annotated export produced
byte-identical PNGs before and after the change.

The median interval from area confirmation to source preparation
was 64 ms before and 71 ms after. The median interval to the editor
being ready was 142 ms before and 90 ms after in the same six trials
per build. These are development runs with a generated RGB PNG,
app-local input and readiness queries. They exclude display paint.
Forty-seven GPUI image, export and reopen tests and 26 Electron
reference tests passed. The native smoke also covered annotation
save, History and Print.
[4K editor memory and timing measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS screenshot Balance and blackout memory

Balance and blackout preview now read directly from the editor’s
decoded RGBA image. Balance still computes the same edge crop, and
blackout copies only the rectangle it needs to paint. Both paths
previously made a temporary copy of the full image.

Reopening a generated 3840×2160 screenshot with a small saved
blackout used 234 MiB of physical footprint in both appearances,
down from 266 MiB. The 31.6 MiB empty large malloc region from
the baseline was absent. Reopening with Balance enabled used
311 MiB, down from 321–322 MiB. The Balance fixture still reported
an empty large region of 31.6 MiB after opening. These figures
measure the editor while open.

The light and dark offscreen checks passed for scaled blackout
reopen, redaction order, balanced attachments and Balance effects.
The scaled blackout checks compared the preview with the reopened
PNG at four zoom levels and three display scales. The largest
sampled difference was one RGB level. All 376 active GPUI editor
tests and 22 Electron reference tests passed.
[Balance and blackout memory measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Balance sampling from the editor image

Balance now samples the existing editor image directly. Each
translucent sample retains the channel rounding used by the
premultiplied export image. Both paths share the crop calculation.
The editor skips the full-size premultiplied sampling image.

For generated 3840×2160 screenshots with opaque and translucent
borders, light and dark editor peaks fell from about 402 MiB to
392 MiB. The resident empty large malloc region went from
31.6 MiB to 22.4 MiB.

Across 21 paired trials per border type, the median isolated Balance
calculation fell from 56 to 15 ms for an opaque border and from 44
to 19 ms for a translucent border. Both implementations returned the
same four crop margins in every trial. The timing covers only the
crop calculation, including the prior full-image conversion.

The light and dark offscreen checks passed for Balance with frames,
effects and attachments. Forty-nine GPUI tests and 24 Electron
reference tests passed. The new regression compares the crop from
borrowed samples with the premultiplied image across 24 combinations
of border and content alpha, including the content cutoff.
[Balance sampling measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS screenshot export canvas reuse

Screenshot composition now uses the converted source image as its
initial canvas. The previous path allocated a blank canvas and drew
the source onto it at the same size before adding annotations.

For eight annotated 4K PNG exports per build, the median app-local
command time fell from 56 ms to 31 ms. Each command composed and
encoded the image and wrote it to a temporary file. Light and dark
outputs matched byte for byte across both builds. Peak process
footprint remained near 388 MiB and the footprint after closing the
editor remained near 157 MiB.

A regression compares the previous and current compositions
pixel for pixel across all 256 source alpha values, with and
without a translucent annotation. The light and dark offscreen
checks passed for scaled blackout, redaction order, rounded
reopen and Balance effects. The app-local Print and annotation
save probes passed, along with 378 GPUI tests and 24 Electron
reference tests.
[Screenshot export measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS inset preview samples the editor image

The screenshot editor now samples inset color directly from the
decoded RGBA image. It uses the same Balance crop and translucent
channel rounding as the premultiplied export path. The previous
preview path built a full-image pixmap just to read the border.

In the development app, reopening generated 3840×2160 images
with Balance and an eight-point inset reduced measured peak
footprint by 8.5–11.3 MiB. The fixture covered opaque and
translucent borders in light and dark.

The median isolated inset-color calculation fell from 11.30 ms
to 0.96 ms for the opaque border and from 13.76 ms to 1.14 ms
for the translucent border. All 21 paired runs per border type
returned identical colors. These timings cover sampling and the
former pixel conversion.

The light and dark checks passed for fractional frames, scaled
wallpaper and Balance effects. The scaled wallpaper check
compared preview and saved pixels at 1.25× and 2× with transparent
content. The unit regression compares cropped and uncropped inset
colors across 24 alpha combinations. All 384 targeted GPUI tests
and 16 Electron reference tests passed.
[Inset preview measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS AVAssetReader 4K60 decoder experiment

A standalone AVAssetReader probe loaded a generated 4K60 H.264
clip while two independent software encoders ran. Median decode
and transfer time for 180 frames was 2.16 s for the existing FFmpeg
route, 0.79 s for AVAssetReader BGRA, and 0.63 s for AVAssetReader
NV12. The native arms copied each frame into reusable memory.
[Apple AVAssetReaderTrackOutput documentation](https://developer.apple.com/documentation/avfoundation/avassetreadertrackoutput)

AVAssetReader BGRA differed from the current decoder in 26–30% of
sampled bytes. NV12 matched on both limited- and full-range H.264.
Passing that NV12 through a separate bundled FFmpeg conversion
reproduced the reference BGRA exactly for both fixtures. The
two-process bridge had a 229–242 ms maximum frame wait under load,
compared with 89–91 ms for the current route in the paired trials.

A variable-rate fixture with a 0.5 s leading edit gap also exposed
an extra black sample at zero in AVAssetReader. Starting the reader
at the first nonempty track segment reproduced all 12 reference
NV12 frames and timestamps in that fixture. Seeking to a point
between two samples returned the expected earlier pixels, with the
timestamp clipped to the seek boundary. The rotated 4:3 sample-aspect
ratio fixture exposed its transform and aspect ratio separately
from the coded pixels.

This was a standalone experiment. The app retains its current
decoder. Sustained 4K60 editor playback under load remains open.
Twelve GPUI decoder tests and 58 Electron reference tests passed.
[AVAssetReader probe measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS video sample aspect ratio and rotation

The shared FFmpeg decoder now scales non-square pixels to their
display geometry before the editor composes preview and export.
It expands the affected axis and applies the stored orientation
to the resulting dimensions. Square-pixel sources use the
original conversion filter. Oversized aspect-ratio expansions
are rejected using the existing image allocation budget.

Four H.264 fixtures went through the GPUI recording preview,
paused editor and MP4 export using app-local commands and a
fixture daemon. The output dimensions matched Electron's video
metadata in each case.

| Source SAR and rotation | Old preview and export | GPUI preview and export | Electron video |
| ----------------------- | ---------------------: | ----------------------: | -------------: |
| 4:3, upright            |              144 × 108 |               192 × 108 |      192 × 108 |
| 4:3, 90°                |              108 × 144 |               108 × 192 |      108 × 192 |
| 3:4, upright            |              144 × 108 |               144 × 144 |      144 × 144 |
| 3:4, 90°                |              108 × 144 |               144 × 144 |      144 × 144 |

Each paused preview matched every RGB byte of the corresponding
normalized source frame. The decoded H.264 exports had mean RGB
differences of 1.96–3.36 levels from the previews. Both light and
dark ran in the fixture. The native decoder tests also covered
repeated seeks and a fractional 182.5 px display dimension,
which rounds to 183 px as in Electron. Twenty-four GPUI tests
and 58 Electron reference tests passed.
[Video display geometry evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS recording thumbnail borrowing experiment

A candidate resized recording thumbnails directly from the decoder's
BGRA buffer. Idle, hover and exit screenshots matched the current
path byte for byte in 18 offscreen pairs. The fixtures covered 4K,
non-square-pixel landscape and rotated portrait in light and dark
at 2×. The recording thumbnail uses the shared video decoder.

At compile optimization level 3 the isolated candidate saved 31.6
MiB in peak footprint and cut resize time by about 5 ms. With the
development app's optimization level 1, the isolated borrowed resize
took about 95–98 ms against 35–38 ms for the existing path. A separate
size-optimized caller also regressed. These standalone variants
linked the same optimized development image dependency.

In four paired app-local 4K Stop tests, the candidate delayed first
thumbnail readiness by 46–92 ms. The second pair per appearance
reversed test order and traced FFmpeg delivery. Initial frame
payloads finished in about 13–16 ms in both variants. The known
performance cost is in the thumbnail stage. The app kept its
existing resizer. All 47 GPUI capture-preview tests, four still-image
pin tests and 54 Electron reference tests passed.
[Recording thumbnail experiment](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS first recording thumbnail

The first recording thumbnail asks for the video's first decoded
frame. The Unix decoder can return it as soon as its bytes arrive
and keep the stream for playback. Timeline seeks still verify the
requested timestamp through the usual lookahead path.

In six paired app-local 4K trials, median Stop-to-thumbnail
readiness fell from 174.3 to 134.5 ms. Every pair improved.
A separate gate withheld the second decoded frame. The thumbnail
still appeared and the recording reached History after release,
in light and dark. Six offscreen PNG pairs for idle, hover and
exit matched byte for byte with a static 4K source.

The timings stop at thumbnail readiness in the app. These trials
used a fixture daemon and app-local input. The 61 GPUI decoder and
preview tests and 54 Electron reference tests passed.
[First recording thumbnail measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS editor opening with a delayed camera track

The screen preview can open while the camera sidecar is reading
metadata. Camera warmup can queue on the pending decoder. A frame
that shows the camera waits for the decoder, and a superseding
paused seek can interrupt that wait while retaining the decoder.

In a fixture with the camera segment starting at one second, the
baseline stayed on its loading state during a 400 ms metadata gate.
The updated editor showed the screen at 133–152 ms with the gate
still closed. Seeking into the camera segment held the last complete
image, then seeking back refreshed the screen before release.
After release, the screen-only and camera-visible snapshots matched
the baseline byte for byte in light and dark.

This exercised generated video files with a fixture daemon. The
gate timings measure app readiness and deliberately delay camera
metadata. Twenty-seven GPUI tests and 58 Electron reference tests
passed.
[Delayed camera track verification](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS scrubbing past delayed camera metadata

Moving the scrub or hover position can now interrupt a pending
camera metadata wait. The pending camera decoder survives and
ordinary frame decoding can still finish during the gesture.
Once metadata is available, the editor can use the camera frame
even if a newer scrub position is queued.

In a 320×180 fixture, a camera segment from 1.0 to 2.0 seconds
was held at its FFmpeg metadata read. After the screen frame
at 1.2 seconds decoded, the app-local scrub or hover moved to
0.5 seconds. The updated editor showed the screen-only frame
in 45–59 ms while the camera remained held. The baseline stayed
on the preceding image throughout a deliberate 400 ms hold.
After release, the screen-only and camera-visible snapshots
matched across both builds in light and dark.

The fixture used generated video files and measured app readiness.
It exercised a scrub in progress and timeline hover. All 190
targeted GPUI tests and 48 Electron reference tests passed.
[Scrub and hover measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS playback past delayed camera metadata

Playback and composition share the same camera visibility check.
If the playhead reaches a screen-only section while camera
metadata is pending, the editor releases that wait and renders the
latest request. The decoder stays available when the camera returns.

A generated 320×180 clip played through a camera segment ending
at 0.8 seconds in light and dark. With the metadata gate still
closed, the updated editor resumed at a playhead around 0.801 s.
In the repeat after the build finished, the resumed frame arrived
804–805 ms after Play. The baseline held its previous frame until
the probe released metadata at 1.2 seconds. Paused screen and
camera frames matched the baseline byte for byte after release.

The times measure ready frames in the app using a fixture daemon.
All 178 GPUI tests and 48 Electron reference tests passed.
[Playback and delayed camera evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS playback past a delayed camera frame

The editor can now stop waiting for a camera frame when playback
leaves its segment. It can also stop waiting while that request is
still queued behind camera warmup. The decoder can serve later
camera frames when playback or a seek returns to the segment.

In a generated clip with a camera segment ending at 0.8 seconds,
the updated preview resumed at 802–813 ms after Play in light and
dark with camera frame delivery deliberately held. The baseline
waited for the gate to open at 1.2 seconds. After release, paused
camera and screen snapshots matched the baseline byte for byte.

This app-local fixture held decoded frame delivery after the camera
metadata query. The timing ends at the app's frame update.
All 191 targeted GPUI tests and 48 Electron reference tests passed.
[Delayed camera frame measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS camera decoder cleanup after the last segment

The editor now cancels camera warmup, releases its cached camera
image and retires the FFmpeg frame stream when the camera is
disabled or its last visible segment has ended. It keeps the
decoder's metadata so a later seek can open a fresh frame stream.

In light and dark tests with a 1080p camera sidecar, the previous
FFmpeg camera process held about 230 MiB after playback ended.
The revised process exited, and the editor's own footprint stayed
near 246 MiB in both builds. The worker also exited when the
fixture deliberately held its camera warmup before the first frame.

Seeking back to the camera segment opened a new frame stream.
Camera and screen snapshots matched the baseline bytes in all
normal and held cases. The fixture used generated video files
and app-local controls. All 191 targeted GPUI tests and 48 Electron
reference tests passed.
[Camera decoder lifetime measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS decoder cleanup at the timeline end

The video editor releases the decoded source frames after composing
the final paused image. The displayed image stays available while
the editor is idle. Replay and subsequent seeks can open the frame
stream again.

A 4K H.264 fixture had three seconds of source video trimmed to a
one-second timeline. After the timeline ended, the baseline screen
decoder still held 882–884 MiB. The revised app reaped the FFmpeg
process in both appearances after each of two Play runs. The
editor process itself measured 422–470 MiB across the updated
samples. Final frames and a paused seek after release matched
baseline PNGs byte for byte. App-local replay also published
frames earlier than the end frame and released the decoder again.

These results used a generated clip and a fixture daemon. All
190 targeted GPUI tests and 48 Electron reference tests passed.
[End-of-timeline decoder measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K decoder thread and memory measurements

A four-input-thread FFmpeg candidate lowered parked decoder
footprint from about 908 MiB to 632 MiB in a generated 4K60
H.264 test. It improved the first-two-frame time in the standalone
socket test. Slice-only decoding used less memory and was slower
over the full batch.

The candidate saved roughly 278 MiB in the GPUI development
editor too. Across six app-local pairs, it tied the baseline
frame count twice and presented fewer frames in four pairs.
Its opening PNG matched the baseline in every pair. The thread
limit remained an experiment because steady playback did not improve.

A stage trace put the slowest request at 1,304 ms in the decoder
wait and 12 ms in the rest of composition. Twelve raw frames came
through during that wait, with the longest payload taking 234 ms.
These measurements cover a generated clip through the app and
measure applied preview images. Sustained 4K60 editor playback
under encoding load remains open. Nineteen GPUI decoder tests
and 48 Electron reference tests passed.
[Decoder memory and stage measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS decoder cleanup for a hidden paused editor

The paused video editor retires its preview decoder when an
onscreen window is hidden or minimized. It keeps the composed
preview. AppKit reports a covered window as still onscreen,
so the editor keeps its decoder available through ordinary
occlusion. [AppKit visibility documentation](https://developer.apple.com/documentation/appkit/nswindow/isvisible)

In light and dark 4K fixtures, hiding and minimizing each
released an FFmpeg process that held 882–884 MiB in the baseline.
Showing the window presented the existing preview without
starting another decoder. The displayed image and the next
seek matched the baseline bytes. The repeat measured the first
seek after release at 547–731 ms, compared with 181–204 ms
while the baseline decoder was warm. A separate opaque fixture
covered the editor and confirmed that its decoder stayed open.

The tests measured app-local visibility and readiness with
generated media. All 190 targeted GPUI tests and 48 Electron
reference tests passed.
[Hidden editor memory and seek measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS 4K seek after decoder release

A cold FFmpeg seek can now discard frames before the nominal
preceding interval inside the filter graph. This avoids reading
those frames as raw BGRA. The editor still selects the last frame
at or before the target. If the filter misses that frame, the
decoder retries with full preroll.

The app-local fixture hid a paused 4K editor, confirmed the old
decoder exited, restored the retained preview, and sought from
0.35 to 0.75 seconds. Across six pairs in light and dark, the
raw frame payloads fell from 26 to four and all seek PNGs matched.
Median seek readiness fell from 448 to 277 ms. The baseline's
first pair ran during a test build. In the two later pairs, the
baseline took 397–455 ms and the updated build took 274–289 ms.

The fixture used generated H.264 footage with B frames. The
decoder tests covered exact fractional timestamps, sparse
screen and camera timelines, and a fallback that recovers a
held frame on a track with a nonzero start timestamp. All 195
targeted GPUI tests and 48 Electron reference tests passed.
[Cold seek measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS startup with Freeze Screen disabled

GPUI now checks the Freeze Screen preference before scheduling
startup prewarm. On macOS that call can take a display snapshot
when Screen Recording access is already available. The desktop
daemon still starts through its normal startup path.

The light and dark app-local probes saw one prewarm with either
preference in the baseline. The updated build sent zero prewarms
with Freeze Screen off and one with it on. A separate live-mode
area selection passed the selector's exclusion ID to the fixture
daemon in every case and saved byte-identical PNGs before and after.

The regression is available in the native capture history probe
with `--freeze-prewarm`. All 121 GPUI capture tests, 100 Electron
reference tests, and the Apple Silicon and Rosetta screenshot
geometry fixtures passed.
[Startup and live capture probe results](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS editor startup after daemon exits before ready

The native daemon's startup wait now observes both readiness
and exit for the process it launched. An early exit releases
the startup caller, so an editor opened from a file can appear
while the watchdog retries. Each early-exit retry remains owned
by the watchdog. A fixture that failed twice recovered on the
third launch.

Opening a generated screenshot with a daemon that exited on
its first launch took 10.6–11.4 seconds to produce an onscreen
editor window in the baseline. The revised build took
259–265 ms in a repeat after compilation. The daemon recovered
on its second launch in both appearances.

This measures WindowServer visibility, using a fixture daemon.
All 130 GPUI daemon and capture tests and 41 Electron reference
tests passed.
[Early daemon exit measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS file editor opens during daemon startup

Opening an existing file directly now starts the native daemon
on the background executor. The screenshot editor can open while
the daemon waits for Ready. Freeze Screen prewarm runs after
readiness when the preference is enabled. Intent-based startup
still waits for the daemon as before.

In light and dark, a fixture held the daemon's Ready event.
The baseline window stayed closed during a one-second hold.
The revised file editor became onscreen while Ready was still
held. In the repeat after the build finished, WindowServer
reported the revised window in 304–977 ms, compared with
1,293–1,335 ms for the baseline after its gate was released.
The updated Settings intent also waited for the gate.

The observation covers WindowServer visibility with a generated
PNG and a fixture daemon. The enabled and disabled Freeze Screen
probes passed in both appearances, along with 130 GPUI daemon
and capture tests and 42 Electron reference tests.
[Direct file startup measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS shutdown while the daemon starts

Shutdown now signals the in-progress daemon startup before taking
the lifecycle lock. The startup caller reaps the unfinished daemon
and releases the lock. A daemon that is already ready still receives
the graceful Quit request. The quit grace period applies when that
request was sent successfully.

The isolated macOS subprocess fixture held the daemon before
Ready. Stop cancelled the wait, the process was reaped, and the test
finished in 60 ms, including starting the fixture. The daemon and
capture suites covered the existing restart and shutdown paths.
All 131 GPUI tests and 42 Electron reference tests passed.
[Startup cancellation verification](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS direct screenshot file loading

Opening a screenshot through the file argument now uses the same
background image and History loader as opening from the preview.
The app creates and activates the editor after the prepared image
and saved state are available.

In light and dark, a generated 4K file opened with its History
read deliberately held. The baseline could not process the
startup prewarm callback during the 400 ms hold. The revised app
processed it within 11–14 ms of the fixture releasing the daemon's
Ready event, with History still held. Both editors opened after
the History read completed.

The direct-capture editor probe also passed in both appearances.
The GPUI editor and capture-preview suites passed 425 tests, the
Electron reference suites passed 59, and the non-parity GPUI
check passed.
[Background file-open measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS History thumbnail work after closing

Closing History now tells its background worker to stop before
starting another item. An already-started thumbnail can finish and
remain cached. Reopening History uses the cached thumbnails and
generates the missing ones.

In light and dark development runs with 24 generated 4K screenshots,
the old build generated 23 more thumbnails after closing History.
The revised build generated zero more in both trials. CPU time
during the two seconds after close fell from approximately 0.82 s
to 0.016 s. Physical footprint at the two-second mark fell from
68–70 MiB to 65 MiB. Reopening produced all 24 thumbnails with the
same JPEG bytes as the baseline.

The macOS probe used app-local commands and a temporary profile.
The GPUI History suite passed 43 tests, including cancellation
between two items, and the Electron History suites passed 81.
[History background work measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS decoding during hidden playback

Hiding or minimizing a playing video editor now drops its preview
decoders and stops offscreen composition. The playback clock and audio
transport still advance. On reveal, the editor retains the last image
until it has a frame at the current playhead.

In four paired 4K60 development trials, the old decoder held
884–886 MiB while the editor was hidden. The revised editor released
the decoder in all four trials. GPUI CPU time during a one-second
hidden sample fell from 679–699 ms to 3–7 ms. When restored, the new
preview reached the current playhead in 85–130 ms, and the final 4K
PNG matched the baseline bytes in every trial. A separate run during
compilation took up to 291 ms to refresh.

These runs used generated silent video, app-local commands and
preview-image readiness. The GPUI regression covers an in-flight
composition and playback ending while hidden. The targeted GPUI
and Electron reference suites passed 172 and 66 tests, respectively.
[Hidden playback measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS preview pixmap storage reuse

The video editor now writes successive decoded frames into the same
conversion buffer when it owns that buffer and the dimensions match.
It allocates a fresh buffer when a caller still holds the prior image.
Preview and export use the same BGRA-to-RGBA conversion.

An isolated 120-frame 4K test eliminated 3.71 GiB of cumulative
allocation requests after the first pixmap. Median conversion time
fell from 776 ms to 750 ms across seven paired batches. Both paths
produced identical pixels.

The visible 4K60 editor applied 71–121 preview updates over two
seconds with two concurrent encoders, compared with 86–120 for the
baseline. Composition at the same 384×216 active preview size was
faster in each loaded pair. Cadence and process footprint varied.
Sustained 4K60 playback under contention remains open. All opening
4K preview PNGs matched across the light and dark runs. A split and
export produced identical decoded checksums for the first 12 4K
frames in both builds.

The targeted GPUI and Electron reference suites passed 422 and 66
tests, respectively.
[Preview allocation and timing measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS Accelerate video frame conversion

The video editor and exporter now use macOS Accelerate to reorder
decoded BGRA pixels into the shared RGBA composition buffer. An
isolated 100-frame 4K conversion took a median of 120 ms with
Accelerate, compared with 964 ms using the prior Rust loop across
seven paired rounds. Both produced identical pixels. The native
helper matched all four channels at odd and even row widths on
Apple Silicon and Rosetta.
[Apple Accelerate documentation](<https://developer.apple.com/documentation/accelerate/vimagepermutechannels_argb8888(_:_:_:_:)>)

In four paired two-second playback samples with two competing
software encoders, both builds applied 119–121 preview updates.
The updated editor primarily used 432×243 to 640×360 previews,
up from 384×216 and with lower GPUI CPU time in every pair. Without
the test encoders, the updated editor reached 853×480 in each trial.
The process footprint varied across trials.

The 18-second stress sample varied by run. The updated build applied
1,079 of 1,080 expected light-mode updates with a 27 ms maximum gap.
In the dark repeat it applied 523 updates, with a 2.83 s maximum gap.
The longest decode request there took 2.826 s, including a single
raw frame that took 1.616 s to finish after the first byte arrived.
Sustained 4K60 playback under contention remains open.

Opening previews matched across all runs in light and dark. A
split-and-export check matched the first 12 decoded 4K frames.
The targeted GPUI and Electron reference suites passed 423 and 66
tests, respectively.
[Frame conversion and playback measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS raw video socket buffer sweep

Five rotated 180-frame batches with two competing encoders found
that a 6 MiB send buffer lowered the median longest raw transfer
from 69 to 38 ms. It increased the unloaded median total from
2.29 to 2.42 s. Larger receive buffers and 32 KiB read and write
wake-up thresholds also produced mixed results.

In the native editor, the 6 MiB build produced fewer updates in
both two-second runs with fresh encoders. It produced more updates
in both 18-second runs, though the gaps still reached 2.83 s.
With the same encoders running continuously across four additional
pairs, the larger buffer produced one to three more updates in
each two-second sample and a longer measured gap in three pairs.
The experiment used the current 4 MiB default for the final
development binary. Opening preview pixels matched in both themes.
The decoder transport remains an open performance issue.
[Socket timing and frame evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS idle memory after opening a video editor

After composing its first paused preview, the editor now releases
the screen and camera decoder processes and retains the source
pixmaps for same-position redraws. Playback starts a decoder when it
needs the next frame.

Four paired light and dark 4K60 runs released an FFmpeg process with
about 880 MiB of physical footprint. The editor's own footprint was
486–495 MiB. The opening and same-position redraw PNGs matched byte
for byte. Same-position redraws took 69–72 ms after release and did
not start a decoder.

The first preview update after Play took 95–97 ms, up from 22–23 ms
with a resident decoder. These app-local timings measure preview
image readiness with generated media. The GPUI video editor suite
passed 174 tests, the development FFmpeg camera fixture passed, and
66 Electron reference tests passed.
[Paused opening memory and timing measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS paused video editor after losing focus

A visible macOS video editor now releases its decoder processes
when paused and inactive. It keeps the displayed preview and the
source pixmaps. Playback can continue while the visible editor
is inactive. The decoder stays available during a foreground pause.

An AppKit focus change to Recent Captures verified both orders
in light and dark. Pausing before switching focus and pausing after
switching both released an FFmpeg process that held about 883 MiB
in the baseline. Each paused preview matched its own pixels
through focus loss and regain. Refocusing kept the decoder
released until Play.

The first applied image after resuming took 114–118 ms in the
updated build and 21–23 ms with the resident decoder. These
timings use app-local commands and generated 4K60 media. The
GPUI editor suite passed 174 tests and the Electron reference
suites passed 66.
[Paused background memory and timing measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS tooltips when the owner fills the work area

A protected tooltip now uses the side of the pointer with room
when it cannot fit outside its owner. The previous bottom-edge
placement covered the pointer by six points in a narrow fixture.

The real AppKit panel kept an 18-point gap at both the top and
bottom edges of the simulated work area in light and dark. It
remained visible inside the work area, ignored pointer events,
and unregistered on dismissal. The existing tall-owner side
placement retained its four-point owner gap.

Focused and complete Swift fixtures passed on Apple Silicon and
Rosetta, as did both daemon typechecks and 138 Electron reference
and daemon contract tests. The probe used app-local coordinates
on the built-in display.
[Tooltip fallback evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-19 macOS ellipsis in narrow protected tooltips

The native fixture renders an unbroken English token and Hebrew
text at 2× in an injected 104-point work area. The tooltip fits
the area with a label at most 80 points wide. The fourth line
paints an ellipsis on the right for English and on the left for
Hebrew, in both light and dark.

The regression reads the label pixels and detects the three dots
at the reading end. Disabling truncation in the test panel makes
the check fail, so the result verifies the rendered glyphs.
[Native ellipsis verification](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS rapid recording device-menu toggles

A second press of the same device button now cancels a menu that
is waiting for the macOS toolbar resize. The old path opened the
menu when the 300-point viewport acknowledgement arrived.

The regression delivers the late resize and verifies that the
menu stays closed and sends no device list request. A later single
press still opens it after the resize. All 36 recording-control
scene PNGs matched byte for byte between builds across light and
dark at 2× on the macOS Metal renderer. The fixtures used generated
devices. All 21 targeted GPUI and 82 Electron reference and
daemon contract tests passed.
[Device menu evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS recording device refresh without a menu rebuild

An identical device response now leaves the recording menu instance
in place. Microphone and camera replies compare the requested list
and its system default. The connected iPhone and iPad list also
skips unchanged replies. This avoids resetting the menu's keyboard
state when it already has the requested entries.

The regression failed on the old code for an identical list. It
now confirms that a default-only change still refreshes the menu
and that stale replies are ignored. The 2× macOS Metal fixture
rendered 18 light and 18 dark scenes. All corresponding PNGs
matched byte for byte. All 22 targeted GPUI tests and 82 Electron
reference and daemon contract tests passed with fixture devices.
[Recording-device refresh evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS disconnected iPhone and iPad recording choices

When device enumeration succeeds and the selected iPhone or iPad
is absent, GPUI now clears the saved choice and checks None in the
recording menu, matching Electron. Failed lookups preserve the
selection. Starting a recording invalidates pending device replies
so a late refresh cannot change the requested target.

The macOS Metal fixture reproduces the disconnect using a saved
generated iPad and a successful empty device list. The old toolbar
retained the saved iPad in both themes. The regenerated menu shows
the None checkmark at 2×. The 36 existing light and dark scenes
matched byte for byte. All 22 GPUI tests and 82 Electron reference
and daemon contract tests passed.
[Disconnected-device evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS recording device menus after an enumeration error

A failed microphone or camera lookup now keeps the last successful
device list and system-default label in the recording menu. A
successful empty response still removes the corresponding options.

The native AppKit probe opened a light microphone menu and a dark
camera menu with generated devices. Each received a successful
populated list, an injected error, and a successful empty list.
The old error response rendered the same menu pixels as the empty
list. The revised error response matched the populated menu.

All 38 existing light and dark Metal scenes matched byte for byte.
The targeted GPUI suite passed 23 tests and the Electron reference
and daemon contract suites passed 82. The probes used fixture
devices and app-local input.
[Device enumeration error evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS window recording under the toolbar and tooltip

The current GPUI development app recorded a generated AppKit window
in light and dark through the existing preflighted screen-only
daemon. The protected tooltip was visible over the target during
the green marker phase. The same target then moved beneath the
visible recording toolbar for the yellow phase.

Both 1280×880 source recordings contained 21 frames with strictly
increasing timestamps. The tooltip and toolbar regions stayed
byte-identical to the backdrop. Each take opened from the normal
recording preview, saved a split and exported 82 frames at the
same size. Both sampled regions matched the GPUI preview byte
for byte in every decoded export frame.

The probe used app-local Start, Stop and editor commands, and
injected the tooltip request without moving the desktop pointer.
It removed the generated trial's cursor sidecar before opening
the editor to isolate the source pixels. The daemon was the
earlier authorized screen-only fixture. All 82 Electron reference
and daemon contract tests passed.
[Window recording and export evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS target closure during a paused window recording

On the built-in Retina display, the current GPUI development app
recorded a generated AppKit window in light and dark using the
earlier authorized screen-only daemon. It paused before the target
closed and released its actual window object.

Each closure produced one target-closed event. GPUI sent Stop
automatically within 265–272 ms and removed the toolbar and outline.
The first probe sampled the visible preview after waiting for a
diagnostic. The immediate-polling follow-up below measures the
handoff separately. The recordings opened in the editor, saved a split at 0.45 s, and
exported 58 frames each at the original 720×480 resolution. Both
source movies and both exports retained the three scene stages
with strictly increasing decoded timestamps.

The fixture used app-local commands, an existing screen grant and
generated content. It supplied neither microphone nor camera nor
system audio. All 82 Electron reference and daemon contract tests
passed.
[Paused target-closure evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS paused Stop to preview timing

The earlier closure probe waited for a one-second diagnostic
before sampling WindowServer. Its 1.12 s value was an observation
after that wait. This rerun starts polling immediately and measures
the Stop request, reply, visible window and ready thumbnail
separately.

Across eight paused-window trials, the preview window appeared
104–174 ms after the daemon's Stop reply. From the original
trigger, manual Stop took 130–240 ms to show the window and
target closure took 527–854 ms. The automatic Stop request
itself followed closure by 371–656 ms.

The first decoded thumbnail became available 485–2,008 ms after
the window appeared. These results use app-local commands and
WindowServer polling, with an earlier authorized screen-only
daemon and generated content. Host load varied and reached a
one-minute load average of 547 during the run. All eight previews
opened in the editor, and all 82 Electron reference and contract
tests passed.
[Per-run Stop and preview timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS recording thumbnail copies after resize

The preview now moves the resized buffer into the screenshot
or recording thumbnail. An unblurred recording thumbnail already
has GPUI's BGRA channel order and skips the two channel-swap
passes. Hover blur still uses the same RGBA pipeline.

Allocation tracing measured 112,000 fewer allocated bytes per
recording thumbnail. The regression compared the crop and alpha
with the original resize over seven landscape, portrait, and
small-frame shapes. All six 2× Metal images for idle, hover and
exit in light and dark matched byte for byte.

The four paired GPUI Stop-to-thumbnail runs varied across both
builds. They do not establish a timing improvement in the full
handoff. The existing resize path also avoided a repeat of the
borrowed-buffer slowdown. The 48 GPUI preview tests and 70 Electron
preview and thumbnail tests passed.
[Thumbnail allocation measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS decoder memory for covered recording previews

A recording preview fully covered by another window now releases
the decoder's buffered frames. Cards outside the scrollable
viewport also release their decoder buffers. They retain the last
thumbnail and resume decoding when visible again.

A generated AppKit cover left a partially visible preview playing
and then fully covered the same window. The updated light and
dark 4K previews released a decoder that had used about 880 MiB.
During a one-second covered sample, GPUI CPU time fell from
231–235 ms to 1.2–1.5 ms. The new decoder delivered frame data
162–221 ms after the cover was removed.

The clipped-card regression now checks release and playback
resumption. All six 2× idle, hover and exit images matched across
builds. The 48 GPUI preview tests and 70 Electron reference tests
passed. These results use generated media and an app-local cover.
[Covered preview measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS visibility notifications for covered recording previews

A fully covered preview now waits for an AppKit visibility change
instead of polling on each 125 ms playback tick. The same
notification resumes decoding when the window becomes visible.
The viewport check still handles cards scrolled inside a visible
window.

Across four paired native runs, GPUI interrupt wakeups over three
seconds of coverage fell from 32–40 to 11–25. GPUI CPU time dropped
from 3.8–4.7 ms to 1.1–2.7 ms. First-payload timing after uncover
varied across pairs. The regression verifies resumption without
advancing the playback timer.

The six light and dark 2× Metal scenes matched byte for byte.
All 48 GPUI preview tests and 70 Electron reference tests passed.
[Covered preview notification timings](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS auto-dismiss after uncovering a preview

A fully covered preview now cancels its auto-dismiss countdown.
It receives a fresh delay when AppKit reports it visible, matching
the existing GPUI policy for cards scrolled out of view. A thumbnail
that finishes while covered stays eligible for the delay on reveal.

In native light and dark runs, an opaque AppKit window covered the
preview for 4.6 s with a four-second configured delay. The baseline
card had already closed when uncovered. The revised build showed
the same preview window after uncover and removed it about 4.52–4.54 s
later, including the exit animation and WindowServer sampling.

All 49 GPUI preview tests and 70 Electron reference tests passed.
The fixture used a generated screenshot and app-local commands.
[Covered preview dismissal evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS preview hover tooltips after occlusion

A covered preview now clears pointer state and cancels the native
tooltip request when AppKit reports the window hidden. A fresh
hover can request the tooltip after the preview is visible.

In light and dark native-probe runs, app-local hover requested
Close preview before an opaque AppKit window covered the card.
The baseline kept the request through cover and uncover. The
updated app cleared it during coverage and restored it only after
a new hover. The simulated GPUI regression also covers a tooltip
still waiting for its hover delay when the window becomes hidden.
All 50 GPUI preview tests and 70 Electron reference tests passed.
[Covered tooltip evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS tooltip cancellation during daemon startup

GPUI now checks that a tooltip Show is current before starting the
daemon and again after startup. This covers a Hide that arrives
before there is a daemon to receive it. The existing request
sequence still orders requests at the daemon.

The regression held a Show before startup, issued Hide while the
daemon was absent, then let startup complete. The old path sent
the stale Show. The updated path sent no tooltip request. Separate
light and dark app-local runs held a Show reply in the fixture
daemon. Hide still reached it in 14–17 ms with the newer sequence.
All 11 GPUI daemon tests and 130 Electron reference and contract
tests passed.
[Tooltip startup and dispatch evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS full local regression and preview timeout clock

The full repository check found a camera-preview timeout using
a timer outside GPUI's clock. The supersedable frame request now
uses the GPUI background executor's ten-second deadline. A test
holds the decoder response, advances the simulated clock to the
deadline, and verifies that the request is abandoned. The existing
supersession test still verifies immediate cancellation.

The local check passed 2,418 Electron tests with three skipped.
The full GPUI suite passed 1,217 tests with 24 ignored. Swift scroll
and protected-tooltip fixtures passed on Apple Silicon and Rosetta.
The synthetic recording timeline, timer lifecycle and timer
accessibility fixtures passed on Apple Silicon. Windows-native
checks remain assigned to Windows CI.
[Local regression evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS full-display recording exclusion with generated windows

A two-times-scale recording in light and dark captured the full
3456 × 2234 display. The temporary recorder admitted the generated
scene and recorder controls by window ID, then applied the app's
normal exclusion IDs. The GPUI toolbar and protected AppKit tooltip
were visible on the display. Their regions matched the captured
backdrop in both themes. A regular window moved to the exact tooltip
bounds appeared in the next phase.

Each take opened in the video editor, saved a split and exported.
In the tooltip region of the first decoded export frame, the largest
RGB difference from the editor preview was two levels. The
normal desktop-wide filter still needs a full-frame verification.
[Full-display exclusion evidence](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS idle and video editor wallpaper memory

The video editor requests the desktop wallpaper when its Wallpaper
panel first opens. The tile uses a scaled preview and selecting the
wallpaper still uses the full-resolution source. A failed request can
be retried without blocking the UI thread.

In light and dark runs with a generated 4K wallpaper and 720p video,
the paused editor used about 128 MiB of physical memory with the
Wallpaper panel unopened, down from 216–232 MiB. With Wallpaper
open, usage fell from about 255 MiB to 222 MiB. The deferred path
issued zero wallpaper requests with the panel unopened and one
when it opened. A simulated GPUI regression covers reopening a
sidebar whose remembered tab is Wallpaper.

In eight-second tray-only samples with the Swift daemon, GPUI
used 18–19 MiB of physical memory and the daemon about 8 MiB.
Combined CPU time stayed at or below 0.04 ms per sample across
both builds. The paused editor samples showed no consistent
change in steady-state CPU time.
[Idle and wallpaper measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS paused video editor audio memory

Audio clips at normal speed now share their source's decoded PCM
with the other clips from that source. The audio player supplies
leading silence as it reads, and speed changes still use the same
stretch algorithm as export. Preview and export use the same trim
and placement rules.

With two generated three-minute audio tracks on the M5 Pro, the
paused editor used 275–277 MiB before and 175–177 MiB after, with
one clip and with nine clips in light and dark. The no-audio
control was about 108 MiB in both builds. Paused CPU showed no
consistent change. Sample comparisons verified leading silence,
trims, speed changes and seeks. Four generated-media tests also
passed for sidecar audio, volume at a cut, first-frame audio timing
and replacing a music file.
[Paused audio measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS playback under an occluding window

The video editor now follows AppKit's occlusion notifications.
A fully covered editor keeps its last composed image, stops preview
composition and releases the decoder. The playback clock and audio
transport continue. Once the window is visible again, the editor
composes at the current playhead. Partial coverage leaves preview
playback running.

In native light and dark runs with a generated 4K clip, full
coverage released an FFmpeg decoder holding about 876 MiB. The
baseline applied 37 offscreen updates per 1.2-second sample and
used 34–38 ms of combined GPUI and decoder CPU time. The updated
build applied zero and used about 0.1 ms of GPUI CPU time. The
retained preview was available on reveal. A fresh frame arrived
134–335 ms after reveal, up from 15–19 ms with the warm decoder.

The regression drives GPUI's visibility callback through a
normal editor window and verifies preview retention, playback
continuity and final-frame recovery. All 1,219 runnable GPUI
tests and 17 Electron reference tests passed.
[Occluded playback measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS wakeups during covered video playback

When a covered editor has no audio output, it keeps its wall-clock
playback position and sleeps until the timeline ends. Revealing the
editor refreshes the preview at the current position. Seeking,
pausing or editing the duration updates the pending playback
deadline. If audio becomes ready while the editor is covered, its
transport resumes at the current position and keeps its normal
synchronization cadence.

With a fully covered 720p editor and no audio track, GPUI woke
six times in each 1.5-second sample, down from 50–52 in light
and dark. The first frame after reveal arrived in 55–56 ms
in this fixture, compared with 41–52 ms in the baseline.
An embedded audio track retained the normal cadence. A separate
probe held audio decoding and released it while the editor was
covered. The transport cadence resumed before reveal.

The GPUI regression also covers pause, seek and a shortened
duration while covered. The full GPUI suite passed 1,220 tests.
The Electron player, music and export reference tests passed 17.
[Covered playback wakeup measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS wakeups while waiting for FFmpeg

Audio decoding and video metadata reads now wait for a macOS
process-exit notification or their existing deadline. GIF export
continues to check for cancellation. A timeout or cancellation
still terminates the child process group and drains the readers.

With a generated video project open and audio decoding held,
GPUI registered three wakeups in each one-second sample, down
from 144–145 in light and dark. The sidecar decoded successfully
after release in both builds. A separate child-process probe
registered zero wakeups during the wait, down from 72–75, and
preserved stdout, stderr and the exit status.

The full GPUI suite passed 1,223 tests. Four generated-media
tests and 28 Electron player, audio and export tests also passed.
[FFmpeg wait measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS full-display ScreenCaptureKit exclusion

A native 3456 × 2234 ScreenCaptureKit stream used the normal
display exclusion filter, with only the visible GPUI recording
toolbar excluded at startup. The native AppKit tooltip appeared
during the stream and the production updater added its window ID.
In light and dark, the toolbar and visible tooltip regions matched
the generated backdrop. A regular window placed at the tooltip's
exact bounds appeared at the same point in the stream.

A second pass also excluded the other existing desktop windows.
That pass verified the whole generated backdrop outside a small
upper-right region and the ordinary window's edge. The probe
retained aggregate pixel counts. The normal filter now has direct
stream evidence. The end-to-end full-display record and export
evidence above uses generated-window isolation.

All 82 Electron recording-control and daemon contract tests
passed. [Full-display stream measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS scroll duplicate comparison experiments

Two independently captured 3200 × 2000 images of a generated
AppKit window compared equal. After a colored marker changed,
the next image compared different in both builds. The capture
used ScreenCaptureKit with the existing Screen Recording grant
and retained only comparison results and timings.

Comparing normalized bands reduced peak memory and took longer
on identical generated frames. Comparing packed bytes made
synthetic duplicates faster. The direct ScreenCaptureKit frames
had 8 KiB of additional backing storage, so the guarded packed
path deferred to the existing comparison. The native medians
were about 16 ms in both builds. The experiments were reverted.

The arm64 and Rosetta Swift fixtures passed with the candidate,
including the fixed-header seam and accepted-prefix checks.
All 62 Electron scroll and daemon contract tests passed.
[Duplicate comparison measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS opaque screenshot Save performance

An unedited opaque RGBA screenshot can now pass its source pixels
directly to the sRGB PNG encoder. Transparency, annotations,
attached images, wallpaper effects and JPEG continue through
the composition path.

In light and dark native editor runs, the warm median for saving
a generated 3840 × 2160 PNG through the background writer fell
from 51.9 ms to 35.0–35.7 ms. Peak physical footprint fell by about
62 MiB in both appearances. All outputs matched byte for byte.

The native Save sheet opened and canceled three times per theme.
It kept the fixture directory and separate filename. The annotated
writer and reopen check matched all 1,790 edited pixels in light
and dark. The available probe did not confirm the native sheet.
[Save and memory measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 native macOS narrow video editor and audio tracks

The normal editor entry point restored a saved 720 × 520-point
window on the built-in Retina display. In light and dark, the
long generated title truncated before the toolbar actions, the
annotation stayed visible in the preview, and Export Video fit
in the sidebar with a tall saved timeline.

The native run exposed an Audio lane on a recording with no
audio stream. The editor now uses FFmpeg's existing metadata to
distinguish embedded audio from a silent movie. Saved track
settings remain in the project. When the program audio is known
to be absent, the editor omits its lane and Audio panel row and
disables scrubbing audio. A valid sidecar supplies its own
System Audio lane.

Both themes verified silent movies, a saved legacy audio entry,
embedded AAC and a separate AAC sidecar. All 1,225 runnable
GPUI tests, four generated-media tests and 18 Electron reference
tests passed. The window capture targeted only the generated
editor and used no desktop input.
[Light Audio panel](tests/native/evidence/macos-ui/video-native-narrow-light-audio-20261120.png)
[Dark Export panel](tests/native/evidence/macos-ui/video-native-narrow-dark-export-20261120.png)
[Native measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS audio decoder startup for silent recordings

The editor waits for existing FFmpeg metadata before opening an
embedded-audio fallback. A silent movie skips that decoder.
Separate system and mic files remain available, and a genuine
embedded stream starts decoding as soon as metadata is ready.

In native light and dark runs, silent movies went from two PCM
decode launches to zero. Mic-only projects went from two failed
movie reads and two mic reads to one successful mic read. Separate
system audio used one read in both builds. Embedded AAC decoded
successfully with one launch in the updated build. Paired follow-up
samples put the first composed preview at about one second in both
builds, with no consistent change in readiness time.

All 1,226 runnable GPUI tests, four generated-media tests and
28 Electron reference tests passed.
[Audio decoder startup measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 current macOS offscreen workflow regression

The loaded-editor fixture now supplies generated AAC for its
audio-lane contrast and Scrub Audio checks. The video used by
the fixture is silent, so the previous setup depended on the
phantom Audio lane. Missing bounds now identify the measurement
name and index.

All four macOS offscreen Metal profiles passed. Both 1280-point
profiles covered area, window and display recording handoffs and
decoded export comparisons, plus screenshot selectors, timer,
scroll, annotation and reopen. Both 720 × 520 Retina profiles
checked the loaded editor, keyboard access and horizontal
scrolling for Scrub Audio. The runs used generated media and
fixture daemons.

The repository gate passed 2,418 Vitest tests with three skips,
as well as the daemon contract, TypeScript, lint, format and
React Compiler checks. The Windows-native check skipped on macOS.
[Offscreen workflow measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

### 2026-11-20 macOS full-display recording with guarded pixels

The native GPUI app ran Start and Stop through the normal
full-display ScreenCaptureKit filter. The fixture supplied only
the app's toolbar ID at startup. The Swift daemon's live filter
update excluded the protected tooltip when it became visible.

The temporary recorder validated the toolbar and tooltip regions
as uniform generated colors before encoding. It built each writer
frame from those colors and a black background, keeping unrelated
desktop pixels out of the files. A regular window placed at the
tooltip's exact bounds was admitted in both themes.

Both 3456 × 2234 recordings opened from the GPUI preview, saved
a split and exported at their original resolution. The toolbar
and tooltip samples stayed within four RGB levels of the backdrop.
The first decoded export sample was within one level of the
paused editor preview. This verifies the control regions through
the native writer and GPUI export. An unmasked desktop recording
and export remains open.
[Guarded recording measurements](tests/native/evidence/macos-ui/native-capture-latency-20261118.txt)

## Review gate

| Review category                       | Result                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Scope reviewed                        | Capture/editor/recording behavior, shared themes, visual/workflow fixtures and dependency evidence. The initial review used `ac384ae`; later reviews covered tooltip client geometry and placement, Electron frozen-frame ownership and hide failure handling, along with the targeted workflows listed above.                                                               |
| Fixed                                 | The defects listed above, including click-position window picking, frozen selection lifecycle, failed timer cleanup, native selector client alignment, full-window QR detection and recording-tooltip overlap; workflow checks verify capture-before-release and exact output.                                                                                               |
| Rejected after validation             | A proposed ZXing license truncation was rejected: raw bytes from the exact pinned upstream revision match the bundled file, including its upstream appendix (SHA-256 `3f62881f0566227a24b12e5a754cc79f39aaa94883038e95c94812e1f50af42f`).                                                                                                                                    |
| Deferred major or dangerous refactors | None. The music asset lifecycle refactor is implemented. Scroll histories that yield identical frames remain ambiguous. Camera authorization and multi-display behavior remain open on macOS. Windows runtime behavior also remains open.                                                                                                                                    |
| Verification                          | Local checks, exact visual comparison, native workflows and image review are recorded above.                                                                                                                                                                                                                                                                                 |
| Blocked or unverified                 | The last full-matrix CI pass was at [`b8dd8672`](https://github.com/Porabuild/Poratake/actions/runs/43688781846); Windows packaging was skipped. Verify current PR-head checks before merging. NVIDIA baselines are stale. Remaining macOS live checks include live camera and mixed-DPI display transitions. Windows runtime and other platform gaps remain in the backlog. |

An earlier local Windows GPUI suite passed serially with 948 tests and one ignored. One
earlier parallel run exited with access violation 0xc0000005 in ntdll.dll while
three tests were unfinished. Those tests passed individually, and the video-
export group passed with four test threads. An earlier revision also recorded
the cancellation-and-retry test failing alone with the same exception. The
parallel run's unsymbolized crash dump contains stack-memory addresses from
Media Foundation and NVIDIA's H.264 transform; this is a lead, not an unwound
stack or a confirmed cause. The intermittent failure remains unverified.
