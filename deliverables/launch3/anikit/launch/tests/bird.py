import math
def resample(poly, n):
    seg=[math.dist(poly[i],poly[i+1]) for i in range(len(poly)-1)]; L=sum(seg); out=[]
    for k in range(n):
        d=(k+0.5)/n*L; i=0
        while i<len(seg)-1 and d>seg[i]: d-=seg[i]; i+=1
        f=d/seg[i] if seg[i] else 0
        out.append((poly[i][0]+(poly[i+1][0]-poly[i][0])*f, poly[i][1]+(poly[i+1][1]-poly[i][1])*f))
    return out
def bird(phase, cx=0.0, cy=0.0, s=1.0):
    """A gull seen from above, flying toward -y; flap phase 0..1 -> 144 outline points (yards)."""
    up = 0.5 + 0.5*math.cos(phase*2*math.pi)     # 1 wings level (full span), 0 wings raised (short)
    span = 7.0 + 9.5*up                           # half span
    sweep = 1.5 + 3.0*(1-up)                      # tips swept back when raised
    bend = 1.2 + 1.2*up
    # right half outline from beak, around the wing, to tail centre
    def half(sd):
        return [(0,-6.2),(0.7*sd,-4.6),(1.1*sd,-2.2),
                (span*0.45*sd,-2.6-bend*0.4),(span*0.8*sd,-2.0+sweep*0.4),(span*sd,-1.2+sweep),
                (span*0.82*sd,0.2+sweep*0.8),(span*0.5*sd,0.6+sweep*0.3),(1.2*sd,1.0),
                (0.9*sd,3.6),(2.0*sd,6.2),(0,5.5)]
    r = half(1); l = half(-1)
    outline = r + l[::-1][1:]
    pts = resample(outline, 144)
    return [(cx+s*x, cy+s*y) for x,y in pts]
