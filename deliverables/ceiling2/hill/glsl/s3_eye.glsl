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
    stroke = furTex(hp, -0.6);
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
    float lowO = er*0.52*clamp(open*1.25 - 0.1, 0.0, 1.1);          // the lower lid leads, the upper drags behind it
    float sag = 1.0 + 0.18*(eq.x/er)*(1.0 - smoothstep(0.6, 1.0, open)); // the upper lid peels open from the outer corner
    float upper = eq.y - (lidH*sag*(1.0 - pow(eq.x/er, 2.0)) );
    float lower = -(eq.y + lowO*0.72*(1.0 - pow(eq.x/er, 2.0)));
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
    // thick fleshy upper lid catching the lantern, and a wet bead of light along the lower lid
    float lidBand = smoothstep(0.0, 0.004, -upper*0.9 + 0.022) * smoothstep(0.03, 0.012, -upper*0.9 + 0.022)*step(0.0, upper)*step(abs(eq.x), er);
    col = mix(col, hex(60.,36.,40.) + warm*0.18, lidBand*0.85*step(0.002, open));
    float wet = exp(-pow((lower + 0.008)/0.0025, 2.0))*smoothstep(er, er*0.3, abs(eq.x + 0.03))*smoothstep(0.3, 0.8, open);
    col += hex(255.,226.,190.)*wet*0.55*lamp;
    // lid fold above the eye and a crease below
    float fold = eq.y - ((er*0.56 + 0.05*clamp(open, 0.0, 1.2))*(1.0 - pow(eq.x/(er*1.15), 2.0)) + 0.01);
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

    // ---- snow: four depths, near flakes smeared and soft, warm near the lantern
    col = snowfall(col, sp, 0.0, lanW, lamp, hex(214.,218.,250.), hex(255.,214.,150.), 0.5);

    col = finish(col, fc, res);
    o = vec4(col, 1.0);
}
