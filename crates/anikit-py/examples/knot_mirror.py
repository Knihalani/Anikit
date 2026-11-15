"""Render a lit chrome torus knot over a mirror floor, graded like film.

    python examples/knot_mirror.py [out_dir] [frames]

Writes ``knot_000.png`` ... with only numpy and the standard library (a
tiny PNG writer below). The camera flies a short banked path around the
knot, light pulses run along its tube, and the frame gets bloom, anamorphic
streaks, a vignette and grain.
"""

from __future__ import annotations

import math
import pathlib
import struct
import sys
import time
import zlib

import numpy as np

import anikit
from anikit import raster, video
from anikit.transform import Camera


def write_png(path: pathlib.Path, rgba: np.ndarray) -> None:
    """Minimal RGBA PNG (8-bit, no interlace)."""
    h, w = rgba.shape[:2]
    raw = np.zeros((h, 1 + w * 4), np.uint8)  # filter byte 0 per row
    raw[:, 1:] = rgba.reshape(h, w * 4)

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw.tobytes(), 6))
        + chunk(b"IEND", b"")
    )


def main() -> None:
    out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "knot_frames")
    frames = int(sys.argv[2]) if len(sys.argv) > 2 else 6
    out.mkdir(parents=True, exist_ok=True)

    W, H, FPS = 960, 540, 30
    r3 = raster.Renderer3D(W, H)
    knot = raster.torus_knot(p=2, q=3, radius=260, spread=110, tube=52, segments=480, sides=24)
    floor = raster.plane(1)
    FLOOR_Y = -440.0

    # Chrome with glowing lines and pulses running along the tube.
    chrome = raster.Material(
        metal=0.9, iridescence=0.25, specular=1.4, shininess=120,
        lines=1.0, line_color=(0.3, 0.8, 1.6), grid=(0, 6), pulse=2.5, pulse_count=3,
    )  # fmt: skip
    floor_mat = raster.Material(lines=0.6, line_color=(0.5, 0.3, 1.2), grid=(60, 60), line_width=0.02)
    lights = raster.Lighting(
        sky=(0.05, 0.07, 0.16), horizon=(0.9, 0.5, 0.95), ground=(0.01, 0.01, 0.02),
        strips=(2.2, 2.3, 2.6), fog=(1500, 5000),
    )  # fmt: skip

    # A banked camera flight around the knot.
    path = anikit.flight_path(
        [(0, 60, 2300), (1500, 0, 1000), (1500, 200, -1100), (-300, 80, -2100), (-1700, 150, 0)],
        look_ahead=900, bank=1.2,
    )  # fmt: skip
    distance = path.timing([(0, 0), (frames / FPS, 0.35)])
    cam = Camera(fov=45, far=20000)
    grade = video.PostFx(
        bloom=0.8, bloom_threshold=0.55, bloom_radius=30, streaks=0.4, streak_length=200,
        vignette=0.45, grain=3, contrast=1.06, shadows=(0, 0.01, 0.03),
    )  # fmt: skip

    spin = raster.Instances(1)
    for i in range(frames):
        t = i / FPS
        path.apply(cam, distance(t))
        cam.target = (0.0, 0.0, 0.0)  # keep the knot framed while flying
        spin.set(0, rx=20 + 10 * math.sin(t), ry=t * 40)
        chrome.pulse_phase = t * 0.8
        t0 = time.perf_counter()
        frame = r3.render(
            cam,
            [
                raster.Item(knot, spin, chrome),
                raster.Item(
                    floor, raster.instances(1, y=FLOOR_Y, scale=12000, color=(0.04, 0.04, 0.05)), floor_mat, mirror=True
                ),
            ],
            background="#04050a",
            mirror=raster.Mirror(FLOOR_Y, strength=0.55),
            lighting=lights,
        )
        grade(frame, seed=i)  # in place, no copy
        ms = (time.perf_counter() - t0) * 1000
        name = out / f"knot_{i:03d}.png"
        write_png(name, frame)
        print(f"{name}  {r3.triangles} triangles  render {r3.last_ms:.1f} ms  render+grade {ms:.1f} ms")


if __name__ == "__main__":
    main()
