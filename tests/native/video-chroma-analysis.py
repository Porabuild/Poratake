import argparse
import json
import subprocess
from pathlib import Path


def rgb(*arguments):
    return subprocess.check_output(
        ["ffmpeg", "-hide_banner", "-v", "error", *arguments, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]
    )


def analyze(reference, export):
    original = rgb("-i", str(reference), "-frames:v", "1")
    converted = rgb("-i", str(reference), "-vf", "format=yuv420p,format=rgb24", "-frames:v", "1")
    decoded = rgb("-ss", "1", "-i", str(export), "-frames:v", "1")
    assert len(original) == len(converted) == len(decoded) == 320 * 180 * 3
    conversion = [abs(a - b) for a, b in zip(original, converted)]
    error = [abs(a - b) for a, b in zip(original, decoded)]
    residual = [abs(a - b) for a, b in zip(converted, decoded)]
    return {
        "channels": len(original),
        "yuv_mean": sum(conversion) / len(conversion),
        "yuv_above_32": sum(value > 32 for value in conversion),
        "export_mean": sum(error) / len(error),
        "export_above_32": sum(value > 32 for value in error),
        "shared_above_32": sum(a > 32 and b > 32 for a, b in zip(conversion, error)),
        "decoded_vs_yuv_mean": sum(residual) / len(residual),
    }


parser = argparse.ArgumentParser()
parser.add_argument("references", type=Path)
parser.add_argument("exports", type=Path)
parser.add_argument("--export-pattern", default="recording-{target}-edited.mp4")
args = parser.parse_args()
print(
    json.dumps(
        {
            target: analyze(
                args.references / f"{target}.png",
                args.exports / args.export_pattern.format(target=target),
            )
            for target in ("area", "window", "screen")
        },
        indent=2,
    )
)
