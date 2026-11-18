// A print coming up in the developer: darks first, unevenly, from paper white.
// Applies only inside one sheet (centre cx,cy and half-size hw,hh in frame px, rotation rot in degrees).
uniform float dev;   // = 1.0  0 blank paper, 1 fully developed
uniform float cx;    // = 960.0
uniform float cy;    // = 540.0
uniform float hw;    // = 700.0
uniform float hh;    // = 470.0
uniform float rot;   // = 0.0
void mainImage(out vec4 o, in vec2 f){
  vec2 uv = f / iResolution.xy;
  vec3 c = texture(iChannel0, uv).rgb;
  vec2 q = vec2(f.x - cx, (iResolution.y - f.y) - cy);
  float a = -rot * 0.0174533;
  q = vec2(q.x*cos(a) - q.y*sin(a), q.x*sin(a) + q.y*cos(a));
  vec2 s = q / vec2(hw, hh);
  float inside = step(abs(s.x), 0.985) * step(abs(s.y), 0.98);
  float L = dot(c, vec3(0.299, 0.587, 0.114));
  float n = sin(s.x*2.6 + 1.3) * sin(s.y*2.2 + 0.4) * 0.5 + 0.5;
  float d = dev * 1.7 - L * 0.7 - 0.14 * n;
  float k = smoothstep(0.0, 1.0, d);
  vec3 paper = vec3(0.95, 0.93, 0.89);
  o = vec4(mix(c, mix(paper, c, k), inside), 1.0);
}
