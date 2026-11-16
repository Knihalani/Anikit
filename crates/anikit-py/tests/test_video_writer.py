import shutil
import subprocess

import pytest

np = pytest.importorskip("numpy")
from anikit.video import VideoWriter, post  # noqa: E402  (needs numpy)

pytestmark = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="needs ffmpeg")


def test_writes_an_mp4(tmp_path):
    out = tmp_path / "clip.mp4"
    with VideoWriter(str(out), 64, 32, fps=30, preset="ultrafast") as v:
        for i in range(12):
            f = np.zeros((32, 64, 4), np.uint8)
            f[..., 0] = i * 20
            f[:, i * 4 : i * 4 + 8] = 255
            v.write(post(f, vignette=0.3))
    assert v.frames == 12
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-count_frames", "-show_entries", "stream=nb_read_frames,width,height", "-of", "csv=p=0", str(out)],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    assert probe == "64,32,12"


def test_odd_sizes_are_padded_like_the_cli(tmp_path):
    out = tmp_path / "odd.mp4"
    with pytest.warns(UserWarning, match="padded to 64x36"):
        v = VideoWriter(str(out), 63, 35, fps=30, preset="ultrafast")
    with v:
        for k in range(5):
            v.write(np.full((35, 63, 4), 40 * k, np.uint8))
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-count_frames", "-show_entries", "stream=nb_read_frames,width,height", "-of", "csv=p=0", str(out)],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    assert probe == "64,36,5"


def test_rejects_wrong_sizes(tmp_path):
    with pytest.raises(ValueError, match="positive"):
        VideoWriter(str(tmp_path / "a.mp4"), 0, 32)
    with VideoWriter(str(tmp_path / "b.mp4"), 64, 32, preset="ultrafast") as v:
        with pytest.raises(ValueError, match="64x32"):
            v.write(np.zeros((16, 16, 4), np.uint8))
        v.write(np.zeros((32, 64, 4), np.uint8))
