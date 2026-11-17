"""Race simulator for the "Telemetry" film.

Simulates the final laps of a fictional race (Meridian Formula Series, Kessra
Harbour Circuit) from one speed profile, so every number on screen agrees with
every other: sector times sum to lap times, gaps come from line-crossing times,
positions on the map come from the same clock, and the gauges read the same
speed profile the lap times were integrated from.

    python3 examples/video/telemetry/race.py      # writes race.json next to this file

`spec.py` turns race.json into spec.json.
"""

import json
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))

TRACK_M = 5112.0  # metres per lap
A_LAT = 15.0  # lateral grip, m/s^2
A_TRACTION = 10.0  # peak traction-limited acceleration, m/s^2
LAPS = 58
FIRST_LAP = 44  # history kept for the lap-time chart
SECTORS = (0.0, 0.318, 0.672, 1.0)  # sector boundaries as lap fractions

# Circuit centre line: control points of a closed Catmull-Rom spline (map units, y down).
CONTROL = [
    (330, 600), (560, 606), (790, 602), (930, 586), (985, 540), (975, 470),
    (905, 432), (820, 405), (790, 340), (838, 268), (930, 222), (950, 140),
    (885, 82), (760, 78), (655, 128), (585, 200), (500, 214), (430, 160),
    (330, 96), (215, 92), (130, 150), (118, 245), (190, 300), (262, 352),
    (228, 430), (150, 470), (120, 540), (170, 590),
]
START = (360, 601)  # start/finish line (lap fraction 0)

TEAMS = {
    "VANTOR": "#FF7A1A",
    "HALCYON": "#19D3C5",
    "ARDENT": "#F0323C",
    "NORDLINE": "#3D7BFF",
    "KESTREL": "#F5C518",
    "SOLENNE": "#D45CFF",
}

# code, full name, team, lap-time offset vs the leader (s), gap at the line before lap 54 (s)
DRIVERS = [
    ("MAR", "Luca Marchetti", "HALCYON", 0.00, 0.0),
    ("OKA", "Rafe Okafor", "VANTOR", 0.00, 1.34),
    ("LIN", "Saga Lindqvist", "ARDENT", 0.22, 6.4),
    ("IVE", "Noa Ivers", "NORDLINE", 0.04, 7.02),
    ("NOV", "Daria Novak", "HALCYON", 0.30, 11.2),
    ("FER", "Tiago Ferreira", "KESTREL", 0.34, 14.7),
    ("WEN", "Kai Wen", "ARDENT", 0.37, 18.3),
    ("DUF", "Mael Dufresne", "VANTOR", 0.41, 21.4),
    ("CAS", "Elio Castell", "SOLENNE", 0.43, 25.0),
    ("HOL", "Freya Holm", "NORDLINE", 0.49, 28.6),
]


def catmull(points, per=60):
    n = len(points)
    out = []
    for i in range(n):
        p0, p1, p2, p3 = points[(i - 1) % n], points[i], points[(i + 1) % n], points[(i + 2) % n]
        for k in range(per):
            s = k / per
            s2, s3 = s * s, s * s * s
            out.append(tuple(
                0.5 * (2 * p1[j] + (-p0[j] + p2[j]) * s + (2 * p0[j] - 5 * p1[j] + 4 * p2[j] - p3[j]) * s2
                       + (-p0[j] + 3 * p1[j] - 3 * p2[j] + p3[j]) * s3)
                for j in range(2)))
    return out


def build_track():
    pts = catmull(CONTROL)
    # rotate so index 0 is the start line
    i0 = min(range(len(pts)), key=lambda i: (pts[i][0] - START[0]) ** 2 + (pts[i][1] - START[1]) ** 2)
    pts = pts[i0:] + pts[:i0]
    # resample uniformly by arc length
    seg = [math.dist(pts[i], pts[(i + 1) % len(pts)]) for i in range(len(pts))]
    total = sum(seg)
    n = 2400
    out, acc, j = [], 0.0, 0
    for k in range(n):
        target = total * k / n
        while acc + seg[j] < target:
            acc += seg[j]
            j += 1
        f = (target - acc) / seg[j]
        a, b = pts[j], pts[(j + 1) % len(pts)]
        out.append((a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f))
    return out


def speed_profile(track):
    """Quasi-steady-state lap: corner speed limited by lateral grip, then forward
    (traction) and backward (braking) passes. Returns m/s per sample."""
    n = len(track)
    ds = TRACK_M / n
    heading = [math.atan2(track[(i + 1) % n][1] - track[i][1], track[(i + 1) % n][0] - track[i][0]) for i in range(n)]
    curv = []
    w = 6
    for i in range(n):
        d = heading[(i + w) % n] - heading[(i - w) % n]
        d = (d + math.pi) % (2 * math.pi) - math.pi
        curv.append(abs(d) / (2 * w * ds))
    vmax = 334 / 3.6
    v = [min(vmax, math.sqrt(A_LAT / max(c, 1e-6))) for c in curv]
    for _ in range(2):
        for i in range(1, 2 * n):  # traction-limited acceleration
            a, b = (i - 1) % n, i % n
            acc = A_TRACTION * (1 - (v[a] / vmax) ** 2) + 0.6
            v[b] = min(v[b], math.sqrt(v[a] ** 2 + 2 * acc * ds))
        for i in range(2 * n, 0, -1):  # braking
            a, b = i % n, (i - 1) % n
            v[b] = min(v[b], math.sqrt(v[a] ** 2 + 2 * 46.0 * ds))
    return v


def gear_of(kmh):
    edges = [0, 95, 128, 160, 194, 230, 262, 296]
    return max(g + 1 for g, e in enumerate(edges) if kmh >= e)


def simulate():
    """Run the race. Returns (data, api): data is what race.json holds; api has
    frac_at(code, T), lap_clock(code, lap, f) and the line-crossing times."""
    track = build_track()
    n = len(track)
    v = speed_profile(track)
    ds = TRACK_M / n
    # time to reach each sample on a reference lap
    tref = [0.0]
    for i in range(n):
        tref.append(tref[-1] + ds / ((v[i] + v[(i + 1) % n]) / 2))
    ref_lap = tref[-1]
    sec_ref = [tref[round(SECTORS[k + 1] * n)] - tref[round(SECTORS[k] * n)] for k in range(3)]

    # throttle / brake from the profile's longitudinal acceleration
    throttle, brake = [], []
    for i in range(n):
        dv = (v[(i + 1) % n] - v[i - 1]) / (2 * ds) * v[i]  # m/s^2
        if dv < -4:
            throttle.append(0.0)
            brake.append(min(1.0, -dv / 40))
        elif dv > 1.5 or v[i] > 330 / 3.6:
            throttle.append(1.0)
            brake.append(0.0)
        else:
            throttle.append(0.55 + 0.4 * max(0.0, dv / 1.5))
            brake.append(0.0)

    # ---- race -------------------------------------------------------------------
    # Lap clock: the reference lap's time-to-distance, scaled per driver and lap,
    # plus local events (a lock-up, a tow) that add or remove time smoothly
    # around a lap fraction. Sector and lap times are read off the same clock.
    def ref_time(f):
        x = f * n
        i = min(n - 1, int(x))
        return tref[i] + (tref[i + 1] - tref[i]) * (x - i)

    t1 = 0.212  # Turn 1 apex (lap fraction)
    events = {  # (code, lap): [(centre frac, width, seconds)]
        ("MAR", 57): [(t1 - 0.016, 0.014, 0.55)],          # locks up into Turn 1
        ("OKA", 57): [(0.11, 0.08, -0.10), (0.5, 0.2, -0.20), (0.85, 0.1, -0.14)],  # tow, then the fastest lap
        ("LIN", 55): [(0.567, 0.012, 0.42)],               # runs wide at Turn 7
        ("MAR", 58): [(0.45, 0.1, 0.30)],
        ("OKA", 58): [(0.9, 0.08, 0.55)],                  # lifts to the flag
    }
    pace = {}
    for code, _, _, off, _ in DRIVERS:
        for lap in range(FIRST_LAP, LAPS + 1):
            k = lap - FIRST_LAP
            p = off + 0.052 * k - 0.058 * k + 0.09 * math.sin(lap * 1.7 + ord(code[0]) * 0.7 + ord(code[1]))
            if code == "OKA":  # fresh softs after a late stop
                p -= 0.43 - 0.004 * k
            if code == "IVE":
                p -= 0.20
            pace[code, lap] = p

    def smooth(x):
        x = min(1.0, max(0.0, x))
        return x * x * (3 - 2 * x)

    def lap_clock(code, lap, f):
        t = ref_time(f) * (1 + pace[code, lap] / ref_lap)
        for c, w, dt in events.get((code, lap), []):
            t += dt * smooth((f - (c - w / 2)) / w)
        return t

    sectors = {c: {lap: [round(lap_clock(c, lap, SECTORS[k + 1]) - lap_clock(c, lap, SECTORS[k]), 4) for k in range(3)]
                   for lap in range(FIRST_LAP, LAPS + 1)} for c, *_ in DRIVERS}
    lap_len = {(c, lap): lap_clock(c, lap, 1.0) for c, *_ in DRIVERS for lap in range(FIRST_LAP, LAPS + 1)}

    # gaps are set at the line before lap 54; earlier laps are back-computed
    want54 = {c: g for c, _, _, _, g in DRIVERS}
    starts = {}
    for code, *_ in DRIVERS:
        t54 = want54[code]
        st = {54: t54}
        for lap in range(53, FIRST_LAP - 1, -1):
            st[lap] = st[lap + 1] - lap_len[code, lap]
        for lap in range(54, LAPS + 1):
            st[lap + 1] = st[lap] + lap_len[code, lap]
        starts[code] = st

    def frac_at(code, T):
        """Laps completed since the line before FIRST_LAP (float) at race time T."""
        st = starts[code]
        if T <= st[FIRST_LAP]:
            return 0.0
        for lap in range(FIRST_LAP, LAPS + 1):
            if T < st[lap + 1]:
                break
        else:
            return float(LAPS + 1 - FIRST_LAP)
        u = T - st[lap]
        lo, hi = 0.0, 1.0
        for _ in range(40):
            mid = (lo + hi) / 2
            if lap_clock(code, lap, mid) < u:
                lo = mid
            else:
                hi = mid
        return lap - FIRST_LAP + lo

    def when(code, laps_done):
        """Race time at which the car has covered `laps_done` laps (as frac_at counts them)."""
        lo, hi = starts[code][FIRST_LAP], starts[code][LAPS + 1]
        for _ in range(50):
            mid = (lo + hi) / 2
            if frac_at(code, mid) < laps_done:
                lo = mid
            else:
                hi = mid
        return hi

    def order_at(T):
        return sorted((c for c, *_ in DRIVERS), key=lambda c: -frac_at(c, T))

    def crossing(a, b, t0, t1):
        """Race time at which car a's distance passes car b's (None if it doesn't)."""
        d = lambda T: frac_at(a, T) - frac_at(b, T)
        lo, hi = t0, t1
        if d(lo) * d(hi) > 0:
            return None
        for _ in range(60):
            mid = (lo + hi) / 2
            if d(lo) * d(mid) <= 0:
                hi = mid
            else:
                lo = mid
        return lo

    oka_pass = crossing("OKA", "MAR", starts["MAR"][57], starts["MAR"][58])
    ive_pass = crossing("IVE", "LIN", starts["LIN"][55], starts["LIN"][56])

    laptimes = {c: {lap: round(lap_len[c, lap], 3) for lap in range(FIRST_LAP, LAPS + 1)} for c in sectors}
    fastest = min(((laptimes[c][l], c, l) for c in laptimes for l in laptimes[c]))

    # classification at the flag
    finish = sorted(((starts[c][LAPS + 1], c) for c in starts))
    win_t = finish[0][0]

    out = {
        "series": "Meridian Formula Series",
        "event": "Kessra Grand Prix",
        "circuit": "Kessra Harbour Circuit",
        "track_m": TRACK_M,
        "laps": LAPS,
        "sectors": SECTORS,
        "teams": TEAMS,
        "drivers": [{"code": c, "name": nm, "team": tm} for c, nm, tm, _, _ in DRIVERS],
        "track": [[round(x, 2), round(y, 2)] for x, y in track],
        "speed_kmh": [round(x * 3.6, 1) for x in v],
        "throttle": [round(x, 3) for x in throttle],
        "brake": [round(x, 3) for x in brake],
        "gear": [gear_of(x * 3.6) for x in v],
        "tref": [round(x, 4) for x in tref],
        "ref_lap": ref_lap,
        "sector_times": {c: {str(l): [round(x, 3) for x in s] for l, s in sectors[c].items()} for c in sectors},
        "lap_times": {c: {str(l): t for l, t in laptimes[c].items()} for c in laptimes},
        "lap_starts": {c: {str(l): round(t, 4) for l, t in starts[c].items()} for c in starts},
        "fastest_lap": {"time": fastest[0], "code": fastest[1], "lap": fastest[2]},
        "overtakes": {
            "OKA_MAR": {"t": oka_pass, "lap": FIRST_LAP + frac_at("OKA", oka_pass)},
            "IVE_LIN": {"t": ive_pass, "lap": FIRST_LAP + frac_at("IVE", ive_pass) if ive_pass else None},
        },
        "classification": [{"code": c, "time": round(t, 3), "gap": round(t - win_t, 3)} for t, c in finish],
    }
    # sanity
    for c in sectors:
        for l in sectors[c]:
            assert abs(sum(sectors[c][l]) - laptimes[c][l]) < 0.003
    api = {"frac_at": frac_at, "lap_clock": lap_clock, "starts": starts, "order_at": order_at,
           "crossing": crossing, "crossing_time": when, "n": n, "first_lap": FIRST_LAP}
    return out, api


def main():
    out, api = simulate()
    with open(os.path.join(HERE, "race.json"), "w") as f:
        json.dump(out, f, separators=(",", ":"))
    lt = out["lap_times"]
    print(f"ref lap {out['ref_lap']:.3f}s  fastest {out['fastest_lap']}")
    print("overtakes", out["overtakes"])
    for c in ("MAR", "OKA", "LIN", "IVE"):
        print(c, [lt[c][str(l)] for l in range(50, 59)])
    print("classification", [(r["code"], r["gap"]) for r in out["classification"]])


if __name__ == "__main__":
    main()
