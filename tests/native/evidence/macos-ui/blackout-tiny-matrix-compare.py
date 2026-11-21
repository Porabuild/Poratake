import json
from pathlib import Path

root = Path(__file__).parent
samples = json.loads((root / 'blackout-coverage-matrix-20261116.json').read_text())['samples']
errors = []
for line in (root / 'blackout-tiny-matrix-20261116.txt').read_text().splitlines():
    offset_x, offset_y, data = line.split(' ', 2)
    red = json.loads(data)
    reference = next(sample for sample in samples if sample['radius'] == 2
                     and sample['offsetX'] == float(offset_x)
                     and sample['offsetY'] == float(offset_y))
    errors.extend(abs(red[row][column] - reference['red'][row][column])
                  for row in range(12) for column in range(12))
print(json.dumps({'samples': len(errors), 'over_two': sum(error > 2 for error in errors),
                  'maximum': max(errors), 'mean': sum(errors) / len(errors)}, indent=2))
