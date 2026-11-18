// Low Tide — the harbour lights write the lyric.
// iChannel0: the lyric, white on black (its opacity is each lamp's power).
// mode 0: lamps on the quay, mirrored in water (tide in) or puddled mud (tide out).
// mode 1: the lyric cut into wet sand, seen from above; the tide line washes it out.
// mode 2: the camera sees only the water; the lyric reads in the reflection.
uniform float mode;     // = 0.0
uniform float horizon;  // = 0.55  water line, fraction of height from the bottom
uniform float pitch;    // = 15.0  lamp spacing in px
uniform float tideout;  // = 0.0   0 = water, 1 = mud flats with puddles
uniform float warmth;   // = 0.0   lamp colour: 0 sodium-white, 1 amber
uniform float night;    // = 0.0   sky: 0 blue hour, 1 night
uniform float outx;     // = 9999.0 lamps right of this x (px) are out
uniform float zoom;     // = 1.0
uniform float wash;     // = -200.0 mode 1: tide line y (px from the bottom)
uniform float glowk;    // = 1.0
uniform float fade;     // = 1.0
uniform float panx;     // = 0.0   camera travel along the harbour (px)

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p) {
    float a = 0.5, s = 0.0;
    for (int k = 0; k < 4; k++) { s += a * vnoise(p); p *= 2.03; a *= 0.5; }
    return s;
}

vec3 lampColour(float h) {
    vec3 cool = vec3(1.0, 0.86, 0.66);
    vec3 amber = vec3(1.0, 0.56, 0.20);
    return mix(cool, amber, clamp(warmth + 0.15 * (h - 0.5), 0.0, 1.0));
}

// Light from the lamp grid at field point p; sy squashes distance vertically (reflections streak).
vec3 lamps(vec2 p, float sy, float t) {
    vec3 acc = vec3(0.0);
    vec2 id0 = floor(p / pitch);
    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 id = id0 + vec2(float(i), float(j));
            float h = hash(id);
            vec2 c = (id + 0.5) * pitch;
            vec2 uv = c / iResolution.xy;
            if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) continue;
            float m = texture(iChannel0, uv).r;
            // Lamps switch on in their own order as the word's power rises.
            float on = smoothstep(h * 0.75, h * 0.75 + 0.25, m);
            // Bulbs fail one by one: each has its own moment as the dark sweeps past, and
            // stutters just before it goes.
            float k = (outx - c.x - (h - 0.5) * 900.0) / 120.0;
            float alive = clamp(k, 0.0, 1.0);
            alive *= mix(1.0, step(0.5, fract(sin(floor(t * 24.0) * 12.9898 + h * 78.233) * 43758.5453)), step(k, 0.9) * step(0.0, k));
            on *= alive;
            if (on <= 0.0) continue;
            // Real lamps: a little off the grid, each its own brightness, a slow flicker.
            float h2 = hash(id + 17.3);
            c += (vec2(h2, hash(id + 5.1)) - 0.5) * pitch * 0.08;
            float power = 0.8 + 0.3 * h2 * h2;
            on *= power * (0.93 + 0.07 * sin(t * (4.0 + 5.0 * h) + h * 40.0));
            vec2 d = p - c;
            d.y *= sy;
            float r2 = dot(d, d);
            float core = exp(-r2 / (pitch * pitch * (0.014 + 0.02 * h2)));
            float halo = 0.06 / (1.0 + r2 / (pitch * pitch * 0.25));
            halo *= smoothstep(pitch * pitch * 2.2, pitch * pitch * 0.6, r2); // fits inside the 3×3 cells
            // Incandescent warm-up: a filament starts deep orange and whitens as it reaches power.
            vec3 lc = mix(vec3(1.0, 0.32, 0.06), lampColour(h), smoothstep(0.0, 1.0, on));
            acc += lc * on * (core * 2.2 + halo * glowk);
        }
    }
    return acc;
}

// Mist: the lamps' light scattered in the air (a wide blur of the lyric).
vec3 mist(vec2 p) {
    float g = 0.0;
    for (int k = 0; k < 8; k++) {
        float a = float(k) * 0.785398;
        vec2 uv = (p + vec2(cos(a), sin(a)) * pitch * 3.5) / iResolution.xy;
        g += texture(iChannel0, uv).r;
    }
    g += 2.0 * texture(iChannel0, p / iResolution.xy).r;
    return lampColour(0.5) * g * 0.006;
}

vec3 sky(float y, float t) {
    vec3 blueTop = vec3(0.04, 0.07, 0.16), blueLow = vec3(0.22, 0.24, 0.36);
    vec3 nightTop = vec3(0.008, 0.010, 0.022), nightLow = vec3(0.05, 0.045, 0.07);
    vec3 top = mix(blueTop, nightTop, night), low = mix(blueLow, nightLow, night);
    low = mix(low, low + vec3(0.10, 0.05, 0.0), warmth);
    return mix(low, top, pow(clamp(y, 0.0, 1.0), 0.7));
}

// The moon, low over the water on the right; it brightens as the night deepens.
vec3 moon(vec2 p) {
    vec2 mc = vec2(0.90, 0.92) * iResolution.xy;
    float d = length(p - mc);
    float disc = smoothstep(26.0, 22.0, d);
    float glow = 0.18 / (1.0 + d * d / 9000.0);
    return vec3(0.75, 0.80, 0.92) * (disc * 0.9 + glow) * (0.25 + 0.75 * night);
}

void mainImage(out vec4 o, in vec2 f) {
    float t = iTime;
    vec2 res = iResolution.xy;
    vec2 ctr = 0.5 * res;
    // Camera: zoom about the centre, plus a slow vertical drift.
    vec2 p = (f - ctr) / zoom + ctr + vec2(panx, 0.0);
    float reflonly = step(1.5, mode);
    float rgain = 1.0 + 0.5 * reflonly;
    vec3 col;

    if (mode < 0.5 || mode > 1.5) {
        float hy = horizon * res.y;
        if (p.y >= hy) {
            float sy = (p.y - hy) / max(res.y - hy, 1.0);
            col = sky(sy, t) + moon(p) * (1.0 - warmth * 0.6);
            // Haze glow that the lamps throw into the air near the horizon.
            col += lamps(p, 1.0, t) + mist(p) * glowk * step(p.x, outx + 200.0);
            // The quay: a thin dark band with posts, just above the water.
            // The far shore: a low dark line with mooring posts and a crane.
            float shore = hy + 6.0 + 10.0 * fbm(vec2(p.x * 0.004, 1.0));
            float post = step(fract(p.x / 173.0 + 0.3), 0.012) * step(p.y, hy + 38.0);
            float sil = max(step(p.y, shore), post);
            col = mix(col, vec3(0.010, 0.010, 0.016), sil * 0.92);
        } else {
            float depth = clamp((hy - p.y) / max(hy, 1.0), 0.0, 1.0); // 0 at the waterline
            // Mud flats: puddles hold the reflections, mud between them is dull.
            float n = fbm(vec2(p.x * 0.004, p.y * 0.012 / (0.2 + depth)) + 3.0);
            float puddle = mix(1.0, smoothstep(0.50, 0.56, n), tideout);
            // Wavelets: small horizontal displacement, and the streaks break into dashes.
            float rip = vnoise(vec2(p.x * 0.02, p.y * mix(0.35 / (0.15 + depth), 0.06, reflonly) - t * 1.3)) - 0.5;
            float calm = 1.0 - 0.7 * tideout;
            // The returning tide (mode 2): a surge from below that breaks up whatever it reaches.
            float surge = reflonly * smoothstep(wash + 70.0, wash - 70.0, f.y);
            vec2 q = vec2(p.x + rip * (mix(3.0 + 14.0 * depth, 5.0, reflonly) + 70.0 * surge) * calm, mix(2.0 * hy - p.y, p.y, reflonly) + rip * 40.0 * surge);
            float dash = smoothstep(0.25, 0.75, vnoise(vec2(p.x * 0.05, p.y * 0.45 / (0.15 + depth) + t * 0.9)));
            vec3 refl = lamps(q, mix(0.18, 0.8, reflonly), t) * (0.7 - 0.3 * depth * (1.0 - reflonly)) * mix(1.0, 0.35 + 0.9 * dash, calm) * rgain;
            refl += mist(q) * 0.6 * rgain * glowk;
            refl *= 1.0 - 0.75 * surge;
            // The surface: the sky's gradient broken by moving wavelets.
            float wv = fbm(vec2(p.x * 0.004 + t * 0.04, p.y * 0.018 / (0.1 + depth) - t * 0.3));
            float sheen = smoothstep(0.35, 0.85, wv);
            vec3 skyRefl = sky(mix(0.15, 0.6, f.y / res.y), t);
            vec3 water = vec3(0.006, 0.01, 0.02) + skyRefl * (0.18 + 0.55 * sheen) + refl;
            // Glints: crests catching the sky, twinkling as the wavelets move.
            vec2 gc = floor(vec2(p.x / 5.0, p.y / 3.0));
            float glint = step(0.993, hash(gc + floor(t * 6.0 + hash(gc) * 6.0))) * sheen;
            water += vec3(0.55, 0.62, 0.8) * glint * 0.6 * (1.0 - 0.5 * tideout);
            // The moon's path on the water, broken by wavelets.
            float mpath = exp(-pow((q.x - 0.90 * res.x) / (30.0 + 160.0 * depth + 220.0 * reflonly), 2.0));
            water += vec3(0.6, 0.66, 0.8) * mpath * sheen * 0.25 * (0.25 + 0.75 * night) * (1.0 - warmth * 0.6);
            vec3 mud = mix(vec3(0.018, 0.016, 0.014), vec3(0.04, 0.035, 0.03), fbm(p * 0.02)) * (1.0 + 0.6 * night * 0.0);
            mud += refl * 0.06;
            col = mix(mud, water, puddle);
            // Boats left on the mud by the tide: dark hulls with masts, tilted on their keels.
            for (int b = 0; b < 3; b++) {
                float fb = float(b);
                vec2 bc = vec2((0.18 + 0.33 * fb) * res.x + panx * 0.0, hy - (40.0 + 70.0 * fb));
                vec2 d = p - bc;
                float tilt = 0.12 * (fb - 1.0);
                d = vec2(d.x * cos(tilt) - d.y * sin(tilt), d.x * sin(tilt) + d.y * cos(tilt));
                float sz = 1.0 + 0.5 * fb;
                float hull = step(abs(d.x) / (120.0 * sz) + max(d.y, 0.0) / (34.0 * sz), 1.0) * step(-26.0 * sz, d.y) * step(d.y, 6.0 * sz);
                float mast = step(abs(d.x - 10.0 * sz), 2.0 * sz) * step(0.0, d.y) * step(d.y, 230.0 * sz);
                col = mix(col, vec3(0.006, 0.006, 0.01), max(hull, mast) * tideout);
            }
            // Foam along the surge's edge.
            float edge = wash + 70.0 * (fbm(vec2(f.x * 0.004 + t * 0.2, t * 0.3)) - 0.5);
            float lace = smoothstep(0.45, 0.8, fbm(vec2(f.x * 0.02, f.y * 0.04 - t * 2.0)));
            col += vec3(0.62, 0.68, 0.78) * reflonly * exp(-pow((f.y - edge) / 26.0, 2.0)) * lace * 0.3;
            // The waterline sparkle.
            col += lampColour(0.5) * 0.04 * exp(-depth * 40.0) * glowk;
        }
    } else {
        // Wet sand from above, raked by the moon; the lyric is pressed into it.
        vec2 uv = f / res;
        float e = 2.5;
        float m0 = texture(iChannel0, uv).r;
        float mx = texture(iChannel0, uv + vec2(e / res.x, 0.0)).r - texture(iChannel0, uv - vec2(e / res.x, 0.0)).r;
        float my = texture(iChannel0, uv + vec2(0.0, e / res.y)).r - texture(iChannel0, uv - vec2(0.0, e / res.y)).r;
        float ripx = sin((p.x * 0.55 + p.y) * 0.04 + 3.0 * fbm(p * 0.004)) * 0.5 + 0.2 * sin((p.x - p.y * 0.3) * 0.09);
        // Under the returning tide the letters soften away.
        float under = smoothstep(wash + 40.0, wash - 40.0, f.y);
        float keep = 1.0 - 0.9 * under;
        vec3 nrm = normalize(vec3(-(mx * 4.0 * keep) - ripx * 0.22, -(my * 4.0 * keep) - 0.06 * cos(p.y * 0.05), 1.0));
        vec3 L = normalize(vec3(-0.6, 0.55, 0.5));
        float diff = max(dot(nrm, L), 0.0);
        vec3 H = normalize(L + vec3(0.0, 0.0, 1.0));
        float spec = pow(max(dot(nrm, H), 0.0), 40.0);
        float tex = fbm(p * 0.03);
        vec3 sand = mix(vec3(0.13, 0.14, 0.17), vec3(0.22, 0.23, 0.27), tex) * (0.25 + 1.1 * diff);
        // Wet grains catching the moon.
        float glint = step(0.985, hash(floor(p / 3.0))) * (0.5 + 0.5 * sin(t * 3.0 + hash(floor(p / 3.0) + 2.0) * 30.0));
        sand += vec3(0.7, 0.78, 0.95) * (spec * 0.9 + glint * 0.5 * (1.0 - under));
        // The letters hold standing water that mirrors the moon: silver light in dark sand.
        sand *= 0.6;
        float shimmer = 0.75 + 0.25 * fbm(vec2(p.x * 0.02 + t * 0.4, p.y * 0.05));
        vec3 pool = vec3(0.50, 0.58, 0.74) * shimmer + vec3(0.9, 0.95, 1.0) * spec;
        sand = mix(sand, pool, smoothstep(0.2, 0.8, m0) * keep);
        // Moonlight falls off toward the bottom right.
        sand *= 0.75 + 0.5 * (f.y / res.y) - 0.15 * (f.x / res.x);
        float foam = exp(-pow((f.y - wash) / 10.0, 2.0)) * (0.6 + 0.4 * fbm(vec2(f.x * 0.02, t * 2.0)));
        vec3 waterc = vec3(0.02, 0.03, 0.05) + spec * vec3(0.4, 0.45, 0.55) * 0.6;
        col = mix(sand, waterc, under * 0.8) + vec3(0.7, 0.75, 0.8) * foam * 0.3;
    }
    // Film: a little vignette.
    vec2 v = f / res - 0.5;
    col *= 1.0 - 0.45 * dot(v, v);
    o = vec4(col * fade, 1.0);
}
