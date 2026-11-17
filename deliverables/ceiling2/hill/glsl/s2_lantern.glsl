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
