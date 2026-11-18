// Shot 6: the bag. Kraft paper, a crimped top, the round label carried over from every shot before.
const vec2 C = vec2(540.0, 1100.0);
float box(vec2 p, vec2 b, float r){ vec2 d = abs(p) - b + r; return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - r; }
void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  vec3 col = field(f, hexc(24.0, 64.0, 40.0));
  vec2 bc = vec2(540.0, 1150.0);
  vec2 p = f - bc;
  // bag silhouette: slightly wider at the base, crimped top
  float wTop = 380.0, wBot = 410.0;
  float yy = clamp((p.y + 540.0) / 1080.0, 0.0, 1.0);
  float hw = mix(wBot, wTop, yy);
  float body = max(abs(p.x) - hw, abs(p.y) - 540.0);
  body = max(body, -(-p.y - 540.0 + 0.0));
  float bodyR = box(vec2(p.x * 400.0 / hw, p.y), vec2(400.0, 540.0), 26.0);
  // shadow
  float shd = box(vec2((p.x - 40.0) * 400.0 / hw, p.y + 50.0), vec2(400.0, 540.0), 26.0);
  col *= 1.0 - 0.5 * smoothstep(90.0, -20.0, shd);
  float inBag = smoothstep(1.0, -1.0, bodyR);
  // kraft paper
  vec2 kp = p * 0.02;
  float fibre = fbm(vec2(kp.x * 0.6, kp.y * 3.0)) * 0.6 + 0.4 * noise(p * 0.35);
  vec3 kraft = mix(hexc(176.0, 128.0, 82.0), hexc(206.0, 160.0, 108.0), fibre);
  kraft += (hash(floor(f)) - 0.5) * 0.03;
  // side gussets: vertical folds near the edges
  float xn = p.x / hw;
  float fold = smoothstep(0.80, 0.84, abs(xn));
  kraft *= 1.0 - 0.18 * fold * (xn > 0.0 ? 1.0 : 0.4);
  kraft *= 1.0 + 0.10 * smoothstep(0.84, 0.95, abs(xn)) * (xn < 0.0 ? 1.0 : -0.6);
  // light across the bag
  kraft *= 0.86 + 0.18 * smoothstep(1.0, -1.0, xn) * 0.8 + 0.08 * yy;
  // wrinkles
  float wr = fbm(vec2(p.x * 0.004, p.y * 0.012) + 3.0);
  kraft *= 0.95 + 0.1 * wr;
  // top band: folded over with a tin tie
  float bandY = 540.0 - 150.0;
  float band = smoothstep(bandY - 1.0, bandY + 1.0, p.y);
  kraft = mix(kraft, kraft * 0.84, band);
  kraft *= 1.0 - 0.22 * smoothstep(14.0, 0.0, abs(p.y - bandY)) * step(p.y, bandY);
  float tie = smoothstep(2.0, -2.0, box(p - vec2(0.0, bandY + 70.0), vec2(hw - 10.0, 9.0), 4.0));
  vec3 tin = hexc(70.0, 64.0, 58.0) * (0.8 + 0.4 * smoothstep(-9.0, 9.0, p.y - bandY - 70.0));
  kraft = mix(kraft, tin, tie);
  // crimped top edge
  float crimp = smoothstep(1.0, -1.0, 540.0 - 10.0 + 6.0 * abs(fract(p.x / 18.0) - 0.5) - p.y);
  kraft = mix(kraft, kraft * 0.7, crimp);
  // label: printed circle
  vec2 lp = f - C;
  float lr = length(lp);
  float lab = smoothstep(358.0, 356.0, lr);
  vec3 lc = hexc(248.0, 168.0, 120.0) * (0.95 + 0.05 * fibre);
  lc *= 1.0 - 0.06 * smoothstep(300.0, 357.0, lr) * smoothstep(-1.0, 1.0, lp.x / 357.0);
  float rule = smoothstep(2.2, 0.8, abs(lr - 332.0));
  lc = mix(lc, hexc(42.0, 18.0, 10.0), rule);
  kraft = mix(kraft, lc, lab);
  col = mix(col, kraft, inBag);
  o = vec4(col, 1.0);
}
