import ast
import json
import re
from pathlib import Path

root = Path(__file__).parent
reference = json.loads((root / "blackout-corner-electron-43.json").read_text())
prior = json.loads((root / "blackout-corner-results.json").read_text())
output = {"electron": reference["electron"], "chrome": reference["chrome"], "comparisons": {}}

for name, values in (
    ("tiny_skia", prior["tiny_skia"]["samples"]),
    ("tiny_skia_4x_supersampled_scratch", prior["tiny_skia_4x_supersampled_scratch"]),
):
    cases = []
    assert len(values) == len(reference["samples"])
    for value, expected in zip(values, reference["samples"], strict=True):
        match = re.fullmatch(r"scale=(\S+) offset=(\S+) red=(\[.*\])", value)
        assert match is not None
        assert float(match.group(1)) == expected["scale"]
        assert float(match.group(2)) == expected["offset"]
        actual = ast.literal_eval(match.group(3))
        errors = [
            abs(pixel - reference_pixel)
            for row, reference_row in zip(actual, expected["red"], strict=True)
            for pixel, reference_pixel in zip(row, reference_row, strict=True)
        ]
        cases.append(
            {
                "scale": expected["scale"],
                "offset": expected["offset"],
                "pixels": len(errors),
                "over_two": sum(error > 2 for error in errors),
                "absolute_error": sum(errors),
                "maximum_error": max(errors),
            }
        )
    output["comparisons"][name] = {
        "pixels": sum(case["pixels"] for case in cases),
        "over_two": sum(case["over_two"] for case in cases),
        "absolute_error": sum(case["absolute_error"] for case in cases),
        "maximum_error": max(case["maximum_error"] for case in cases),
        "cases": cases,
    }

print(json.dumps(output, indent=2))
