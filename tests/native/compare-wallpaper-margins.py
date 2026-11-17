import argparse
import json
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('actual')
parser.add_argument('expected')
parser.add_argument('--orientation', choices=('horizontal', 'vertical'), required=True)
parser.add_argument('--margin', type=int, required=True)
args = parser.parse_args()


def dimensions(path):
    result = subprocess.check_output([
        'ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries',
        'stream=width,height', '-of', 'json', path,
    ])
    stream = json.loads(result)['streams'][0]
    return stream['width'], stream['height']


def pixels(path):
    return subprocess.check_output([
        'ffmpeg', '-v', 'error', '-i', path, '-f', 'rawvideo', '-pix_fmt', 'rgba', '-',
    ])


width, height = dimensions(args.actual)
if dimensions(args.expected) != (width, height):
    raise ValueError('Image sizes differ')
horizontal = args.orientation == 'horizontal'
if not 0 < args.margin <= (height if horizontal else width) // 2:
    raise ValueError('Invalid margin')
actual, expected = pixels(args.actual), pixels(args.expected)
for far_edge in (False, True):
    count = total = over_two = worst = 0
    for long in range(width if horizontal else height):
        for margin in range(args.margin):
            if horizontal:
                x, y = long, height - 1 - margin if far_edge else margin
            else:
                x, y = width - 1 - margin if far_edge else margin, long
            offset = (y * width + x) * 4
            delta = max(abs(actual[offset + i] - expected[offset + i]) for i in range(4))
            count += 1
            total += delta
            over_two += delta > 2
            worst = max(worst, delta)
    print(json.dumps({
        'far_edge': far_edge,
        'pixels': count,
        'over_two': over_two,
        'mean_max_channel_delta': round(total / count, 3),
        'max_channel_delta': worst,
    }))
