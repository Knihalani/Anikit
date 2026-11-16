import math

from anikit import flight_path


class _Cam:
    position = (0.0, 0.0, 0.0)
    target = (0.0, 0.0, 0.0)
    roll = 0.0


def test_shake_is_deterministic_and_small():
    p = flight_path([(0, 0, 0), (0, 0, -1000), (0, 0, -2000)], look_ahead=200)
    c = _Cam()
    p.apply(c, 500)
    still = c.position
    p.apply(c, 500, shake=20)
    a = c.position
    p.apply(c, 500, shake=20)
    assert c.position == a
    assert 0.5 < math.dist(still, a) < 40
    p.apply(c, 520, shake=20)
    assert math.dist(c.position[:2], a[:2]) < 10
