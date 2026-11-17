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
    q.x += 0.03*(0.0 - q.y)*(0.6 + 0.4*s2);
    float cloak = sdTrap(q, 0.25, 0.11, 0.27);
    // wavy hem
    cloak = max(cloak, -(p.y - 0.12 - 0.018*sin(p.x*28.0 + ph*6.28)));
    cloak = smin(cloak, sdEll(p - vec2(0.01, 0.64), vec2(0.15, 0.11)), 0.05);   // shoulders
    // hood with a long point flopping back
    float hood = sdEll(p - vec2(0.03, 0.80), vec2(0.13, 0.135));
    vec2 tipA = vec2(-0.05, 0.86), tipB = vec2(-0.15, 0.88 + 0.008*s2), tipC = vec2(-0.21, 0.82 + 0.012*s);
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
    float sc = sdBox(p - vec2(-0.14, 0.50), vec2(0.06, 0.10)) - 0.03;
    sc = min(sc, sdEll(p - vec2(-0.13, 0.66), vec2(0.10, 0.045)));
    sc = min(sc, sdSeg(p, vec2(-0.20, 0.36), vec2(-0.22, 0.30 + 0.01*s), 0.012));   // a dangling cup
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

// Shot 2: tracking beside the wanderer through the trunks at the hill's foot. Warm lantern vs cold moon.

float trunks(vec2 p, float cell, float seed, float wmin, float wmax, out float side){
    float ci = floor(p.x/cell);
    float d = 1e3; side = 0.0;
    for(int k=-1;k<=1;k++){
        float cc = ci + float(k);
        if(h11(cc*3.7 + seed) < 0.45) continue;
        float tx = (cc + 0.5 + 0.5*(h11(cc*1.3 + seed) - 0.5))*cell;
        float w = mix(wmin, wmax, h11(cc*7.1 + seed));
        float lean = (h11(cc*2.9 + seed) - 0.5)*0.06;
        float x = p.x - tx - lean*p.y;
        // slight taper and bark wobble
        float ww = w*(1.0 - 0.15*p.y) + 0.0015*(vn(vec2(cc, p.y*40.0)) - 0.5);
        float dd = abs(x) - ww;
        if(dd < d){ d = dd; side = x/ww; }
    }
    return d;
}

void mainImage(out vec4 o, in vec2 fc){
    vec2 res = iResolution.xy;
    float px = 1.0/res.y;
    vec2 sp = (fc - 0.5*res)/res.y;
    vec2 bo = boilOff(sp, 0.0012);

    // figure placement (camera tracks it; the world slides by)
    vec2 feet = vec2(0.16, -0.375);
    float fh = 0.60;
    vec2 lanL;
    vec2 fq = (sp - feet)/fh;
    float fd = sdTraveller(fq + bo*0.6/fh, walk, lanL)*fh;
    vec2 lanW = feet + lanL*fh;
    vec2 lv = sp - lanW;
    float ld = length(lv);
    float flick = 0.92 + 0.08*vn1(iTime*9.0);
    vec3 warm = hex(255.,160.,64.)*lamp*flick;

    // ---- sky glimpsed between trunks: moon upper left
    vec2 moonP = vec2(-0.55, 0.42);
    float md = length(sp - moonP);
    vec3 col = mix(hex(12.,14.,40.), hex(44.,52.,120.), exp(-md*1.6));
    col += hex(170.,176.,240.)*0.35*exp(-md*4.0);
    col *= 0.93 + 0.14*spray(fc, 0.5, 0.9);

    // ---- far trunks (pale in haze), parallax 0.3
    float side;
    vec2 p0 = vec2(sp.x + camx*0.3, sp.y) + bo*0.3;
    float sprD = 1e3;
    float sci = floor(p0.x/0.13);
    for(int k=-2;k<=2;k++){
        float cc = sci + float(k);
        float tx = (cc + 0.5 + 0.4*(h11(cc*2.3) - 0.5))*0.13;
        float th = 0.55 + 0.35*h11(cc*4.1);
        sprD = min(sprD, sdSpruce(p0 - vec2(tx, -0.30), th, cc + 40.0));
    }
    col = mix(col, mix(hex(50.,58.,124.), hex(86.,94.,172.), exp(-md*1.2)), cov(sprD, px));
    float t0 = trunks(p0 + vec2(0.05, 0.0), 0.11, 1.0, 0.005, 0.012, side);
    col = mix(col, mix(hex(40.,46.,104.), hex(70.,78.,150.), exp(-md*1.2)), cov(t0, px)*0.8);
    // mist band
    col = mix(col, hex(80.,88.,165.), 0.6*smoothstep(0.22, -0.05, sp.y + 0.12)*smoothstep(-0.5, -0.25, sp.y));

    // ---- ground: snow bank, crest gently rolling
    float gx = sp.x + camx;
    float g = -0.36 + 0.02*sin(gx*3.0 + 1.0) + 0.006*(fbm1(gx*14.0) - 0.5);
    float cg = cov(sp.y - g, px);

    // ---- mid trunks, lit by the lantern on the side that faces it, parallax 0.65
    vec2 p1 = vec2(sp.x + camx*0.65, sp.y) + bo*0.5;
    float t1 = trunks(p1, 0.24, 7.0, 0.016, 0.042, side);
    float c1 = cov(t1, px)*step(g - 0.04, sp.y);
    vec3 trunkC = hex(14.,16.,44.);
    // lantern light wraps the facing side
    float facing = clamp(side*sign(lanW.x - sp.x)*0.5 + 0.5, 0.0, 1.0);
    float lit = exp(-pow(abs(sp.x - lanW.x)/0.45, 1.5))*exp(-abs(sp.y - lanW.y)*2.2);
    trunkC += warm*0.55*pow(facing, 3.0)*lit;
    // moon rim on the other side
    trunkC += hex(120.,130.,220.)*0.25*smoothstep(0.7, 1.0, -side)*exp(-md*0.8);
    // bark strokes
    trunkC *= 0.85 + 0.3*vn(vec2(p1.x*300.0, p1.y*18.0));
    col = mix(col, trunkC, c1);

    // snow
    float dep = clamp((g - sp.y)/0.16, 0.0, 1.0);
    vec3 snow = mix(hex(118.,126.,204.), hex(36.,40.,96.), pow(dep, 0.7));
    snow = mix(snow, hex(196.,200.,246.), smoothstep(0.012, 0.0, g - sp.y)*0.8);
    snow *= 0.94 + 0.12*spray(fc, 0.5, 0.8);
    // lantern pool on snow (an ellipse stretched along the ground)
    float pool = exp(-pow(length((sp - vec2(lanW.x, g - 0.02))*vec2(0.8, 3.0))/0.30, 2.0));
    snow += warm*0.75*pool;
    // footprints behind (they scroll away with the ground)
    float fpx = (gx - (feet.x + camx))/0.05;
    float fi = floor(fpx);
    float fy = sp.y - (g - 0.016 - 0.008*mod(fi, 2.0));
    float foot = smoothstep(0.005, 0.002, length(vec2((fract(fpx) - 0.5)*0.05*0.35, fy*1.6)))*step(fpx, -0.6);
    snow = mix(snow, snow*0.55, foot);
    col = mix(col, snow, cg);

    // air glow around the lantern
    col += warm*0.22*exp(-ld*5.0);
    col += warm*0.25*exp(-ld*18.0);

    // ---- the wanderer: dark silhouette, warm rim on the lantern side, cold rim on the back
    vec3 figC = hex(6.,7.,22.);
    // a little cloth form: lighter fold strokes
    float fold = smoothstep(0.006, 0.0, abs(abs(fq.x - 0.02 + 0.25*(0.5 - fq.y)*0.3) - 0.07 - 0.1*(0.5 - fq.y)) - 0.002)*smoothstep(0.14, 0.3, fq.y)*smoothstep(0.55, 0.35, fq.y);
    figC += hex(26.,30.,70.)*fold*0.6;
    float cf = cov(fd, px);
    col = mix(col, figC, cf);
    // rims: sample the sdf a little toward each light
    vec2 toL = normalize(lanW - sp + 1e-5);
    float dW = sdTraveller(fq + toL*0.012, walk, lanL)*fh;
    float rimW = cf*smoothstep(0.0, 0.004, dW)*exp(-ld*2.5);
    col += warm*1.2*rimW;
    float dM = sdTraveller(fq + vec2(-0.012, 0.012), walk, lanL)*fh;
    col += hex(140.,150.,240.)*0.45*cf*smoothstep(0.0, 0.004, dM);
    // two bright eyes in the hood's shadow, catching the lantern
    vec2 eq = fq - vec2(0.115, 0.79);
    float eyes = min(length((eq - vec2(-0.012, 0.0))*vec2(1.0, 0.6)), length((eq - vec2(0.035, 0.0))*vec2(1.0, 0.6))) - 0.011;
    float blink = step(0.06, abs(fract(iTime*0.31 + 0.2) - 0.5));
    col = mix(col, hex(255.,226.,170.), cov(eyes*fh, px)*blink);

    // lantern: glass glow inside the cage
    float glass = sdBox(lv, vec2(0.036, 0.05)*fh);
    col = mix(col, hex(255.,236.,190.)*flick, cov(glass + 0.006*fh, px)*lamp);
    col += hex(255.,200.,120.)*lamp*exp(-ld*90.0)*0.8;

    // ---- snow in the air: flakes near the lantern catch the warm light
    for(int l=0;l<3;l++){
        float fl = float(l);
        float sc = 8.0 + fl*9.0;
        vec2 q = sp*sc + vec2((camx*(0.9 + fl*0.3) + iTime*0.15)*sc, iTime*(0.8 + fl*0.25)*sc*0.12);
        q.x += 0.3*sin(q.y*0.5 + fl*2.0 + iTime*0.6);
        vec2 gi = floor(q);
        vec2 ff = fract(q) - 0.5 - (h22(gi + fl*17.0) - 0.5)*0.6;
        float on = step(0.72, h21(gi*1.3 + fl));
        float r = 0.07 + 0.05*h21(gi + 2.0);
        float soft = l == 0 ? 0.7 : 0.3;
        float flake = (1.0 - smoothstep(r*(1.0 - soft), r, length(ff)))*on;
        vec3 fcol = mix(hex(150.,158.,230.), hex(255.,214.,150.), exp(-ld*3.5));
        col = mix(col, fcol*(1.0 + 0.6*exp(-ld*6.0)), flake*(0.35 + 0.2*fl));
    }

    // ---- foreground: a near trunk sweeps through, and blurred grass at the bottom
    float nx = sp.x + camx*1.9;
    float nt = abs(nx - 1.35) - 0.07;
    col = mix(col, hex(3.,4.,13.), smoothstep(0.012, -0.012, nt));
    col += warm*0.25*smoothstep(0.03, 0.0, abs(nt))*exp(-abs(nx - 1.35 - (lanW.x + camx*1.9 - 1.35))*1.0)*0.0;
    float gx2 = sp.x + camx*2.3;
    float gg = 1e3;
    float gc = floor(gx2/0.06);
    for(int k=-1;k<=1;k++){
        float cc = gc + float(k);
        float bx = (cc + 0.5)*0.06 + (h11(cc*3.3) - 0.5)*0.05;
        float len = 0.05 + 0.13*h11(cc*5.1);
        float lean = (h11(cc*7.7) - 0.5)*0.6; if(h11(cc*9.1) < 0.4) continue;
        gg = min(gg, sdSeg(vec2(gx2, sp.y), vec2(bx, -0.54), vec2(bx + lean*len, -0.54 + len*1.3), 0.011));
    }
    col = mix(col, hex(3.,4.,13.), smoothstep(0.022, -0.012, gg)*0.92);

    col = finish(col, fc, res);
    o = vec4(col, 1.0);
}
