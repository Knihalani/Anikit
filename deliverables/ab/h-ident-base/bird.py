import math
# Frontal kestrel silhouette, hovering. Coordinates centred on (0,0); returns SVG path d.
def P(x,y): return f"{x:.1f} {y:.1f}"
def wing(theta, lag, side):
    # theta: arm angle (deg, + up); lag: extra hand angle
    s = side
    sx, sy = 16*s, -22
    a1 = math.radians(theta); a2 = math.radians(theta + lag)
    L1, L2 = 120, 175
    wx, wy = sx + s*L1*math.cos(a1), sy - L1*math.sin(a1)          # wrist
    tx, ty = wx + s*L2*math.cos(a2), wy - L2*math.sin(a2)          # tip
    # perpendicular (towards trailing edge = "down" relative to wing)
    def perp(a): return (s*math.sin(a), math.cos(a))
    p1, p2 = perp(a1), perp(a2)
    # leading edge: shoulder -> wrist (slight arch) -> tip
    le1 = (sx + s*0.5*L1*math.cos(a1) - p1[0]*10, sy - 0.5*L1*math.sin(a1) - p1[1]*10)
    le2 = (wx + s*0.55*L2*math.cos(a2) - p2[0]*4, wy - 0.55*L2*math.sin(a2) - p2[1]*4)
    # trailing edge: tip -> primaries -> secondaries -> body
    te_hand = (wx + s*0.45*L2*math.cos(a2) + p2[0]*44, wy - 0.45*L2*math.sin(a2) + p2[1]*44)
    te_arm  = (sx + s*0.55*L1*math.cos(a1) + p1[0]*74, sy - 0.55*L1*math.sin(a1) + p1[1]*74)
    root = (12*s, 42)
    pts = [(sx,sy), le1, (wx-p1[0]*6,wy-p1[1]*6), le2, (tx,ty), te_hand, (wx+p1[0]*52,wy+p1[1]*52), te_arm, root]
    if s < 0: pts = pts[::-1]     # keep every subpath wound the same way (nonzero fill, no seams)
    d = f"M{P(*pts[0])}"
    for i in range(1, 9, 2): d += f" Q{P(*pts[i])} {P(*pts[i+1])}"
    return d + " Z"
HOLE = "M-43.5 167 Q0 184 43.5 167 L42 160 Q0 176 -42 160 Z"
def body():
    d  = "M0 -86 C 17 -86 24 -74 22 -58 C 31 -40 30 8 15 46 "   # head, chest
    d += "L 50 186 Q 0 214 -50 186 L -15 46 C -30 8 -31 -40 -22 -58 C -24 -74 -17 -86 0 -86 Z"
    return d
def bird(theta, lag):
    return "M-340 -280 L-340 -280 Z M340 280 L340 280 Z " + body() + " " + HOLE + " " + wing(theta, lag, 1) + " " + wing(theta, lag, -1)
def phases(n=8):
    out=[]
    for k in range(n):
        ph = 2*math.pi*k/n
        th = 12 + 40*math.cos(ph)
        lag = 22*math.sin(ph) - 8   # hand trails on the stroke
        out.append(bird(th, lag))
    return out
GLIDE = bird(14, -6)   # the held pose = the logo mark

PAD = "M-340 -280 L-340 -280 Z M340 280 L340 280 Z "
# the kestrel's dark subterminal tail band, drawn in the background colour
BAND = PAD + "M-41 160 L41 160 L44 172 L-44 172 Z"
