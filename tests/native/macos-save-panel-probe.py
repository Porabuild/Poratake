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
env["PORATAKE_NATIVE_UI_THEME"] = "dark"
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


def wait_for_save_completion():
    deadline = time.monotonic() + 30
    while True:
        command("save-status")
        line = wait_for(
            lambda value: value.startswith("isolated save pending="),
            timeout=max(0.01, deadline - time.monotonic()),
        )
        if line == "isolated save pending=false":
            return


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
    command("save-open")
    wait_for(lambda line: line == "isolated editor requested save dialog")
    deadline = time.monotonic() + 30
    while True:
        command("save-panel-status")
        line = wait_for(
            lambda value: value.startswith("save panel directory=")
            or value == "isolated save panel is not visible",
            timeout=max(0.01, deadline - time.monotonic()),
        )
        if line.startswith("save panel directory="):
            break
    directory, filename = line.removeprefix("save panel directory=").split(", name=", 1)
    expected = Path(tempfile.gettempdir()) / f"poratake-native-ui-probe-{process.pid}"
    assert Path(directory).resolve() == expected.resolve(), (directory, expected)
    assert filename.startswith("Screenshot ") and filename.endswith(".png"), filename
    command("save-cancel")
    wait_for(lambda line: line == "isolated save panel canceled")
    wait_for_save_completion()
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
        shutil.copy2(expected / "fixture-saved.png", evidence_path / "native-save-annotated.png")
    print(annotation)
    print(saved_status)
    print("native save panel uses the isolated directory and a separate PNG filename")
    print("directly saved PNG and reopened editor match the annotated editor pixel for pixel")
finally:
    if process.poll() is None:
        try:
            command("quit")
            process.wait(timeout=5)
        except BrokenPipeError:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)
