import json
import math
from pathlib import Path

root = Path(__file__).parent
samples = json.loads((root / 'blackout-coverage-matrix-20261116.json').read_text())['samples']
models = {name: {} for name in ('implicit', 'distance', 'hybrid')}
for sample in samples:
    radius = sample['radius']
    x, y = 8 + sample['offsetX'], 8 + sample['offsetY']
    for row in range(12):
        for column in range(12):
            px, py = 7.5 + column, 7.5 + row
            dx, dy = max(x + radius - px, 0), max(y + radius - py, 0)
            distance = math.hypot(dx, dy)
            if dx and dy:
                implicit = 0.5 + (radius * radius - distance * distance) / (2 * distance)
                signed_distance = 0.5 + radius - distance
                values = {
                    'implicit': implicit,
                    'distance': signed_distance,
                    'hybrid': signed_distance if distance > radius else implicit,
                }
            else:
                coverage = px - x + 0.5 if dx else py - y + 0.5 if dy else 1
                values = dict.fromkeys(models, coverage)
            for name, coverage in values.items():
                red = round(255 * (1 - min(1, max(0, coverage))))
                models[name].setdefault(radius, []).append(abs(red - sample['red'][row][column]))

summary = {}
for name, radii in models.items():
    summary[name] = {}
    for radius, errors in radii.items():
        summary[name][radius] = {
            'samples': len(errors),
            'over_two': sum(error > 2 for error in errors),
            'maximum': max(errors),
            'mean': sum(errors) / len(errors),
        }
print(json.dumps(summary, indent=2))
