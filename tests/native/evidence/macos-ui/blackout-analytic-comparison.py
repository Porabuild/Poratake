import json
import math
from pathlib import Path

root = Path(__file__).parent
samples = json.loads((root / 'blackout-rasterizer-electron-20261116.json').read_text())['samples']
results = []
for sample in samples:
    if sample['mode'] != 'svg-image':
        continue
    scale, offset = sample['scale'], sample['offset']
    x = y = (8 + offset) * scale
    candidate = []
    for row in range(4):
        values = []
        for column in range(4):
            px, py = math.floor(x) + column + 0.5, math.floor(y) + row + 0.5
            dx, dy = max(x + 2 - px, 0), max(y + 2 - py, 0)
            if dx and dy:
                coverage = 0.5 + (4 - dx * dx - dy * dy) / (2 * math.hypot(dx, dy))
            elif dx:
                coverage = px - x + 0.5
            elif dy:
                coverage = py - y + 0.5
            else:
                coverage = 1
            values.append(round(255 * (1 - min(1, max(0, coverage)))))
        candidate.append(values)
    errors = [abs(candidate[row][column] - sample['red'][row][column])
              for row in range(4) for column in range(4)]
    results.append({'scale': scale, 'offset': offset, 'candidate': candidate,
                    'maximum_error': max(errors), 'total_error': sum(errors)})
print(json.dumps(results, indent=2))
