import errno
import json
import os
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
def held_history(path):
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
        replacement.write_text("[]")
        replacement.replace(path)
        os.write(writer, b"[]")
        os.close(writer)


def drain(stream, replies):
    for line in stream:
        replies.put(line.strip())
    replies.put(None)


def verify(executable):
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

        def eventually(action):
            deadline = monotonic() + 10
            while not command(action):
                if monotonic() >= deadline:
                    raise RuntimeError(f"{action} did not become ready")
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
        try:
            wait("NATIVE_SMOKE ready")
            os.mkfifo(history_path)
            capture_area()
            with held_history(history_path):
                start = monotonic()
                send("preview ready")
                assert wait("NATIVE_SMOKE preview ready ", timeout=4).endswith(" false")
                elapsed_ms = (monotonic() - start) * 1000
                print(f"preview status replied in {elapsed_ms:.1f} ms while history was held")
                first, = captured_files(1)
                capture_area()
                second, = captured_files(2) - {first}

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
            eventually("preview ready")
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
    else:
        root = Path(__file__).resolve().parents[2]
        verify(root / "src/main/target/debug/poratake-gpui")
