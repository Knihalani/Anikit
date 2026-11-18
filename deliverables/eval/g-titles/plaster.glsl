// Lime-washed plaster: warm, low-contrast, trowel marks and pores.
float h(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float n(vec2 p){ vec2 i = floor(p), f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(h(i), h(i+vec2(1,0)), f.x), mix(h(i+vec2(0,1)), h(i+vec2(1,1)), f.x), f.y); }
float fbm(vec2 p){ float a = 0.5, s = 0.0; for (int i = 0; i < 5; i++){ s += a*n(p); p = p*2.03 + 17.0; a *= 0.5; } return s; }
void mainImage(out vec4 o, in vec2 f){
  vec2 uv = f / iResolution.y;
  float big = fbm(uv * 3.0);
  float trowel = fbm(vec2(uv.x * 1.5 + uv.y * 0.6, uv.y * 9.0));
  float pores = smoothstep(0.78, 0.92, n(f * 0.35)) * 0.5;
  vec3 base = vec3(0.925, 0.900, 0.850);
  vec3 c = base * (0.95 + 0.07 * big + 0.04 * trowel) - pores * 0.035;
  // light from upper left, falling off
  c *= 1.02 - 0.10 * length(uv - vec2(0.3, -0.2)) * 0.6;
  o = vec4(c, 1.0);
}
