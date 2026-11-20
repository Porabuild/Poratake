import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time


root = Path(__file__).resolve().parents[2]
subprocess.run(
    [
        "cargo", "build", "--offline", "--locked", "--manifest-path", "src/main/Cargo.toml",
        "-p", "poratake-gpui", "--features", "parity",
    ],
    cwd=root,
    check=True,
)
executable = root / "src/main/target/debug/poratake-gpui"
env = os.environ.copy()
theme = env.setdefault("PORATAKE_NATIVE_UI_THEME", "dark")
scratch = tempfile.TemporaryDirectory(prefix="poratake-save-panel-")
confirm_save = env.get("PORATAKE_NATIVE_SAVE_CONFIRM") == "1"
save_helper = Path(scratch.name) / "save-panel-ax"
if confirm_save:
    subprocess.run(
        [
            "xcrun", "swiftc", "-o", str(save_helper),
            str(Path(__file__).with_name("macos-save-panel-ax.swift")),
        ],
        cwd=root,
        check=True,
    )
env["TMPDIR"] = scratch.name
env["PORATAKE_NATIVE_UI_NORMAL_LEVEL"] = "1"
env.pop("PORATAKE_NATIVE_UI_AUTO_DISMISS", None)
env.pop("PORATAKE_PARITY_DAEMON", None)
process = subprocess.Popen(
    [str(executable), "--native-ui-probe"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
    text=True,
    bufsize=1,
    env=env,
)
lines = queue.Queue()
history = []


def read_output():
    for line in process.stdout:
        line = line.rstrip()
        history.append(line)
        lines.put(line)
    lines.put(None)


threading.Thread(target=read_output, daemon=True).start()


def command(value):
    process.stdin.write(value + "\n")
    process.stdin.flush()


def wait_for(predicate, timeout=30):
    deadline = time.monotonic() + timeout
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("timed out waiting for native save panel")
        line = lines.get(timeout=remaining)
        if line is None:
            raise RuntimeError(f"native probe exited with {process.poll()}:\n" + "\n".join(history[-100:]))
        if predicate(line):
            return line


def wait_for_panel_state(query, matches, complete):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        command(query)
        line = wait_for(matches, timeout=deadline - time.monotonic())
        if complete(line):
            return line
        time.sleep(0.01)
    raise TimeoutError(f"timed out waiting for {query}")


def open_save_panel(expected_directory):
    command("save-open")
    wait_for(lambda line: line == "isolated editor requested save dialog")
    line = wait_for_panel_state(
        "save-panel-status",
        lambda value: value.startswith("save panel directory=")
        or value == "isolated save panel is not visible",
        lambda value: value.startswith("save panel directory="),
    )
    directory, filename = line.removeprefix("save panel directory=").split(", name=", 1)
    assert Path(directory).resolve() == expected_directory.resolve(), (directory, expected_directory)
    assert (
        Path(filename).name == filename
        and filename.startswith("Screenshot ")
        and filename.endswith(".png")
    ), filename
    return expected_directory / filename


def wait_for_save_completion():
    wait_for_panel_state(
        "save-status",
        lambda value: value.startswith("isolated save pending="),
        lambda value: value == "isolated save pending=false",
    )


try:
    wait_for(lambda line: line.startswith("capture preview: visible"))
    command("edit")
    wait_for(lambda line: line == "synthetic preview double-click dispatched")
    command("rectangle")
    wait_for(lambda line: line == "isolated editor rectangle tool selected")
    command("draw")
    wait_for(lambda line: line == "synthetic editor drag dispatched")
    command("annotation-status")
    annotation = wait_for(lambda line: line.startswith("isolated annotation changed pixels="))
    assert int(annotation.rsplit("=", 1)[1]) > 0, annotation
    expected = Path(scratch.name) / f"poratake-native-ui-probe-{process.pid}"
    for trial in range(1, 4):
        started = time.monotonic()
        path = open_save_panel(expected)
        shown = time.monotonic()
        cancel_started = time.monotonic()
        command("save-cancel")
        wait_for(lambda value: value == "isolated save panel canceled")
        cancel_returned = time.monotonic()
        wait_for_save_completion()
        canceled = time.monotonic()
        assert not path.exists()
        print(
            f"{theme} trial {trial}: save panel visible in {(shown - started) * 1000:.1f} ms, "
            f"cancel returned in {(cancel_returned - cancel_started) * 1000:.1f} ms, "
            f"save state cleared in {(canceled - cancel_started) * 1000:.1f} ms"
        )
    if confirm_save:
        path = open_save_panel(expected)
        assert not path.exists()
        subprocess.run([str(save_helper), str(process.pid)], check=True, timeout=15)
        wait_for_save_completion()
        assert path.is_file(), path
        print(f"{theme}: native Save confirmed and wrote the proposed PNG")
    else:
        path = expected / "fixture-saved.png"
        command("save-to-fixture")
        wait_for(lambda line: line == "isolated fixture PNG saved")
    command("reopen-saved")
    saved_status = wait_for(lambda line: line.startswith("isolated saved PNG: "))
    wait_for(lambda line: line == "isolated saved PNG matches the editor output")
    wait_for(lambda line: line == "isolated saved PNG reopened: true")
    wait_for(lambda line: line == "isolated reopened editor matches the saved PNG")
    evidence = os.environ.get("PORATAKE_NATIVE_SAVE_EVIDENCE")
    if evidence:
        evidence_path = Path(evidence)
        assert evidence_path.is_absolute() and evidence_path.is_dir()
        shutil.copy2(expected / "preview.png", evidence_path / "native-save-source.png")
        shutil.copy2(path, evidence_path / "native-save-annotated.png")
    print(annotation)
    print(saved_status)
    print("native save panel uses the isolated directory and a separate PNG filename")
    print("PNG saved by the background writer and reopened editor match the annotated editor pixel for pixel")
finally:
    try:
        if process.poll() is None:
            try:
                command("quit")
                process.wait(timeout=5)
            except BrokenPipeError:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)
    finally:
        scratch.cleanup()
