import React from "react";
import { Easing, interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { At, C, Stage, fadeUp, inOutCubic, outCubic, outExpo, prog } from "./common";

const box = (l: number, t: number, w: number, h: number, extra: React.CSSProperties = {}): React.CSSProperties => ({
  position: "absolute", left: l, top: t, width: w, height: h, ...extra,
});
const lbl: React.CSSProperties = { fontSize: 24, fontWeight: 500, color: C.muted };
const val: React.CSSProperties = { fontSize: 56, fontWeight: 700 };
const H1 = [180, 260, 220, 340, 300, 420, 380];
const H2 = [240, 200, 300, 380, 360, 460, 440];

const Card: React.FC<{ y: number; label: string; value: string; style: React.CSSProperties }> = ({ y, label, value, style }) => (
  <div style={{ position: "absolute", inset: 0, ...style }}>
    <div style={box(250, y - 85, 440, 170, { borderRadius: 20, background: "#22223a" })} />
    <At x={280} y={y - 40} anchor="l" style={lbl}>{label}</At>
    <At x={280} y={y + 25} anchor="l" style={val}>{value}</At>
  </div>
);

export const Product: React.FC = () => {
  const f = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = f / fps;
  const win = prog(f, fps, 0, 0.8, outCubic);
  const count = (to: number, at: number) => Math.round(to * prog(f, fps, at, 2)).toLocaleString("en-US");
  const toastIn = prog(f, fps, 6, 0.7, outExpo);
  const toastOut = prog(f, fps, 10.5, 0.5, Easing.in(Easing.cubic));
  const cur = prog(f, fps, 13, 1.2, inOutCubic);
  const press = interpolate(t, [14.4, 14.55, 14.7], [1, 0.92, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const pulse = 0.65 + 0.35 * Math.cos(((t % 1.5) / 1.5) * 2 * Math.PI);
  return (
    <Stage>
      <div style={{ position: "absolute", inset: 0, opacity: win, translate: `0 ${(1 - win) * 60}px`, scale: String(0.96 + 0.04 * win) }}>
        <div style={box(160, 90, 1600, 900, { borderRadius: 24, background: C.surface, border: "2px solid #ffffff14" })} />
        {["#ff5f57", "#febc2e", "#28c840"].map((c, i) => <div key={c} style={box(192 + i * 28, 116, 16, 16, { borderRadius: 8, background: c })} />)}
        <div style={box(660, 108, 600, 32, { borderRadius: 16, background: "#ffffff10" })} />
        <div style={box(160, 157, 1600, 2, { background: "#ffffff14" })} />
      </div>
      <At x={250} y={220} anchor="l" style={{ fontSize: 44, fontWeight: 700, ...fadeUp(f, fps, 0.4) }}>Revenue</At>
      <div style={box(1243, 213, 14, 14, { borderRadius: 7, background: "#28c840", opacity: pulse })} />
      <At x={1270} y={220} anchor="l" style={{ fontSize: 24, fontWeight: 600, color: C.muted }}>Live</At>
      <div style={box(1490, 190, 220, 60, { borderRadius: 30, background: C.primary, display: "flex", alignItems: "center", justifyContent: "center", fontSize: 24, fontWeight: 600, scale: String(press) })}>Share</div>
      <Card y={360} label="Revenue ($)" value={count(12480, 1.2)} style={fadeUp(f, fps, 0.6)} />
      <Card y={570} label="Active users" value={count(3920, 1.35)} style={fadeUp(f, fps, 0.75)} />
      <Card y={780} label="Conversion" value="4.8%" style={fadeUp(f, fps, 0.9)} />
      <div style={{ position: "absolute", inset: 0, ...fadeUp(f, fps, 1.0) }}>
        <div style={box(750, 260, 960, 620, { borderRadius: 20, background: "#22223a" })} />
        <At x={790} y={310} anchor="l" style={{ fontSize: 28, fontWeight: 600 }}>Weekly signups</At>
      </div>
      {H1.map((h, i) => {
        const grow = h * prog(f, fps, 1.5 + i * 0.1, 0.6);
        const height = grow + (H2[i] - h) * prog(f, fps, 10 + i * 0.1, 0.8, inOutCubic);
        return <div key={i} style={{ position: "absolute", left: 800 + i * 130, bottom: 240, width: 80, height, borderRadius: 8, background: C.primary }} />;
      })}
      {t >= 6 && t < 11 && (
        <div style={{ position: "absolute", inset: 0, opacity: toastIn * (1 - toastOut), translate: `${(1 - toastIn) * 500}px 0` }}>
          <div style={box(1290, 887, 420, 90, { borderRadius: 16, background: "#2a2a44", border: "2px solid #ffffff20" })} />
          <div style={box(1330, 922, 20, 20, { borderRadius: 10, background: "#28c840" })} />
          <At x={1370} y={932} anchor="l" style={{ fontSize: 26, fontWeight: 600 }}>Export complete</At>
        </div>
      )}
      {t >= 12.5 && (
        <div style={box(1286.5 + 300 * cur, 686.5 - 475 * cur, 27, 27, { borderRadius: 14, background: "#fff", border: "3px solid #0b0b12", opacity: prog(f, fps, 12.5, 0.5) })} />
      )}
    </Stage>
  );
};
