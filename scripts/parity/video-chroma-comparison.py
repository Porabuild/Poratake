import argparse
import json
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
    differences = [
        abs(first[index + channel] - second[index + channel])
        for index in mask for channel in range(3)
    ]
    return {
        'meanRgbError': sum(differences) / len(differences),
        'maxChannelError': max(differences),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('preview', type=Path)
    parser.add_argument('decoded', type=Path)
    parser.add_argument('--ffmpeg', default='ffmpeg')
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
    if not mask:
        raise ValueError('the preview has no red drawing pixels')
    print(json.dumps({
        'frameSize': [width, height],
        'redPixels': len(mask),
        'previewToDecoded': compare(preview, decoded, mask),
        'previewTo420RoundTrip': compare(preview, converted, mask),
        'roundTripToDecoded': compare(converted, decoded, mask),
    }, indent=2))


if __name__ == '__main__':
    main()
