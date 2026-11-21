import json
from pathlib import Path

root = Path(__file__).parent
accelerated = json.loads((root / 'blackout-accelerated-20261116.json').read_text())
software = json.loads((root / 'blackout-software-20261116.json').read_text())

for radius in (0.5, 1, 1.5, 2, 3, 4, 8):
    errors = []
    for first, second in zip(accelerated['samples'], software['samples'], strict=True):
        assert (first['radius'], first['offsetX'], first['offsetY']) == (
            second['radius'], second['offsetX'], second['offsetY']
        )
        if first['radius'] != radius:
            continue
        errors.extend(
            abs(a - b)
            for first_row, second_row in zip(first['red'], second['red'], strict=True)
            for a, b in zip(first_row, second_row, strict=True)
        )
    print(radius, len(errors), sum(error > 2 for error in errors), max(errors))
