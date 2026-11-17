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

    // ---- light snow: sparse fine flakes + rare large soft ones
    for(int l=0;l<2;l++){
        float fl = float(l);
        float sc = l == 0 ? 6.0 : 30.0;
        vec2 q = sp*sc + vec2(camx*(l == 0 ? 2.5 : 0.8)*sc + iTime*0.3, iTime*(l == 0 ? 0.9 : 1.2));
        q.x += 0.35*sin(q.y*0.6 + fl*2.0 + iTime*0.4);
        vec2 gi = floor(q);
        vec2 fq = fract(q) - 0.5 - (h22(gi + fl*17.0) - 0.5)*0.6;
        float on = step(l == 0 ? 0.9 : 0.80, h21(gi*1.3 + fl));
        float r = l == 0 ? 0.10 : 0.08;
        float soft = l == 0 ? 0.8 : 0.4;
        float flake = (1.0 - smoothstep(r*(1.0 - soft), r, length(fq)))*on;
        col = mix(col, hex(214.,218.,250.), flake*(l == 0 ? 0.3 : 0.6));
    }

    col = finish(col, fc, res);
    o = vec4(col, 1.0);
}
