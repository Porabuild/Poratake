import errno
import json
import os
import shutil
import sys
from contextlib import contextmanager
from pathlib import Path
from queue import Queue
from struct import pack
from subprocess import PIPE, Popen
from tempfile import TemporaryDirectory
from threading import Thread
from time import monotonic, sleep
from zlib import compress, crc32


def fixture_daemon():
    root = Path(__file__).resolve().parents[2]
    clip = root / "src/main/app-gpui/src/ui/parity/fixtures/capture.mov"
    def chunk(name, data):
        body = name + data
        return pack(">I", len(data)) + body + pack(">I", crc32(body))

    width, height = 300, 220
    row = b"\0" + bytes([64, 112, 192]) * width
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", compress(row * height))
        + chunk(b"IEND", b"")
    )
    print(json.dumps({"event": "system:ready", "data": {"pid": os.getpid()}}), flush=True)
    for line in sys.stdin:
        request = json.loads(line)
        method = request["module"], request["method"]
        if method == ("system", "quit"):
            break
        result = {}
        if method == ("screenshot", "capture-area"):
            path = Path(request["params"]["path"])
            assert path.is_relative_to(Path(os.environ["PORATAKE_NATIVE_SMOKE_OUTPUT"]))
            path.write_bytes(png)
            result = {"path": str(path), "width": width, "height": height}
        elif method == ("screen-recorder", "start"):
            path = Path(request["params"]["outputPath"])
            assert path.is_relative_to(Path(os.environ["PORATAKE_NATIVE_SMOKE_OUTPUT"]))
            shutil.copyfile(clip, path)
        elif method == ("screen-recorder", "stop"):
            result = {"duration": 2.0}
        elif method == ("recording-overlay", "show"):
            result = {"visible": True}
        elif method == ("camera-preview", "setContentProtection"):
            result = {"protected": request["params"]["enabled"]}
        else:
            assert method in {
                ("freeze-screen", "prewarm"),
                ("recording-overlay", "hide"),
                ("camera-preview", "hide"),
                ("desktop-icons", "show"),
            }, method
        response = {"id": request["id"], "success": True, "result": result}
        print(json.dumps(response), flush=True)


@contextmanager
def held_history(path, contents="[]"):
    deadline = monotonic() + 12
    while True:
        try:
            writer = os.open(path, os.O_WRONLY | os.O_NONBLOCK)
            break
        except OSError as error:
            if error.errno != errno.ENXIO or monotonic() >= deadline:
                raise
            sleep(0.01)
    try:
        yield
    finally:
        replacement = path.with_suffix(".ready")
        replacement.write_text(contents)
        replacement.replace(path)
        os.write(writer, contents.encode())
        os.close(writer)


def drain(stream, replies):
    for line in stream:
        replies.put(line.strip())
    replies.put(None)


def verify(executable, recording=False, opening=None, mutation=None):
    with TemporaryDirectory(prefix="poratake-history-hold-") as directory:
        root = Path(directory)
        profile = root / "profile"
        captures = root / "captures"
        profile.mkdir()
        captures.mkdir()
        (profile / "config.json").write_text(json.dumps({
            "onboarding": {"completed": True, "skipped": True},
            "general": {"autoCheckUpdates": False, "playSoundOnScreenshot": False},
            "shortcuts": {},
            "appearance": {"mode": os.environ.get("PORATAKE_HISTORY_PROBE_THEME", "system")},
            "history": {"enabled": True},
        }))
        bridge = root / "daemon"
        bridge.write_text(
            '#!/bin/sh\n'
            'exec "$PORATAKE_HISTORY_PROBE_PYTHON" "$PORATAKE_HISTORY_PROBE_SCRIPT" --daemon\n'
        )
        bridge.chmod(0o700)
        app = Popen(
            [str(executable), "--native-smoke-recording"],
            stdin=PIPE, stdout=PIPE, text=True, bufsize=1,
            env=os.environ | {
                "PORATAKE_HISTORY_PROBE_PYTHON": sys.executable,
                "PORATAKE_HISTORY_PROBE_SCRIPT": str(Path(__file__).resolve()),
                "PORATAKE_DAEMON_PATH": str(bridge),
                "PORATAKE_CONFIG_DIR": str(profile),
                "PORATAKE_NATIVE_SMOKE_OUTPUT": str(captures),
                "PORATAKE_NATIVE_SMOKE_RECT": "650,410,300,220",
                "PORATAKE_NATIVE_SMOKE_FREEZE": "0",
                "PORATAKE_NATIVE_SMOKE_HISTORY": "1",
            },
        )
        replies = Queue()
        Thread(target=drain, args=(app.stdout, replies), daemon=True).start()

        def send(command):
            app.stdin.write(command + "\n")
            app.stdin.flush()

        def wait(prefix, timeout=10):
            deadline = monotonic() + timeout
            while True:
                line = replies.get(timeout=max(0, deadline - monotonic()))
                if line is None:
                    raise RuntimeError(f"GPUI exited before {prefix}")
                if line.startswith(prefix):
                    return line

        def command(action):
            send(action)
            return wait(f"NATIVE_SMOKE {action} ").endswith(" true")

        def eventually(action, *, expected=True):
            deadline = monotonic() + 10
            while command(action) != expected:
                if monotonic() >= deadline:
                    raise RuntimeError(f"{action} did not return {expected}")
                sleep(0.01)

        def capture_area():
            assert command("capture area")
            eventually("pointer down 650 410")
            assert command("pointer move 950 630")
            assert command("pointer up 950 630")

        def captured_files(count):
            deadline = monotonic() + 10
            while True:
                files = {path for path in captures.glob("*.png") if not path.name.startswith(".")}
                if len(files) == count:
                    return files
                if monotonic() >= deadline:
                    raise RuntimeError(f"expected {count} saved captures, found {len(files)}")
                sleep(0.01)

        history_path = profile / "history.json"
        preview_delete = mutation == "preview delete"
        try:
            wait("NATIVE_SMOKE ready")
            if opening or mutation:
                capture_area()
                eventually("preview ready")
                if mutation and not preview_delete:
                    assert command("open history")
                    eventually("history ready")
                contents = history_path.read_text()
                history_path.unlink()
                os.mkfifo(history_path)
                if preview_delete:
                    send(mutation)
                elif mutation:
                    send(f"history {mutation}")
                elif opening == "editor":
                    assert command("preview editor")
                else:
                    send("open history")
                with held_history(history_path, contents):
                    start = monotonic()
                    send("status")
                    assert wait("NATIVE_SMOKE status ", timeout=4).endswith(" false")
                    elapsed_ms = (monotonic() - start) * 1000
                    print(f"app status replied in {elapsed_ms:.1f} ms while the history read was held")
                    if opening == "history":
                        assert command("history open")
                        assert not command("history ready")
                    if mutation:
                        if preview_delete:
                            assert command("preview ready")
                            assert command("preview deleting")
                        else:
                            assert command("history ready")
                        captured_files(1)
                if mutation:
                    if preview_delete:
                        captured_files(0)
                        eventually("preview ready", expected=False)
                    else:
                        eventually("history empty")
                    assert not list(captures.glob("*.png"))
                    assert json.loads(history_path.read_text()) == []
                    print(f"{mutation} removed the capture and its entry after release")
                    if preview_delete:
                        capture_area()
                        eventually("preview ready")
                        files = captured_files(1)
                        contents = history_path.read_text()
                        history_path.unlink()
                        os.mkfifo(history_path)
                        captures.chmod(0o500)
                        try:
                            send(mutation)
                            with held_history(history_path, contents):
                                assert command("preview deleting")
                            eventually("preview deleting", expected=False)
                            assert command("preview ready")
                            assert captured_files(1) == files
                            assert history_path.read_text() == contents
                        finally:
                            captures.chmod(0o700)
                        assert command(mutation)
                        captured_files(0)
                        eventually("preview ready", expected=False)
                        assert json.loads(history_path.read_text()) == []
                        print("failed deletion retained the preview and source, and retry deleted it")
                elif opening == "editor":
                    eventually("close editor")
                    print("the screenshot editor opened after history was released")
                else:
                    eventually("history ready")
                    print("Recent Captures showed its entries after history was released")
                assert command("quit")
                assert app.wait(timeout=6) == 0
                return

            preview = "video preview ready" if recording else "preview ready"
            os.mkfifo(history_path)
            if recording:
                eventually("start-ready")
                command("start")
                eventually("status")
                command("stop")
            else:
                capture_area()
            with held_history(history_path):
                start = monotonic()
                send(preview)
                assert wait(f"NATIVE_SMOKE {preview} ", timeout=4).endswith(" false")
                elapsed_ms = (monotonic() - start) * 1000
                print(f"preview status replied in {elapsed_ms:.1f} ms while history was held")
                if recording:
                    first, = captures.glob("*.poratake")
                    state = json.loads((first / "state.json").read_text())
                    assert state["sourceDuration"] == 2.0 and len(state["segments"]) == 1
                else:
                    first, = captured_files(1)
                capture_area()
                second, = captured_files(1 if recording else 2) - {first}

            deadline = monotonic() + 10
            while True:
                try:
                    history = json.loads(history_path.read_text())
                except json.JSONDecodeError:
                    history = []
                if len(history) >= 2:
                    break
                if monotonic() >= deadline:
                    raise RuntimeError("both captures must be persisted in history")
                sleep(0.01)
            saved_order = [Path(item["originalPath"]) for item in history]
            assert saved_order == [second, first], saved_order
            eventually(preview)
            print("both captures reached history in newest-first order; preview became ready")
            assert command("quit")
            assert app.wait(timeout=6) == 0
        finally:
            if app.poll() is None:
                app.terminate()
                app.wait(timeout=6)


if __name__ == "__main__":
    if sys.argv[1:] == ["--daemon"]:
        fixture_daemon()
    elif sys.argv[1:] in (
        [],
        ["--recording"],
        ["--editor-open"],
        ["--history-open"],
        ["--history-delete"],
        ["--history-clear"],
        ["--preview-delete"],
    ):
        root = Path(__file__).resolve().parents[2]
        verify(
            root / "src/main/target/debug/poratake-gpui",
            recording=sys.argv[1:] == ["--recording"],
            mutation=next(
                (mode for mode in ("delete", "clear") if sys.argv[1:] == [f"--history-{mode}"]),
                "preview delete" if sys.argv[1:] == ["--preview-delete"] else None,
            ),
            opening=next(
                (mode for mode in ("editor", "history") if sys.argv[1:] == [f"--{mode}-open"]),
                None,
            ),
        )
    else:
        raise SystemExit("Usage: python3 tests/native/macos-capture-history-probe.py [--recording | --editor-open | --history-open | --history-delete | --history-clear | --preview-delete]")
