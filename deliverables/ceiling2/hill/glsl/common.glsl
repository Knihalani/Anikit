// ---- shared painted-world toolkit (prepended to every shot by build.py) ----
uniform float camx;   // = 0.0
uniform float camy;   // = 0.0
uniform float zoom;   // = 1.0
uniform float wake;   // = 0.0   creature awakening 0..1
uniform float eye;    // = 0.0   eye open 0..1
uniform float walk;   // = 0.0   walk cycle phase (cycles)
uniform float lamp;   // = 1.0   lantern intensity
uniform float fade;   // = 1.0

#define PI 3.14159265

float h11(float p){ p = fract(p*0.1031); p *= p + 33.33; p *= p + p; return fract(p); }
float h21(vec2 p){ vec3 p3 = fract(vec3(p.xyx)*0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y)*p3.z); }
vec2 h22(vec2 p){ vec3 p3 = fract(vec3(p.xyx)*vec3(0.1031,0.1030,0.0973)); p3 += dot(p3, p3.yzx+33.33); return fract((p3.xx+p3.yz)*p3.zy); }

float vn(vec2 p){
    vec2 i = floor(p), f = fract(p);
    vec2 u = f*f*(3.0-2.0*f);
    return mix(mix(h21(i), h21(i+vec2(1,0)), u.x), mix(h21(i+vec2(0,1)), h21(i+vec2(1,1)), u.x), u.y);
}
float vn1(float x){ float i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f); return mix(h11(i), h11(i+1.0), f); }
float fbm(vec2 p){ float a = 0.5, s = 0.0; for(int i=0;i<5;i++){ s += a*vn(p); p = p*2.03 + 17.1; a *= 0.5; } return s; }
float fbm1(float x){ float a = 0.5, s = 0.0; for(int i=0;i<5;i++){ s += a*vn1(x); x = x*2.07 + 3.7; a *= 0.5; } return s; }

// hand-drawn "on twos": the drawing is re-made 12 times a second
float boilT(){ return floor(iTime*12.0); }
vec2 boilOff(vec2 p, float amt){
    float b = boilT();
    return amt*(vec2(vn(p*9.0 + b*7.13), vn(p*9.0 + 31.0 + b*5.71)) - 0.5);
}

// soft coverage of a signed distance (px-aware): d in world units, px = world units per pixel
float cov(float d, float px){ return clamp(0.5 - d/px, 0.0, 1.0); }

// airbrushed stipple: value v (0..1) dithered with grainy spray like gouache-on-tooth
float spray(vec2 fc, float v, float amt){
    float b = boilT();
    float n = h21(floor(fc) + b*vec2(13.1, 7.7));
    float n2 = vn(fc*0.35 + b*3.1);
    return clamp(v + amt*((n - 0.5) + 0.6*(n2 - 0.5)), 0.0, 1.0);
}

vec3 hex(float r, float g, float b){ return vec3(r,g,b)/255.0; }

float sdCircle(vec2 p, float r){ return length(p) - r; }
float sdBox(vec2 p, vec2 b){ vec2 d = abs(p)-b; return length(max(d,0.0)) + min(max(d.x,d.y),0.0); }
float sdSeg(vec2 p, vec2 a, vec2 b, float r){ vec2 pa = p-a, ba = b-a; float h = clamp(dot(pa,ba)/dot(ba,ba),0.0,1.0); return length(pa-ba*h) - r; }
float sdEll(vec2 p, vec2 r){ float k0 = length(p/r); float k1 = length(p/(r*r)); return k0*(k0-1.0)/k1; }
float smin(float a, float b, float k){ float h = clamp(0.5+0.5*(b-a)/k,0.0,1.0); return mix(b,a,h) - k*h*(1.0-h); }
float sdTri(vec2 p, vec2 p0, vec2 p1, vec2 p2){
    vec2 e0=p1-p0,e1=p2-p1,e2=p0-p2; vec2 v0=p-p0,v1=p-p1,v2=p-p2;
    vec2 pq0=v0-e0*clamp(dot(v0,e0)/dot(e0,e0),0.0,1.0);
    vec2 pq1=v1-e1*clamp(dot(v1,e1)/dot(e1,e1),0.0,1.0);
    vec2 pq2=v2-e2*clamp(dot(v2,e2)/dot(e2,e2),0.0,1.0);
    float s=sign(e0.x*e2.y-e0.y*e2.x);
    vec2 d=min(min(vec2(dot(pq0,pq0),s*(v0.x*e0.y-v0.y*e0.x)),vec2(dot(pq1,pq1),s*(v1.x*e1.y-v1.y*e1.x))),vec2(dot(pq2,pq2),s*(v2.x*e2.y-v2.y*e2.x)));
    return -sqrt(d.x)*sign(d.y);
}

// a hand-drawn pine: stacked drooping triangles, p in tree units (base at 0, height 1)
float sdPine(vec2 p, float seed){
    float d = sdSeg(p, vec2(0.0,-0.05), vec2(0.0,0.3), 0.025);
    for(int i=0;i<5;i++){
        float fi = float(i);
        float y0 = 0.12 + fi*0.17;
        float w = 0.30*(1.0 - fi*0.17) * (0.85 + 0.3*h11(seed*7.0+fi));
        float hh = 0.30;
        vec2 q = p - vec2(0.0, y0);
        // droop: bend the lower corners down
        float t = sdTri(q, vec2(-w, -0.02 - 0.05*w), vec2(w, -0.02 - 0.06*w), vec2(0.0, hh));
        d = min(d, t);
    }
    return d;
}

float sdTrap(vec2 p, float r1, float r2, float he){
    vec2 k1 = vec2(r2, he); vec2 k2 = vec2(r2 - r1, 2.0*he);
    p.x = abs(p.x);
    vec2 ca = vec2(p.x - min(p.x, (p.y < 0.0) ? r1 : r2), abs(p.y) - he);
    vec2 cb = p - k1 + k2*clamp(dot(k1 - p, k2)/dot(k2, k2), 0.0, 1.0);
    float s = (cb.x < 0.0 && ca.y < 0.0) ? -1.0 : 1.0;
    return s*sqrt(min(dot(ca, ca), dot(cb, cb)));
}

// hooded wanderer facing right: peaked hood, bell cloak, trailing scarf, crooked staff with a lantern.
// p in figure units (feet at y=0, height ~1). Out: lantern centre, eyes sdf.
float sdTraveller(vec2 p, float ph, out vec2 lan){
    float s = sin(ph*2.0*PI), c = cos(ph*2.0*PI);
    float s2 = sin(ph*4.0*PI);
    p.y -= 0.018*abs(s);                       // bob twice per cycle
    // cloak: bell, hem swings back as it walks
    vec2 q = p - vec2(0.0, 0.40);
    q.x += 0.035*(0.0 - q.y)*(0.6 + 0.4*sin(ph*4.0*PI - 0.9));   // hem trails the step
    float cloak = sdTrap(q, 0.25, 0.11, 0.27);
    // wavy hem
    cloak = max(cloak, -(p.y - 0.12 - 0.018*sin(p.x*28.0 + ph*6.28)));
    cloak = smin(cloak, sdEll(p - vec2(0.01, 0.64), vec2(0.15, 0.11)), 0.05);   // shoulders
    // hood with a long point flopping back
    float hood = sdEll(p - vec2(0.03, 0.80), vec2(0.13, 0.135));
    float lag1 = sin(ph*4.0*PI - 1.1), lag2 = sin(ph*4.0*PI - 2.0);
    vec2 tipA = vec2(-0.05, 0.86), tipB = vec2(-0.15, 0.88 + 0.018*lag1), tipC = vec2(-0.21 - 0.01*lag2, 0.82 + 0.03*lag2);
    hood = smin(hood, sdSeg(p, tipA, tipB, 0.05), 0.06);
    hood = smin(hood, sdSeg(p, tipB, tipC, 0.026), 0.04);
    // face opening (a hollow on the right side of the hood)
    float face = sdEll(p - vec2(0.115, 0.78), vec2(0.055, 0.075));
    hood = max(hood, -face + 0.0);
    // boots under the hem
    float b1 = sdEll(p - vec2(-0.03 + 0.09*s, 0.04 + 0.03*max(c, 0.0)), vec2(0.05, 0.03));
    float b2 = sdEll(p - vec2( 0.04 - 0.09*s, 0.04 + 0.03*max(-c, 0.0)), vec2(0.05, 0.03));
    float l1 = sdSeg(p, vec2(-0.02, 0.16), vec2(-0.03 + 0.09*s, 0.06), 0.035);
    float l2 = sdSeg(p, vec2( 0.03, 0.16), vec2( 0.04 - 0.09*s, 0.06), 0.035);
    // pack on the back with a rolled blanket on top
    float pb = 0.014*sin(ph*4.0*PI - 1.4);   // the pack settles a beat after each step
    float sc = sdBox(p - vec2(-0.14, 0.50 + pb), vec2(0.06, 0.10)) - 0.03;
    sc = min(sc, sdEll(p - vec2(-0.13, 0.66 + pb*1.3), vec2(0.10, 0.045)));
    sc = min(sc, sdSeg(p, vec2(-0.20, 0.36 + pb), vec2(-0.22 - 0.02*sin(ph*4.0*PI - 2.2), 0.30 + pb), 0.012));   // a dangling cup swings late
    // arm and staff
    vec2 sh = vec2(0.08, 0.62), hand = vec2(0.24, 0.50 + 0.01*s);
    float arm = sdSeg(p, sh, hand, 0.045);
    vec2 top = hand + vec2(0.13, 0.42), bot = hand + vec2(-0.06, -0.20);
    float staff = sdSeg(p, bot, top, 0.014);
    vec2 crook = top + vec2(0.07, -0.02);
    staff = min(staff, sdSeg(p, top, crook, 0.012));
    float sw = 0.035*sin(ph*2.0*PI - 1.0);
    lan = crook + vec2(sw, -0.17);
    float cord = sdSeg(p, crook, lan + vec2(0.0, 0.07), 0.004);
    float cage = abs(sdBox(p - lan, vec2(0.036, 0.05))) - 0.007;
    cage = min(cage, sdTri(p - lan, vec2(-0.05, 0.045), vec2(0.05, 0.045), vec2(0.0, 0.085)));
    cage = min(cage, sdBox(p - lan - vec2(0.0, -0.055), vec2(0.045, 0.008)));
    float d = min(min(cloak, hood), min(min(b1, b2), min(l1, l2)));
    d = min(d, min(sc, arm));
    d = min(d, min(staff, min(cord, cage)));
    return d;
}

// a spruce silhouette: q relative to its base, th = height. Saw-tooth tiers droop like snow-laden branches.
float sdSpruce(vec2 q, float th, float seed){
    float hh = clamp((q.y + abs(q.x)*0.55)/th, 0.0, 1.0);
    float tiers = 7.0 + floor(4.0*h11(seed*3.3));
    float saw = 1.0 - fract(hh*tiers + h11(seed)*0.5);
    float side = q.x > 0.0 ? 1.0 : 0.93 + 0.1*h11(seed*5.1);
    float w = th*0.21*pow(1.0 - hh, 0.9)*(0.42 + 0.58*saw)*side;
    float d = max(abs(q.x) - w, max(-q.y, q.y - th));
    d = max(d, abs(q.x) - th*0.24*(1.0 - q.y/th));
    d = min(d, sdSeg(q, vec2(0.0, -0.02*th), vec2(0.0, 0.2*th), th*0.018));
    return d;
}

// dry-brush edge: erode/extend a silhouette's edge with bristle streaks running along `dir` (re-drawn on twos)
float brushEdge(float d, vec2 p, vec2 dir, float w){
    vec2 q = vec2(dot(p, dir), dot(p, vec2(-dir.y, dir.x)));
    float b = boilT();
    float n = vn(vec2(q.x*18.0, q.y*420.0) + b*0.37) * 0.65 + vn(vec2(q.x*60.0, q.y*900.0) + b*1.1) * 0.35;
    return d + w*(n - 0.5)*2.0*smoothstep(w*3.0, 0.0, abs(d));
}

// painterly fur: long warped strands along `ang`, gathered into clumps (not a repeating dash grid)
float furTex(vec2 p, float ang){
    float c = cos(ang), s = sin(ang);
    vec2 q = vec2(c*p.x + s*p.y, -s*p.x + c*p.y);
    q += 0.025*vec2(fbm(p*7.0), fbm(p*7.0 + 9.0));
    float b = boilT();
    float strands = vn(vec2(q.x*16.0, q.y*170.0) + b*0.13)*0.65 + vn(vec2(q.x*45.0, q.y*420.0) + b*0.71)*0.35;
    float clump = smoothstep(0.35, 0.75, fbm(p*5.0 + 3.0));
    return smoothstep(0.52, 0.86, strands)*(0.35 + 0.65*clump);
}

// four depths of snow: far fine and dense, near large, motion-smeared and out of focus.
// Flakes near `lp` catch the warm light. `drift` is camera x travel.
vec3 snowfall(vec3 col, vec2 sp, float drift, vec2 lp, float lamp_, vec3 cold, vec3 warmC, float density){
    float gust = 0.35*sin(iTime*0.7) + 0.15*sin(iTime*1.9 + 1.0);
    for(int l=0;l<4;l++){
        float fl = float(l);
        float z = l == 0 ? 0.3 : (l == 1 ? 0.6 : (l == 2 ? 1.1 : 2.6));
        float sc = 30.0/z;
        float fall = 0.10 + 0.16*z;
        vec2 vel = vec2(0.03 + 0.05*gust, -fall);
        vec2 q = (sp + vec2(drift*z*0.6, 0.0) - vel*iTime)*sc;
        q.x += 0.35*sin(q.y*0.45 + fl*2.3 + iTime*0.5);
        vec2 gi = floor(q);
        vec2 f = fract(q) - 0.5 - (h22(gi + fl*17.0) - 0.5)*0.6;
        float on = step(l == 3 ? 0.93 : (1.0 - density*(l == 0 ? 0.55 : 0.35)), h21(gi*1.3 + fl*5.1));
        // smear along the velocity (motion blur grows with nearness)
        vec2 d = normalize(vel);
        float along = dot(f, d), across = dot(f, vec2(-d.y, d.x));
        float smear = 1.0 + 0.9*z*0.4;
        float r = (0.045 + 0.04*h21(gi + 2.0))*(l == 3 ? 2.2 : 1.0);
        float soft = l == 3 ? 0.9 : (l == 2 ? 0.45 : 0.25);
        float dd = length(vec2(along/smear, across));
        float flake = (1.0 - smoothstep(r*(1.0 - soft), r, dd))*on;
        float near = exp(-length(sp - lp)*7.0)*lamp_;
        vec3 fc = mix(cold, warmC, clamp(near*1.6, 0.0, 1.0));
        float a = l == 0 ? 0.35 : (l == 1 ? 0.55 : (l == 2 ? 0.7 : 0.28));
        col = mix(col, fc*(1.0 + 0.8*near), flake*a);
    }
    return col;
}

// filmic finish: tone, grain, vignette (call last)
vec3 finish(vec3 col, vec2 fc, vec2 res){
    vec2 uv = fc/res;
    float vig = 1.0 - 0.55*pow(length((uv - 0.5)*vec2(1.0, 0.8))*1.25, 2.4);
    col *= clamp(vig, 0.0, 1.0);
    float g = h21(fc + floor(iTime*24.0)*vec2(5.3, 9.1)) - 0.5;
    col += g*0.035;
    col = max(col, 0.0);
    col = col/(1.0 + 0.15*col);
    return col*fade;
}
