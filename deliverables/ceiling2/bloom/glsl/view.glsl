// Dark-field microscope: the field's membranes become glowing filaments, its body fine contour lines.
float h11(float p){ p = fract(p*0.1031); p *= p + 33.33; p *= p + p; return fract(p); }
float h21(vec2 p){ vec3 p3 = fract(vec3(p.xyx)*0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y)*p3.z); }
float V(vec2 uv){ return texture(iChannel0, uv).g; }

void mainImage(out vec4 o, in vec2 fc){
    vec2 res = iResolution.xy;
    float beat = (iTime - PRE)*BPM/60.0;
    // cut grid: every 2 beats, then every beat in the drop (beats 8-16), then the long outro
    float cut = beat < 8.0 ? floor(max(beat, 0.0)/2.0) : (beat < 16.0 ? 4.0 + floor(beat - 8.0) : 12.0);
    float outro = smoothstep(16.0, 19.5, beat);
    float pulse = exp(-fract(max(beat, 0.0))*5.0)*step(4.0, beat);
    float k = mod(cut, 3.0);
    float seed = cut*1.37 + 0.5;
    float zoomed = step(0.5, mod(cut, 2.0))*(1.0 - outro);
    vec2 q = (fc - 0.5*res)/res.x;
    float inCut = beat < 8.0 ? fract(beat/2.0) : fract(beat);
    float z = mix(1.3, 1.7 + 0.8*h11(seed), zoomed)*(1.0 + 0.05*inCut);
    z = mix(z, 0.86, outro);
    float ca = h11(seed + 1.0)*6.2832, cr = 0.28*sqrt(h11(seed + 2.0));
    vec2 cen = vec2(cos(ca), sin(ca))*cr*zoomed*(1.0 - outro);
    float rot = (h11(seed + 3.0) - 0.5)*1.4*zoomed;
    vec2 p = vec2(cos(rot)*q.x - sin(rot)*q.y, sin(rot)*q.x + cos(rot)*q.y)/z + cen;
    vec2 uv = vec2(p.x + 0.5, p.y*(res.x/res.y) + 0.5);
    vec2 tx = 1.0/res;
    float v = V(uv);
    float vx = V(uv + vec2(tx.x, 0.0)) - V(uv - vec2(tx.x, 0.0));
    float vy = V(uv + vec2(0.0, tx.y)) - V(uv - vec2(0.0, tx.y));
    vec2 gr = vec2(vx, vy)*0.5/tx.x*z;          // gradient per screen px * res
    float gl = length(gr) + 1e-5;
    // membrane wall: the iso-line v = 0.18, drawn as a thin bright filament with constant screen width
    float wall = exp(-pow((v - 0.18)/(gl*1.6/res.x), 2.0));
    // inner contour lines (fine engraving)
    float N = 22.0;
    float cl = abs(fract(v*N) - 0.5)/(N*gl/res.x);
    float lines = exp(-cl*cl*1.4)*smoothstep(0.18, 0.26, v);
    // body: soft translucent fill, brighter where thick
    float body = smoothstep(0.12, 0.40, v);
    // light: normal from the field, key from upper left
    vec3 n = normalize(vec3(-gr*0.02, 1.0));
    float diff = clamp(dot(n, normalize(vec3(-0.5, 0.6, 0.6))), 0.0, 1.0);
    float spec = pow(max(dot(n, normalize(vec3(-0.25, 0.3, 1.0))), 0.0), 40.0)*body;

    vec3 ink, glow, bg;
    if(k < 0.5){ bg = vec3(0.006, 0.006, 0.008); ink = vec3(0.86, 0.84, 0.80); glow = vec3(1.0, 0.96, 0.9); }
    else if(k < 1.5){ bg = vec3(0.020, 0.006, 0.003); ink = vec3(1.0, 0.36, 0.10); glow = vec3(1.0, 0.82, 0.55); }
    else { bg = vec3(0.80, 0.78, 0.74); ink = vec3(0.02, 0.02, 0.025); glow = vec3(0.0); }
    bool inv = k > 1.5 && outro < 0.5;
    // the outro settles to bone white
    ink = mix(ink, vec3(0.86, 0.84, 0.80), outro); glow = mix(glow, vec3(1.0, 0.96, 0.9), outro); bg = mix(bg, vec3(0.006), outro);

    vec3 col = bg;
    if(inv){
        // printed plate: black ink membranes on bone paper
        float ink_ = clamp(wall*1.2 + lines*0.5 + body*0.35, 0.0, 1.0);
        col = mix(bg, ink, ink_);
        float dr0 = length(p) - 0.468;
        col = mix(col, ink, exp(-pow(dr0/(0.0016/z), 2.0)));
        vec2 qq = q*vec2(1.0, 0.56);
        col *= 1.0 - 0.25*dot(qq, qq)*4.0;
        col += (h21(fc + floor(iTime*30.0)*vec2(3.1, 7.7)) - 0.5)*0.02;
        o = vec4(pow(clamp(col, 0.0, 1.0), vec3(1.0)), 1.0);
        return;
    }
    col += ink*body*(0.10 + 0.25*diff);
    col += ink*lines*0.45;
    col += glow*wall*(1.1 + 0.8*pulse);
    col += glow*spec*0.5;
    // halo around walls (bloom-like, cheap): wider gaussian of the same iso-line
    col += ink*0.07*exp(-pow((v - 0.18)/(gl*9.0/res.x), 2.0));
    // the dish's glass wall: a thin ring with two specular arcs
    float dr = length(p) - 0.468;
    float ring = exp(-pow(dr/(0.0016/z), 2.0));
    float ang = atan(p.y, p.x);
    float arcs = pow(max(cos(ang - 2.2), 0.0), 8.0) + 0.6*pow(max(cos(ang + 0.9), 0.0), 14.0);
    col += glow*ring*(0.25 + 1.4*arcs);
    col += glow*0.05*exp(-abs(dr)*60.0*z)*arcs;
    float vig = 1.0 - 0.45*pow(length(q*vec2(1.0, 0.56))*1.6, 2.0);
    col *= clamp(vig, 0.0, 1.0);
    col += (h21(fc + floor(iTime*30.0)*vec2(3.1, 7.7)) - 0.5)*0.01;
    col = 1.0 - exp(-col*1.5);
    o = vec4(pow(max(col, 0.0), vec3(0.4545)), 1.0);
}
