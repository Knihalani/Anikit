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

// Shot 3: the wanderer rests a hand on the "hill". An eye the size of a house opens above them.

void mainImage(out vec4 o, in vec2 fc){
    vec2 res = iResolution.xy;
    vec2 sp = (fc - 0.5*res)/res.y;
    sp = sp/zoom + vec2(camx, camy);
    float px = 1.0/(res.y*zoom);
    vec2 bo = boilOff(sp, 0.0012);

    // figure (small, bottom right) and the lantern held up
    vec2 feet = vec2(0.50, -0.42);
    float fh = 0.15;
    vec2 lanL;
    vec2 fq = (sp - feet)/fh;
    float fd = sdTraveller(fq + bo*0.5/fh, walk, lanL)*fh;
    vec2 lanW = feet + lanL*fh;
    float ld = length(sp - lanW);
    float flick = 0.92 + 0.08*vn1(iTime*9.0);
    vec3 warm = hex(255.,160.,64.)*lamp*flick;

    // ---- background: a sliver of night sky top-right, past the creature's brow
    vec2 moonP = vec2(0.95, 0.55);
    float md = length(sp - moonP);
    vec3 col = mix(hex(14.,16.,44.), hex(60.,68.,150.), exp(-md*1.8));
    col *= 0.93 + 0.14*spray(fc, 0.5, 0.9);

    // ---- the creature's head: a vast curved mass from top-left, brow line sweeping down to the right
    vec2 hp = sp + bo*0.6;
    float brow = 0.38 - 0.55*(hp.x - 0.1) - 0.35*(hp.x - 0.1)*(hp.x - 0.1);
    float dHead = (hp.y - brow)*0.85;
    dHead = brushEdge(dHead, sp, normalize(vec2(1.0, -0.6)), 0.003);
    float tD = 1e3;
    float tci = floor(hp.x/0.05);
    for(int k=-2;k<=2;k++){
        float cc = tci + float(k);
        if(h11(cc*4.3 + 2.0) < 0.35) continue;
        float tx = (cc + 0.5 + 0.5*(h11(cc*1.7) - 0.5))*0.05;
        float tb = 0.38 - 0.55*(tx - 0.1) - 0.35*(tx - 0.1)*(tx - 0.1) - 0.01;
        float th = 0.18 + 0.22*h11(cc*6.1);
        tD = min(tD, sdSpruce(hp - vec2(tx, tb), th, cc + 70.0));
    }
    float rimD = min(dHead, tD);
    dHead = min(dHead, tD);
    // cheek/jaw meets the snow at the bottom
    float c = cov(dHead, px);
    // fur: layered tuft strokes following the brow direction
    vec2 fp = hp*vec2(55.0, 75.0);
    vec2 fcl = floor(fp);
    vec2 ff = fract(fp) - 0.5 - (h22(fcl) - 0.5)*0.5;
    float ang = -0.6 + 0.3*(h21(fcl) - 0.5);
    vec2 fr = vec2(cos(ang)*ff.x + sin(ang)*ff.y, -sin(ang)*ff.x + cos(ang)*ff.y);
    float stroke = smoothstep(0.08, 0.02, abs(fr.y + 0.3*fr.x*fr.x))*smoothstep(0.42, 0.2, abs(fr.x))*step(0.4, h21(fcl + 7.0));
    vec3 body = mix(hex(8.,9.,28.), hex(20.,22.,58.), smoothstep(-0.6, 0.4, hp.x - hp.y*0.4));
    body += hex(60.,66.,150.)*0.45*smoothstep(-0.06, 0.0, hp.y - brow)*step(hp.y, brow + 0.002);
    // the lantern lights the fur near it
    // the lantern's light, drawn as a shape: a hard-edged wedge thrown up the creature's face
    vec2 lv = sp - lanW;
    float la = atan(lv.y, lv.x);
    float cone = smoothstep(0.50, 0.47, abs(la - 2.42))*step(hp.y, brow - 0.004);          // ~34 degrees wide, aimed up-left
    float reach = smoothstep(0.98, 0.80, ld + 0.08*(fbm(sp*6.0) - 0.5));
    float wedge = cone*reach;
    // falloff inside the wedge in three painted steps, stippled between them
    float fall = spray(fc, 1.0 - ld*0.95, 0.35);
    float band = floor(fall*3.0 + 0.5)/3.0;
    body = mix(body, mix(hex(70.,40.,30.), hex(232.,150.,80.), band)*(0.55 + 0.45*stroke), wedge*0.9*lamp);
    body += hex(40.,44.,100.)*stroke*0.30*(1.0 - wedge);
    body += warm*0.12*exp(-ld*4.0);
    // moonlit snow lying on the brow
    float snowOn = smoothstep(0.035, 0.0, brow - hp.y + 0.012*(fbm(hp*vec2(30.0, 10.0)) - 0.5))*step(hp.y, brow);
    body = mix(body, mix(hex(110.,118.,200.), hex(200.,204.,248.), exp(-md*1.5)), snowOn*(1.0 - 0.9*smoothstep(0.6, 1.0, eye)*smoothstep(0.2, 0.0, abs(hp.x + 0.1))*0.0));
    col = mix(col, body, c);
    col += hex(170.,178.,250.)*0.6*smoothstep(0.004 + px, 0.0, -rimD)*step(rimD, 0.0)*exp(-md*0.9);

    // ---- the eye
    vec2 ec = vec2(-0.10, 0.08);
    vec2 eq = hp - ec;
    float er = 0.24;
    float open = clamp(eye, 0.0, 1.2);
    // lids: two arcs; the opening is their intersection
    float lidH = er*0.52*open;
    float upper = eq.y - (lidH*(1.0 - pow(eq.x/er, 2.0)) );
    float lower = -(eq.y + lidH*0.75*(1.0 - pow(eq.x/er, 2.0)));
    float eyeD = max(max(upper, lower), abs(eq.x) - er);
    float ce = cov(eyeD, px)*step(0.002, open);
    // the eye ball: glowing amber-green, darker at the rim, tracking the lantern
    vec2 look = normalize(lanW - ec)*0.05*smoothstep(0.4, 1.0, open);
    vec2 iq = eq - look;
    float ir = length(iq*vec2(1.0, 1.05));
    vec3 iris = mix(hex(255.,226.,140.), hex(232.,150.,40.), smoothstep(0.0, 0.09, ir));
    iris = mix(iris, hex(150.,70.,20.), smoothstep(0.08, 0.135, ir));
    iris = mix(iris, hex(26.,12.,6.), smoothstep(0.128, 0.142, ir));      // limbal ring
    iris = mix(iris, hex(46.,28.,20.), smoothstep(0.142, 0.16, ir));      // dark sclera
    // radial fibres in the iris
    float fib = vn(vec2(atan(iq.y, iq.x)*18.0, ir*30.0));
    iris *= 0.8 + 0.35*fib;
    // vertical slit pupil, dilating as it finds the light
    float pw = mix(0.010, 0.045, smoothstep(0.7, 1.2, eye));
    float pupil = sdEll(iq, vec2(pw, 0.12));
    iris = mix(iris, hex(4.,3.,8.), cov(pupil, px));
    // lantern reflection: a tiny warm window + the wanderer silhouette in it
    vec2 rq = iq - vec2(0.055, 0.045);
    iris = mix(iris, hex(255.,240.,210.), cov(sdBox(rq, vec2(0.010, 0.014)) - 0.003, px)*lamp);
    iris += hex(255.,220.,170.)*0.4*exp(-length(rq)*60.0)*lamp;
    // eyeball shading: top shadowed by the lid
    iris *= mix(0.25, 1.0, smoothstep(lidH, lidH*0.0, eq.y));
    iris *= mix(0.6, 1.0, smoothstep(er, er*0.5, abs(eq.x)));
    // wet specular: a soft moon highlight on the upper left of the eyeball
    iris += hex(200.,210.,255.)*0.5*exp(-pow(length((eq - vec2(-0.07, 0.04))*vec2(1.0, 2.0))/0.02, 2.0));
    col = mix(col, iris, ce);
    // glow spilling from the eye onto the fur and snow
    float eglow = exp(-max(eyeD, 0.0)*14.0)*smoothstep(0.1, 0.8, open);
    col += hex(255.,190.,90.)*0.30*eglow*(1.0 - ce);
    // lid line (dark wet rim)
    col = mix(col, hex(4.,4.,12.), cov(abs(eyeD) - 0.006, px)*step(0.002, open));
    // lid fold above the eye and a crease below
    float fold = eq.y - (er*0.62*(1.0 - pow(eq.x/(er*1.15), 2.0)) + 0.01);
    col = mix(col, hex(26.,28.,70.) + warm*0.08, smoothstep(0.006, 0.0, abs(fold) - 0.002)*step(abs(eq.x), er*1.1)*smoothstep(0.0, 0.4, open)*0.8);
    float crease = -(eq.y + er*0.50*(1.0 - pow(eq.x/(er*1.05), 2.0)) + 0.01);
    col = mix(col, hex(26.,28.,70.), smoothstep(0.005, 0.0, abs(crease) - 0.0015)*step(abs(eq.x), er*0.9)*smoothstep(0.0, 0.4, open)*0.6);

    // ---- snow ground at the bottom
    float g = -0.42 + 0.015*sin(sp.x*3.0) + 0.004*(fbm1(sp.x*14.0) - 0.5);
    float cg = cov(sp.y - g, px);
    float dep = clamp((g - sp.y)/0.12, 0.0, 1.0);
    vec3 snow = mix(hex(110.,118.,198.), hex(30.,34.,86.), pow(dep, 0.7));
    snow = mix(snow, hex(190.,196.,244.), smoothstep(0.01, 0.0, g - sp.y)*0.8);
    snow += warm*0.6*exp(-pow(length((sp - vec2(lanW.x, g - 0.01))*vec2(0.8, 3.0))/0.22, 2.0));
    snow += hex(255.,190.,90.)*0.15*smoothstep(0.1, 0.8, open)*exp(-length(sp - vec2(-0.1, g))*2.0);
    snow *= 0.94 + 0.12*spray(fc, 0.5, 0.8);
    col = mix(col, snow, cg);

    // ---- wanderer
    col += warm*0.18*exp(-ld*6.0) + warm*0.2*exp(-ld*20.0);
    float cf = cov(fd, px);
    col = mix(col, hex(5.,6.,20.), cf);
    vec2 toL = normalize(lanW - sp + 1e-5);
    float dW = sdTraveller(fq + toL*0.03, walk, lanL)*fh;
    col += warm*1.0*cf*smoothstep(0.0, 0.003, dW)*exp(-ld*3.0);
    // eyes of the wanderer
    vec2 wq = fq - vec2(0.115, 0.79);
    float weyes = min(length((wq - vec2(-0.012, 0.0))*vec2(1.0, 0.6)), length((wq - vec2(0.035, 0.0))*vec2(1.0, 0.6))) - 0.012;
    col = mix(col, hex(255.,226.,170.), cov(weyes*fh, px));
    float glass = sdBox(sp - lanW, vec2(0.036, 0.05)*fh);
    col = mix(col, hex(255.,236.,190.)*flick, cov(glass + 0.006*fh, px)*lamp);
    col += hex(255.,200.,120.)*lamp*exp(-ld*90.0)*0.8;

    // ---- snow sifting off the brow when the eye opens, plus steady fall
    for(int l=0;l<3;l++){
        float fl = float(l);
        float sc = 9.0 + fl*9.0;
        vec2 q = sp*sc + vec2(iTime*0.15*sc, iTime*(0.8 + fl*0.25)*sc*0.12);
        q.x += 0.3*sin(q.y*0.5 + fl*2.0 + iTime*0.6);
        vec2 gi = floor(q);
        vec2 ff2 = fract(q) - 0.5 - (h22(gi + fl*17.0) - 0.5)*0.6;
        float on = step(0.88, h21(gi*1.3 + fl));
        float r = 0.07 + 0.05*h21(gi + 2.0);
        float flake = (1.0 - smoothstep(r*(1.0 - (l == 0 ? 0.7 : 0.3)), r, length(ff2)))*on;
        vec3 fcol = mix(hex(150.,158.,230.), hex(255.,214.,150.), max(exp(-ld*4.0), 0.6*eglow));
        col = mix(col, fcol, flake*(0.35 + 0.2*fl));
    }
    col = finish(col, fc, res);
    o = vec4(col, 1.0);
}
