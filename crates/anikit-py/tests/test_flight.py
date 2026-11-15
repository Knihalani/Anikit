"""Flight paths: numbers must match the TypeScript implementation.

``data/flight_ref.json`` is produced by ``data/flight_ref.ts`` running the
real ``packages/anikit/src/flight.ts`` under Node.
"""

import json
import math
import pathlib

import pytest

import anikit
from anikit.transform import Camera

REF = json.loads((pathlib.Path(__file__).parent / "data" / "flight_ref.json").read_text())


def close(a, b, tol=1e-9):
    if isinstance(a, (list, tuple)):
        assert len(a) == len(b)
        for x, y in zip(a, b):
            close(x, y, tol)
    else:
        assert a == pytest.approx(b, rel=tol, abs=tol)


def pose(p, ref):
    close(p.eye, ref["eye"])
    close(p.target, ref["target"])
    close(p.roll, ref["roll"])
    close(p.forward, ref["forward"])


@pytest.fixture
def open_path():
    return anikit.flight_path([[0, 0, 2000], [300, 80, 0], [-200, -60, -3000], [0, 0, -6000]], look_ahead=500, bank=1)


def test_open_path_matches_typescript(open_path):
    r = REF["open"]
    L = open_path.length
    close(L, r["length"])
    for s, ref in zip([0, 100, 1234.5, 4000, L, L + 50, -20], r["points"]):
        close(open_path.point(s), ref)
    for s, ref in zip([0, 800, 2500, 5000, L - 100], r["at"]):
        pose(open_path.at(s), ref)
    for s, ref in zip([10, 3000], r["tangent"]):
        close(open_path.tangent(s), ref)


def test_timing_matches_typescript(open_path):
    r = REF["open"]
    tm = open_path.timing([(0, 0), (2, 0.3), (5, 1)])
    close([tm(t) for t in [-1, 0, 0.5, 1, 2, 3.3, 4.9, 5, 6]], r["timing"])
    tm2 = open_path.timing([(0, 0), (1, 0.5), (2, 0.5), (3, 1)])
    close([tm2(t) for t in [0.25, 1.5, 2.5]], r["timing2"])


def test_timing_is_monotone_and_holds(open_path):
    tm = open_path.timing([(3, 1), (0, 0), (1, 0.5), (2, 0.5)])  # unsorted keys are fine
    vals = [tm(i / 100) for i in range(0, 301)]
    assert all(b >= a - 1e-9 for a, b in zip(vals, vals[1:]))
    assert tm(1.5) == pytest.approx(0.5 * open_path.length)
    with pytest.raises(ValueError):
        open_path.timing([])


def test_closed_path_matches_typescript():
    b = anikit.flight_path(
        [(0, 300, 2000), (0, 120, -2000), (180, 60, -8000), (-500, 400, -12000)],
        look_ahead=900,
        bank=1.4,
        max_bank=30,
        closed=True,
    )
    r = REF["closed"]
    close(b.length, r["length"])
    for s, ref in zip([0, 5000, b.length + 1000, -700], r["points"]):
        close(b.point(s), ref, 1e-7)
    for s, ref in zip([0, 3000, 9000, 20000], r["at"]):
        pose(b.at(s), ref)
    pose(b.at(3000, look_ahead=100, bank=3), r["atOpts"])
    assert all(abs(b.at(s).roll) <= 30 for s in range(0, 30000, 997))


def test_point_inputs_and_straight_line():
    c = anikit.FlightPath([{"x": 0, "y": 0, "z": 0}, {"x": 100, "y": 0, "z": 0}])
    r = REF["line"]
    close(c.length, r["length"])
    pose(c.at(50), r["at"])
    assert c.timing([(1, 0.5)])(3) == pytest.approx(r["one"])
    with pytest.raises(ValueError):
        anikit.flight_path([(0, 0, 0)])


def test_apply_moves_camera_and_rolls_view():
    path = anikit.flight_path([(0, 0, 2000), (300, 80, 0), (-200, -60, -3000)], look_ahead=300)
    cam = Camera()
    p = path.apply(cam, 900, extra_roll=10)
    assert cam.position == p.eye and cam.target == p.target
    assert cam.roll == pytest.approx(p.roll + 10)
    # Roll rotates clip x/y about the view axis: the aspect-corrected rows keep
    # a point on the view axis in place and rotate an off-axis one.
    cam2 = Camera((0, 0, 1000), roll=90)
    vp0 = Camera((0, 0, 1000)).view_projection(1.0)
    vp = cam2.view_projection(1.0)
    x0, y0, _ = vp0.transform_point((100, 0, 0))
    x1, y1, _ = vp.transform_point((100, 0, 0))
    assert math.isclose(x1, -y0, abs_tol=1e-9) and math.isclose(y1, x0, abs_tol=1e-9)
    assert "roll=90" in repr(cam2)
