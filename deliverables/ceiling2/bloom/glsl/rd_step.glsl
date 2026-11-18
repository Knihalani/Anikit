// One Gray-Scott step. iChannel0 = previous state (r = u, g = v). Spatially varying feed/kill gives coral, worms and spots in one dish.
// anikit:aa off
float h21(vec2 p){ vec3 p3 = fract(vec3(p.xyx)*0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y)*p3.z); }
#define SC 3.6
void mainImage(out vec4 o, in vec2 fc){
    vec2 res = iResolution.xy;
    vec2 px1 = 1.0/res;
    vec2 px = SC/res;
    vec2 uv = fc*px1;
    vec4 c0 = texture(iChannel0, uv);
    // couple neighbouring pixels so the coarse stencil stays one smooth field
    vec4 c = 0.9*c0 + 0.025*(texture(iChannel0, uv + vec2(px1.x, 0.0)) + texture(iChannel0, uv - vec2(px1.x, 0.0)) + texture(iChannel0, uv + vec2(0.0, px1.y)) + texture(iChannel0, uv - vec2(0.0, px1.y)));
    if(iFrame < 1 || (c0.r + c0.g) < 1e-6){
        // seed: a cluster of irregular spores near the centre + a few strays
        vec2 q = (fc - 0.5*res)/res.y;
        float v = 0.0;
        for(int i=0;i<40;i++){
            float fi = float(i);
            vec2 s = vec2(h21(vec2(fi, 1.0)) - 0.5, h21(vec2(fi, 7.0)) - 0.5)*vec2(0.56, 1.0);
            if(i < 8) s *= 0.2;
            v = max(v, step(length(q - s), 0.008 + 0.014*h21(vec2(fi, 3.0))));
        }
        o = vec4(1.0, v*0.5 + 0.0, 0.0, 1.0);
        return;
    }
    vec4 l = texture(iChannel0, uv + vec2(-px.x, 0.0));
    vec4 r = texture(iChannel0, uv + vec2( px.x, 0.0));
    vec4 d = texture(iChannel0, uv + vec2(0.0, -px.y));
    vec4 t = texture(iChannel0, uv + vec2(0.0,  px.y));
    vec4 a = texture(iChannel0, uv + vec2(-px.x, -px.y));
    vec4 b = texture(iChannel0, uv + vec2( px.x, -px.y));
    vec4 e = texture(iChannel0, uv + vec2(-px.x,  px.y));
    vec4 g = texture(iChannel0, uv + vec2( px.x,  px.y));
    vec2 lap = (0.2*(l.xy + r.xy + d.xy + t.xy) + 0.05*(a.xy + b.xy + e.xy + g.xy) - c.xy);
    float u = c.x, v = c.y;
    // feed/kill map: vertical drift from coral (top) to mitosis/worms (bottom), plus a slow swirl
    vec2 q = (fc - 0.5*res)/res.y;
    float m = smoothstep(-0.25, 0.35, q.y + 0.15*sin(q.x*5.0 + 1.0));
    float F = mix(0.0545, 0.030, m);      // labyrinth coral below, dividing cells above
    float K = mix(0.062, 0.062, m);
    float uvv = u*v*v;
    u += 1.0*lap.x - uvv + F*(1.0 - u);
    v += 0.5*lap.y + uvv - (F + K)*v;
    // on each kick (from the score's grid) new spores land at a beat-chosen spot
    float beat = (iTime - PRE)*BPM/60.0;
    float bn = floor(beat);
    float kick = step(0.0, beat)*step(fract(beat), 0.05);
    vec2 sp = vec2(h21(vec2(bn, 2.0)) - 0.5, h21(vec2(bn, 5.0)) - 0.5)*vec2(0.45, 0.85);
    v = max(v, kick*0.5*step(length(q - sp), 0.03));
    // the dish: nothing grows past its wall
    vec2 dq = (fc - 0.5*res)/res.x;
    float dish = smoothstep(0.47, 0.455, length(dq));
    v *= dish; u = mix(1.0, u, dish);
    o = vec4(clamp(u, 0.0, 1.0), clamp(v, 0.0, 1.0), 0.0, 1.0);
}
