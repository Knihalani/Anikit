fn rot(px, py, a) { let c = cos(a); let s = sin(a); [px*c - py*s, px*s + py*c] }
fn lerp2(a, b, t) { a + (b - a) * t }
fn wingpts(ph, fold, side) {
  let open = [[10.0,5.0],[18.0,30.0],[20.0,58.0],[4.0,100.0],[-22.0,150.0],[-40.0,168.0],[-46.0,160.0],[-50.0,140.0],[-52.0,118.0],[-50.0,96.0],[-44.0,70.0],[-40.0,46.0],[-30.0,20.0],[-20.0,5.0]];
  let shut = [[6.0,5.0],[4.0,12.0],[-6.0,16.0],[-30.0,16.0],[-60.0,14.0],[-92.0,10.0],[-96.0,8.0],[-92.0,7.0],[-84.0,7.0],[-74.0,7.0],[-60.0,7.0],[-46.0,6.0],[-32.0,5.0],[-20.0,4.0]];
  let tau = 2.0 * PI();
  let phi = (50.0 * cos(tau * ph) - 6.0) * (1.0 - fold);
  let phi2 = (58.0 * cos(tau * ph - 0.6) - 4.0) * (1.0 - fold);
  let sweep = 1.0 - 0.18 * (0.5 + 0.5 * sin(tau * ph));
  let a1 = phi.to_radians();
  let a2 = phi2.to_radians();
  let out = [];
  let k = 0;
  for p in open {
    let q = shut[k];
    let x = lerp2(p[0], q[0], fold);
    let z = lerp2(p[1], q[1], fold);
    let y = 0.0;
    if z > 58.0 {
      let zr = 58.0;
      let dz = (z - 58.0) * sweep;
      let yy = zr * sin(a1) + dz * sin(a2);
      let zz = zr * cos(a1) + dz * cos(a2);
      y = yy; z = zz;
    } else {
      y = z * sin(a1); z = z * cos(a1);
    }
    out.push([x, y, z * side]);
    k += 1;
  }
  out
}
fn proj(p, pitch, yaw) {
  let r = rot(p[0], p[1], pitch);
  let zz = -r[0] * sin(yaw) + p[2] * cos(yaw);
  let el = -0.5;
  [r[0] * cos(yaw) + p[2] * sin(yaw), r[1] * cos(el) - zz * sin(el)]
}
fn screen(pts, pitch, yaw, ox, oy, s, hx, hy) {
  let out = [];
  for p in pts {
    let q = proj(p, pitch, yaw);
    out.push([ox + s * (q[0] - hx), oy - s * (q[1] - hy)]);
  }
  out
}
// returns draw items for one bird. ph: wing phase 0..1, fold 0..1, ox/oy head position, s scale
fn bird(ph, fold, ox, oy, s, pitchdeg, yawdeg, col, far, eye) {
  let pitch = pitchdeg.to_radians();
  let yaw = yawdeg.to_radians();
  let head = proj([56.0, 8.0, 0.0], pitch, yaw);
  let hx = head[0]; let hy = head[1];
  let body = [[69.0,6.0,0.0],[66.0,13.0,0.0],[59.0,16.0,0.0],[48.0,14.0,0.0],[34.0,12.0,0.0],[10.0,10.0,0.0],[-20.0,6.0,0.0],[-38.0,3.0,0.0],[-38.0,-3.0,0.0],[-22.0,-9.0,0.0],[4.0,-13.0,0.0],[30.0,-11.0,0.0],[44.0,-6.0,0.0],[56.0,-2.0,0.0],[65.0,0.0,0.0]];
  let tilt = (-28.0 * (1.0 - fold)).to_radians();
  let spread = 1.0 - 0.7 * fold;
  let tail = [];
  for p in [[0.0,-5.0],[-66.0,-24.0*spread],[-74.0,-12.0*spread],[-77.0,0.0],[-74.0,12.0*spread],[-66.0,24.0*spread],[0.0,5.0]] {
    let r = rot(p[0], 0.0, tilt);
    tail.push([-36.0 + r[0], r[1], p[1]]);
  }
  let beak = [[67.0,8.0,0.0],[75.0,5.0,0.0],[74.0,0.0,0.0],[67.0,2.0,0.0]];
  let items = [];
  items.push(#{ points: screen(wingpts(ph, fold, -1.0), pitch, yaw, ox, oy, s, hx, hy), closed: true, fill: far });
  items.push(#{ points: screen(tail, pitch, yaw, ox, oy, s, hx, hy), closed: true, smooth: true, fill: col });
  items.push(#{ points: screen(body, pitch, yaw, ox, oy, s, hx, hy), closed: true, smooth: true, fill: col });
  items.push(#{ points: screen(beak, pitch, yaw, ox, oy, s, hx, hy), closed: true, fill: col });
  items.push(#{ points: screen(wingpts(ph, fold, 1.0), pitch, yaw, ox, oy, s, hx, hy), closed: true, fill: col });
  if eye != "" {
    let e = screen([[61.0, 10.0, 0.0]], pitch, yaw, ox, oy, s, hx, hy)[0];
    items.push(#{ circle: [e[0], e[1], 2.2 * s], fill: eye });
  }
  items
}
