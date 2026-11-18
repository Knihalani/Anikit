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

// Shot 4: the hill lifts its head against the moon. The wanderer, tiny, holds up the lantern.

float hillTop(float x){
    float spine = -0.30 + 0.40*exp(-pow((x - 0.04)/0.52, 2.0));
    float head = -0.20 + 0.11*(1.0 - wake)*exp(-pow((x - 0.60)/0.13, 2.0));
    float y = max(spine, head);
    y += 0.03*(1.0 - wake)*smoothstep(0.40, 0.52, x)*smoothstep(0.78, 0.58, x);
    y += 0.010*(fbm1(x*9.0 + 2.0) - 0.5);
    return y;
}

void mainImage(out vec4 o, in vec2 fc){
    vec2 res = iResolution.xy;
    float px = 1.0/(res.y*zoom);
    vec2 sp = (fc - 0.5*res)/res.y/zoom + vec2(0.0, 0.06);
    vec2 bo = boilOff(sp, 0.0012);

    // ---- sky: the moon is just clearing the hill's shoulder
    vec2 moonP = vec2(0.40 - camx*0.08, 0.24);
    vec2 mq = sp - moonP;
    float md = length(mq*vec2(0.8, 1.0));
    vec3 col = mix(hex(9.,10.,32.), hex(30.,36.,92.), smoothstep(0.5, -0.2, sp.y));
    col += hex(96.,108.,210.)*0.85*exp(-md*2.6);
    col += hex(200.,200.,250.)*0.65*exp(-md*7.5);
    col *= 0.93 + 0.14*spray(fc, 0.5, 0.9);
    // painted cloud bars catching the moon on their undersides
    for(int i=0;i<0;i++){
        float fi = float(i);
        float yy = 0.17 + 0.07*fi + 0.01*sin(sp.x*2.0 + fi*1.7);
        float x = sp.x + camx*0.15 + iTime*0.006*(1.0 + fi);
        float lenMask = smoothstep(0.0, 0.25, vn1(x*1.6 + fi*5.0) - 0.35)*smoothstep(0.75, 0.2, abs(sp.x - moonP.x));
        float th = (0.010 + 0.006*vn1(x*5.0 + fi))*lenMask;
        float d = abs(sp.y - yy) - th;
        float under = smoothstep(th, -th, sp.y - yy);
        vec3 cc = mix(hex(40.,46.,110.), hex(170.,172.,236.), exp(-md*2.2)*(0.4 + 0.6*under));
        col = mix(col, cc, smoothstep(px*2.0, -px*2.0, d)*0.9*step(0.0005, th));
    }
    // the moon
    float moon = cov(length(mq) - 0.095, px);
    vec3 moonC = mix(hex(250.,246.,255.), hex(214.,212.,240.), smoothstep(0.02, 0.1, length(mq - vec2(-0.025, 0.03))));
    moonC -= 0.05*smoothstep(0.55, 0.7, fbm(mq*30.0 + 3.0));
    col = mix(col, moonC, moon);
    // stars, only far from the moon
    vec2 sg = floor(sp*80.0);
    float sd = length(fract(sp*80.0) - 0.5 - (h22(sg) - 0.5)*0.7);
    col += step(0.986, h21(sg))*smoothstep(0.1, 0.0, sd)*hex(220.,220.,255.)*smoothstep(0.2, 0.5, md)*(0.4 + 0.6*h21(sg + boilT()));

    // ---- distant range: pale, in the moon's haze
    float x0 = sp.x + camx*0.2 + bo.x*0.3;
    float r0 = -0.12 + 0.05*(fbm1(x0*3.0 + 4.0) - 0.5) + 0.035*abs(sin(x0*5.0 + 1.0))*smoothstep(-0.2, -0.8, x0);
    vec3 far = mix(hex(70.,80.,160.), hex(150.,154.,228.), exp(-md*2.0));
    col = mix(col, far, cov(sp.y - r0, px));
    col = mix(col, hex(140.,146.,222.), 0.35*smoothstep(0.08, 0.0, abs(sp.y + 0.17 + 0.02*fbm(vec2(x0*3.0 + iTime*0.03, 2.0)))));

    // ---- the hill (it is a creature)
    float x1 = sp.x + camx*0.55 + bo.x*0.5;
    float y1 = sp.y + bo.y*0.3;
    float top = hillTop(x1);
    float dh = (y1 - top)*0.93;
    float tD = 1e3;
    float cell = 0.026;
    float ci = floor(x1/cell);
    for(int k=-2;k<=2;k++){
        float cc = ci + float(k);
        float hs = h11(cc*4.7 + 1.3);
        float tx = (cc + 0.5 + 0.6*(h11(cc*1.9) - 0.5))*cell;
        float on = step(0.22, hs)*step(tx, 0.5)*step(-0.62, tx);
        if(on > 0.5){
            float base = hillTop(tx) - 0.012;
            float th = (0.06 + 0.10*h11(cc*8.1))*(0.5 + 0.7*exp(-pow((tx - 0.04)/0.42, 2.0)));
            tD = min(tD, sdSpruce(vec2(x1 - tx, y1 - base), th, cc));
        }
    }
    // the head rises on its neck; spruce 'antlers' crown it
    vec2 hp = vec2(x1, y1);
    vec2 hc = mix(vec2(0.62, -0.21), vec2(0.44, 0.17), wake);
    float ha = mix(0.0, 0.32, wake);
    vec2 hq = hp - hc;
    hq = vec2(cos(ha)*hq.x + sin(ha)*hq.y, -sin(ha)*hq.x + cos(ha)*hq.y);
    float headD = sdEll(hq, vec2(0.12, 0.08));
    headD = smin(headD, sdEll(hq - vec2(0.12, -0.035), vec2(0.085, 0.045)), 0.04);   // snout
    headD = smin(headD, sdEll(hq - vec2(-0.02, -0.06), vec2(0.09, 0.05)), 0.04);    // jaw / beard
    headD += 0.004*(fbm(hq*40.0) - 0.5);
    vec2 neckA = vec2(0.20, 0.0);
    float neck = sdSeg(hp, neckA, hc - vec2(0.03, 0.0), mix(0.09, 0.075, wake));
    float headU = smin(headD, neck, 0.06);
    float ant = min(sdSpruce(hq - vec2(-0.05, 0.05), 0.20, 3.0), sdSpruce(hq - vec2(0.02, 0.06), 0.15, 5.0));
    ant = min(ant, sdSpruce(hq - vec2(-0.10, 0.03), 0.12, 8.0));
    headU = min(headU, mix(1e3, ant, step(0.01, wake)));
    dh = min(dh, headU);
    float shape = min(dh, tD);
    shape = brushEdge(shape, sp, vec2(1.0, 0.0), 0.0022);
    shape += 0.0003*(vn(sp*420.0 + boilT()*3.0) - 0.5) + 0.0012*(vn(sp*90.0) - 0.5);
    float c1 = cov(shape, px);
    // body: near-black, bounce light from the snow rising from the bottom, fur strokes
    vec3 body = mix(hex(6.,7.,22.), hex(26.,30.,76.), smoothstep(-0.20, -0.34, y1)*0.7);
    // fur: short tangential strokes, a little lighter, densest near the lit top
    vec2 fp = vec2(x1, y1)*vec2(60.0, 90.0);
    vec2 fcl = floor(fp);
    vec2 ff = fract(fp) - 0.5 - (h22(fcl) - 0.5)*0.5;
    float ang = 0.35*(h21(fcl) - 0.5) - 0.6*(x1 - 0.04);
    vec2 fr = vec2(cos(ang)*ff.x + sin(ang)*ff.y, -sin(ang)*ff.x + cos(ang)*ff.y);
    float stroke = smoothstep(0.09, 0.02, abs(fr.y + 0.25*fr.x*fr.x))*smoothstep(0.42, 0.2, abs(fr.x))*step(0.45, h21(fcl + 7.0));
    stroke = furTex(vec2(x1, y1), -0.6*(x1 - 0.04));
    float nearTop = smoothstep(-0.12, 0.0, y1 - top);
    body += hex(60.,66.,140.)*stroke*(0.10 + 0.35*nearTop)*step(dh, 0.0);
    col = mix(col, body, c1);
    // rim light, brightest right under the moon
    float rimW = (tD < dh ? 0.0011 : 0.0022 + 0.004*exp(-md*5.0));
    float rim = smoothstep(rimW + px, 0.0, -shape)*step(shape, 0.0)*(tD < dh ? 0.3 : 1.0);
    col += hex(214.,216.,255.)*rim*(0.35 + 1.6*exp(-md*3.2));
    // the eye, small at this distance, amber and lit
    vec2 eq2 = hq - vec2(0.045, 0.012);
    float eyeS = sdEll(eq2, vec2(0.013, max(0.0005, 0.006*smoothstep(0.2, 0.6, wake))));
    col = mix(col, hex(255.,200.,90.), cov(eyeS, px)*step(0.2, wake));
    col += hex(255.,170.,60.)*0.35*exp(-length(eq2)*45.0)*smoothstep(0.2, 0.7, wake);
    // godrays: moonlight cutting between the trees
    vec2 rd = sp - moonP;
    float ray = 0.0;
    float rang = atan(rd.y, rd.x);
    ray = pow(vn1(rang*38.0 + 3.0), 3.0)*smoothstep(0.0, 0.25, md)*exp(-md*2.4)*step(sp.y, moonP.y)*(1.0 - c1);
    col += hex(150.,156.,240.)*ray*0.35;

    // ---- luminous mist lying in front of the hill's foot
    float mistY = -0.30 + 0.012*sin(sp.x*3.0 + iTime*0.2) + 0.02*(fbm(vec2(sp.x*2.5 + camx*0.7 + iTime*0.025, 5.0)) - 0.5);
    float mist = smoothstep(0.09, 0.0, sp.y - mistY)*smoothstep(-0.06, 0.0, sp.y - mistY + 0.04);
    mist *= 0.55 + 0.45*fbm(vec2(sp.x*5.0 + iTime*0.05 + camx*0.7, sp.y*14.0));
    col = mix(col, hex(98.,108.,190.), mist*0.55);

    // ---- snowfield
    float x2 = sp.x + camx;
    float g = -0.33 + 0.016*sin(x2*2.0 + 0.7) + 0.005*(fbm1(x2*12.0) - 0.5) + bo.y*0.3;
    float cg = cov(sp.y - g, px);
    float dep = clamp((g - sp.y)/0.17, 0.0, 1.0);
    vec3 snow = mix(hex(124.,132.,210.), hex(30.,34.,84.), pow(dep, 0.6));
    snow = mix(snow, hex(200.,204.,248.), smoothstep(0.010, 0.0, g - sp.y)*0.85);
    snow *= 0.94 + 0.12*spray(fc, 0.5, 0.8);
    col = mix(col, snow, cg);

    // ---- traveller and the footprint trail behind
    float tx = 0.05 + 0.0*walk;
    float tb = -0.33 + 0.016*sin(tx*2.0 + 0.7) - 0.003;
    float fh = 0.05;
    // footprints: little blue dashes along the crest behind the walker
    float fpx = (x2 - tx)/0.011;
    float fi = floor(fpx);
    float fpy = sp.y - (tb - 0.0035 - 0.0025*mod(fi, 2.0));
    float foot = smoothstep(0.0016, 0.0006, length(vec2((fract(fpx) - 0.5)*0.011*0.5, fpy)))*step(fpx, -0.5)*step(-200.0, fpx);
    col = mix(col, hex(40.,46.,110.), foot*0.8);
    vec2 lanL;
    float fd = sdTraveller((vec2(x2, sp.y) - vec2(tx, tb))/fh, walk, lanL)*fh;
    vec2 lanW = vec2(tx, tb) + lanL*fh;
    float ld = length(vec2(x2, sp.y) - lanW);
    vec3 warm = hex(255.,164.,70.);
    float pool = exp(-pow(length((vec2(x2, sp.y) - lanW - vec2(0.0, -0.035))*vec2(1.0, 2.8))/0.10, 2.0));
    col += warm*0.6*pool*cg*lamp;
    col += warm*0.12*exp(-ld*12.0)*lamp;
    col = mix(col, hex(5.,6.,20.), cov(fd, px));
    col += warm*0.9*cov(abs(fd + 0.0009) - 0.0008, px)*smoothstep(0.03, 0.0, ld)*lamp;
    col = mix(col, hex(255.,242.,205.), cov(ld - 0.0024, px)*lamp);
    col += hex(255.,190.,110.)*lamp*0.9*exp(-ld*200.0);

    // ---- foreground: one big out-of-focus grass clump, bottom right
    float x3 = sp.x + camx*2.4;
    float fg = 1e3;
    for(int k=0;k<7;k++){
        float fk = float(k);
        float gx = 0.52 + 0.06*fk + (h11(fk*3.1) - 0.5)*0.06;
        float len = 0.16 + 0.26*h11(fk*7.3);
        float lean = -0.25 + (h11(fk*5.9) - 0.5)*0.5 + 0.03*sin(iTime*0.9 + fk);
        vec2 a = vec2(gx, -0.56), b = vec2(gx + lean*len, -0.56 + len);
        fg = min(fg, sdSeg(vec2(x3, sp.y), a, b, 0.004 + 0.006*(1.0 - clamp((sp.y + 0.56)/len, 0.0, 1.0))));
    }
    col = mix(col, hex(3.,4.,14.), smoothstep(0.010, -0.010, fg)*0.96);

    // ---- powder snow cascading off the rising neck
    float pour = smoothstep(0.0, 0.25, wake)*smoothstep(1.0, 0.6, wake);
    vec2 pp = vec2(sp.x*140.0, sp.y*22.0 + iTime*9.0);
    float pg = step(0.86, h21(floor(pp)))*smoothstep(0.45, 0.0, abs(fract(pp.x) - 0.5));
    float band = smoothstep(0.16, 0.0, abs(sp.x - 0.33 + 0.25*(sp.y + 0.1)))*step(sp.y, hc.y)*step(-0.32, sp.y);
    col = mix(col, hex(190.,196.,246.), pg*band*pour*0.7);
    col = mix(col, hex(120.,128.,210.), band*pour*0.25*fbm(vec2(sp.x*8.0, sp.y*3.0 + iTime*1.5)));

    // ---- snow: four depths, near flakes smeared and soft, warm near the lantern
    col = snowfall(col, sp, camx, lanW, lamp, hex(214.,218.,250.), hex(255.,214.,150.), 0.6);

    col = finish(col, fc, res);
    o = vec4(col, 1.0);
}
