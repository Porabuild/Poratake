import argparse
import json
import math
import struct
import subprocess
from pathlib import Path


def png_dimensions(path):
    with path.open('rb') as image:
        header = image.read(24)
    if header[:8] != b'\x89PNG\r\n\x1a\n' or header[12:16] != b'IHDR':
        raise ValueError(f'{path} is not a PNG')
    return struct.unpack('>II', header[16:24])


def ffmpeg(ffmpeg_path, arguments, data=None):
    return subprocess.run(
        [ffmpeg_path, '-v', 'error', *arguments],
        input=data,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    ).stdout


def decode_png(ffmpeg_path, path):
    return ffmpeg(
        ffmpeg_path,
        [
            '-i', str(path), '-frames:v', '1', '-f', 'rawvideo',
            '-pix_fmt', 'rgba', 'pipe:1',
        ],
    )


def compare(first, second, mask):
    total = 0
    count = 0
    maximum = 0
    for index in mask:
        for channel in range(3):
            difference = abs(first[index + channel] - second[index + channel])
            total += difference
            maximum = max(maximum, difference)
            count += 1
    if count == 0:
        raise ValueError('comparison region is empty')
    return {
        'meanRgbError': total / count,
        'maxChannelError': maximum,
    }


def region_comparison(preview, decoded, converted, width, height, region):
    x, y, region_width, region_height = region
    if not all(math.isfinite(value) for value in region):
        raise ValueError('region must be finite')
    def f32(value):
        return struct.unpack('f', struct.pack('f', value))[0]

    left = max(0, min(width, math.floor(f32(f32(x) * f32(width)))))
    top = max(0, min(height, math.floor(f32(f32(y) * f32(height)))))
    right = max(0, min(
        width, math.ceil(f32(f32(f32(x) + f32(region_width)) * f32(width)))
    ))
    bottom = max(0, min(
        height, math.ceil(f32(f32(f32(y) + f32(region_height)) * f32(height)))
    ))
    if right <= left or bottom <= top:
        raise ValueError('region must intersect the image')
    def offsets():
        return (
            (row * width + column) * 4
            for row in range(top, bottom)
            for column in range(left, right)
        )

    large = 0
    close_to_420 = 0
    large_420 = 0
    for index in offsets():
        for channel in range(3):
            offset = index + channel
            if abs(preview[offset] - decoded[offset]) <= 32:
                continue
            large += 1
            close_to_420 += abs(decoded[offset] - converted[offset]) <= 8
            large_420 += abs(preview[offset] - converted[offset]) > 16
    return {
        'bounds': [left, top, right, bottom],
        'pixels': (right - left) * (bottom - top),
        'previewToDecoded': compare(preview, decoded, offsets()),
        'previewTo420RoundTrip': compare(preview, converted, offsets()),
        'roundTripToDecoded': compare(converted, decoded, offsets()),
        'channelsOver32': large,
        'channelsOver32With420DifferenceOver16': large_420,
        'channelsOver32Within8Of420': close_to_420,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('preview', type=Path)
    parser.add_argument('decoded', type=Path)
    parser.add_argument('--ffmpeg', default='ffmpeg')
    parser.add_argument('--region-json', type=Path)
    args = parser.parse_args()

    width, height = png_dimensions(args.preview)
    if png_dimensions(args.decoded) != (width, height):
        raise ValueError('preview and decoded frame dimensions differ')
    preview = decode_png(args.ffmpeg, args.preview)
    decoded = decode_png(args.ffmpeg, args.decoded)
    if len(preview) != width * height * 4 or len(decoded) != len(preview):
        raise ValueError('unexpected decoded frame size')
    if any(preview[index] != 255 for index in range(3, len(preview), 4)):
        raise ValueError('the preview must be opaque')
    converted = ffmpeg(
        args.ffmpeg,
        [
            '-f', 'rawvideo', '-pixel_format', 'rgba',
            '-video_size', f'{width}x{height}', '-i', 'pipe:0',
            '-frames:v', '1', '-vf', 'format=yuv420p,format=rgba',
            '-f', 'rawvideo', '-pix_fmt', 'rgba', 'pipe:1',
        ],
        preview,
    )
    if len(converted) != len(preview):
        raise ValueError('unexpected converted frame size')
    mask = [
        index
        for index in range(0, len(preview), 4)
        if preview[index] > 150
        and preview[index] > preview[index + 1] + 60
        and preview[index] > preview[index + 2] + 60
    ]
    if not mask and not args.region_json:
        raise ValueError('the preview has no red drawing pixels')
    result = {
        'frameSize': [width, height],
        'redPixels': len(mask),
    }
    if mask:
        result.update({
            'previewToDecoded': compare(preview, decoded, mask),
            'previewTo420RoundTrip': compare(preview, converted, mask),
            'roundTripToDecoded': compare(converted, decoded, mask),
        })
    if args.region_json:
        region = json.loads(args.region_json.read_text())
        if not isinstance(region, list) or len(region) != 4:
            raise ValueError('region JSON must be a four-number array')
        result['region'] = region_comparison(preview, decoded, converted, width, height, region)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
