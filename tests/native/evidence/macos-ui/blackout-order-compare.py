import json
from pathlib import Path

root = Path(__file__).parent
gpui = json.loads((root / "blackout-order-gpui-20261116.json").read_text())
electron = json.loads((root / "blackout-order-electron-43.json").read_text())
assert len(gpui) == len(electron["samples"])
results = {}

for actual, expected in zip(gpui, electron["samples"], strict=True):
    key = (actual["scale"], actual["offset"], actual["order"], actual["x"], actual["y"])
    assert key == tuple(expected[name] for name in ("scale", "offset", "order", "x", "y"))
    errors = [
        abs(channel - reference)
        for row, reference_row in zip(actual["rgba"], expected["rgba"], strict=True)
        for pixel, reference_pixel in zip(row, reference_row, strict=True)
        for channel, reference in zip(pixel, reference_pixel, strict=True)
    ]
    result = results.setdefault(actual["order"], {"cases": [], "channels": 0, "above_two": 0, "absolute_error": 0, "maximum_error": 0})
    result["cases"].append({
        "scale": actual["scale"],
        "offset": actual["offset"],
        "above_two": sum(error > 2 for error in errors),
        "maximum_error": max(errors),
    })
    result["channels"] += len(errors)
    result["above_two"] += sum(error > 2 for error in errors)
    result["absolute_error"] += sum(errors)
    result["maximum_error"] = max(result["maximum_error"], max(errors))

print(json.dumps({"electron": electron["electron"], "chrome": electron["chrome"], "orders": results}, indent=2))
