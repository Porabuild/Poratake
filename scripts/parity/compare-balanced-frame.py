import argparse
import base64
import json
import subprocess
from html.parser import HTMLParser
from pathlib import Path


class Result(HTMLParser):
    def __init__(self):
        super().__init__()
        self.active = False
        self.value = ''

    def handle_starttag(self, tag, attrs):
        self.active = tag == 'pre' and dict(attrs).get('id') == 'result'

    def handle_endtag(self, tag):
        if tag == 'pre':
            self.active = False

    def handle_data(self, data):
        if self.active:
            self.value += data


def decode(path):
    probe = json.loads(subprocess.check_output([
        'ffprobe', '-v', 'error', '-select_streams', 'v:0',
        '-show_entries', 'stream=width,height', '-of', 'json', str(path),
    ]))['streams'][0]
    pixels = subprocess.check_output([
        'ffmpeg', '-v', 'error', '-i', str(path), '-frames:v', '1',
        '-f', 'rawvideo', '-pix_fmt', 'rgba', '-',
    ])
    size = (probe['width'], probe['height'])
    if len(pixels) != size[0] * size[1] * 4:
        raise ValueError(f'invalid decoded pixels: {path}')
    return size, pixels


def compare(gpui, reference, scale, single_inset=False, single_alpha=False, single_alpha_inset=False, translucent_wallpaper=False):
    size, actual = decode(gpui)
    expected_size, expected = decode(reference)
    if size != expected_size:
        raise ValueError(f'dimensions differ: {size} versus {expected_size}')
    title = round(28 * scale)
    top_height = int((24 + title) * 140 / 80 + 0.5)
    if translucent_wallpaper:
        probes = [(40, 100, (64, 112, 192, 255))]
    elif single_alpha_inset:
        probes = [
            (4, 70, (230, 230, 230, 255)),
            (73, 91, (147, 171, 211, 255)),
            (93, 91, (230, 230, 230, 255)),
        ]
    elif single_alpha:
        probes = [(20, 80, (64, 112, 192, 255))]
    elif single_inset:
        probes = [
            (4, 70, (230, 230, 230, 255)),
            (20, 80, (64, 112, 192, 255)),
            (80, 80, (64, 112, 192, 255)),
        ]
    else:
        probes = [
            (70, title + 10, (240, 80, 64, 255)),
            (70, top_height + title + 20, (64, 112, 192, 255)),
            (150, top_height + title + 20, (72, 192, 112, 255)),
        ]
    for pixels in (actual, expected):
        for x, y, color in probes:
            offset = (y * size[0] + x) * 4
            if tuple(pixels[offset:offset + 4]) != color:
                raise ValueError(f'missing expected image content at {x},{y}')
        if single_alpha:
            for x, y, alpha in [(65, 83, 128), (85, 83, 0)]:
                if pixels[(y * size[0] + x) * 4 + 3] != alpha:
                    raise ValueError(f'missing expected image alpha at {x},{y}')
        if translucent_wallpaper:
            for x, y, alpha in [(0, 0, 128), (85, 103, 192), (105, 103, 128)]:
                if pixels[(y * size[0] + x) * 4 + 3] != alpha:
                    raise ValueError(f'missing expected wallpaper alpha at {x},{y}')
    differences = []
    for i in range(0, len(actual), 4):
        actual_alpha = actual[i + 3]
        expected_alpha = expected[i + 3]
        differences.append(max(
            abs(
                round((actual[i + j] * actual_alpha + 255 * (255 - actual_alpha)) / 255)
                - round((expected[i + j] * expected_alpha + 255 * (255 - expected_alpha)) / 255)
            )
            for j in range(3)
        ))
    return {
        'size': size,
        'pixels': len(differences),
        'pixelsOverTwo': sum(difference > 2 for difference in differences),
        'maximumDifference': max(differences),
        'meanMaximumChannelDifference': sum(differences) / len(differences),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('profile', type=Path)
    parser.add_argument('browser_results', type=Path, nargs='*')
    parser.add_argument('--references', type=Path, required=True)
    parser.add_argument('--fractional-result', type=Path)
    parser.add_argument('--single-inset-result', type=Path)
    parser.add_argument('--single-alpha-result', type=Path)
    parser.add_argument('--single-alpha-inset-result', type=Path)
    parser.add_argument('--single-translucent-wallpaper-result', type=Path)
    args = parser.parse_args()
    if len(args.browser_results) != 3 and not (
        not args.browser_results and args.single_translucent_wallpaper_result
    ):
        parser.error('provide three layered results or a translucent-wallpaper result alone')
    args.references.mkdir(parents=True, exist_ok=True)
    results = {}
    cases = list(zip(('1', '1.25', '2'), args.browser_results))
    if args.fractional_result:
        cases.append(('1.3333333', args.fractional_result))
    for scale, html in cases:
        document = Result()
        document.feed(html.read_text())
        images = json.loads(document.value)
        for style in ('light', 'dark'):
            reference = args.references / f'electron-{scale}-{style}.png'
            reference.write_bytes(base64.b64decode(images[f'{style}Png'], validate=True))
            gpui = args.profile / f'gpui-framed-attachments-{scale}-true-macos-{style}-false-unannotated-export.png'
            results[f'{scale}-{style}'] = compare(gpui, reference, float(scale))
    if args.single_inset_result:
        document = Result()
        document.feed(args.single_inset_result.read_text())
        image = json.loads(document.value)['lightPng']
        reference = args.references / 'electron-single-inset.png'
        reference.write_bytes(base64.b64decode(image, validate=True))
        gpui = args.profile / 'balanced-frame-1-macos-light-0-false-8-unannotated.png'
        results['1-light-single-inset'] = compare(gpui, reference, 1., single_inset=True)
    if args.single_alpha_result:
        document = Result()
        document.feed(args.single_alpha_result.read_text())
        image = json.loads(document.value)['lightPng']
        reference = args.references / 'electron-single-alpha.png'
        reference.write_bytes(base64.b64decode(image, validate=True))
        gpui = args.profile / 'balanced-frame-1-macos-light-0-false-0-alpha-unannotated.png'
        results['1-light-single-alpha'] = compare(gpui, reference, 1., single_alpha=True)
    if args.single_alpha_inset_result:
        document = Result()
        document.feed(args.single_alpha_inset_result.read_text())
        image = json.loads(document.value)['lightPng']
        reference = args.references / 'electron-single-alpha-inset.png'
        reference.write_bytes(base64.b64decode(image, validate=True))
        gpui = args.profile / 'balanced-frame-1-macos-light-0-false-8-alpha-unannotated.png'
        results['1-light-single-alpha-inset'] = compare(gpui, reference, 1., single_alpha_inset=True)
    if args.single_translucent_wallpaper_result:
        document = Result()
        document.feed(args.single_translucent_wallpaper_result.read_text())
        image = json.loads(document.value)['lightPng']
        reference = args.references / 'electron-single-translucent-wallpaper.png'
        reference.write_bytes(base64.b64decode(image, validate=True))
        gpui = args.profile / 'balanced-frame-1-macos-light-0-true-0-alpha-translucent-bg-unannotated.png'
        results['1-light-single-translucent-wallpaper'] = compare(gpui, reference, 1., translucent_wallpaper=True)
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
