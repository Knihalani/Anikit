import React from "react";
import { AbsoluteFill, Sequence, useCurrentFrame, useVideoConfig } from "remotion";
import { At, C, Stage, display, fadeUp, outBack, outCubic, outExpo, prog } from "./common";

const Chars: React.FC<{ text: string; kind: "fadeUp" | "popIn"; stagger: number }> = ({ text, kind, stagger }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <>
      {[...text].map((ch, i) => {
        const at = i * stagger;
        const style: React.CSSProperties =
          kind === "fadeUp" ? fadeUp(frame, fps, at) : { opacity: prog(frame, fps, at, 0.5, outCubic), scale: String(prog(frame, fps, at, 0.5, outBack)) };
        return <span key={i} style={{ display: "inline-block", whiteSpace: "pre", ...style }}>{ch}</span>;
      })}
    </>
  );
};

const Scene: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return <Stage style={{ scale: String(1 + 0.06 * Math.min(frame / fps / 5, 1)) }}>{children}</Stage>;
};

const big = (size: number, color = C.text): React.CSSProperties => ({ ...display, fontSize: size, fontWeight: 800, color });
const sub: React.CSSProperties = { fontSize: 48, fontWeight: 500, color: C.muted };

const S1 = () => {
  const f = useCurrentFrame(), { fps } = useVideoConfig();
  return (
    <Scene>
      <At x={960} y={500} style={big(160)}><Chars text="MAKE VIDEOS" kind="fadeUp" stagger={0.04} /></At>
      <At x={960} y={640} style={{ ...sub, ...fadeUp(f, fps, 0.8) }}>with code, not clicks</At>
    </Scene>
  );
};

const S2 = () => {
  const f = useCurrentFrame(), { fps } = useVideoConfig();
  return (
    <Scene>
      <At x={960} y={360} style={{ ...big(140), ...fadeUp(f, fps, 0) }}>FAST.</At>
      <At x={960} y={540} style={{ ...big(140), ...fadeUp(f, fps, 0.4) }}>NATIVE.</At>
      <At x={960} y={720} style={{ ...big(140, C.accent), ...fadeUp(f, fps, 0.8) }}>AGENT-FIRST.</At>
      <At x={960} y={830} style={{ width: 600, height: 8, borderRadius: 4, background: C.primary, scale: `${prog(f, fps, 1.2, 0.8, outExpo)} 1` }} />
    </Scene>
  );
};

const S3 = () => {
  const f = useCurrentFrame(), { fps } = useVideoConfig();
  const a = prog(f, fps, 0, 0.7, outExpo), b = prog(f, fps, 0.3, 0.7, outExpo);
  return (
    <Scene>
      <At x={960} y={540} style={{ width: 500, height: 500, border: "4px solid #ffffff22", borderRadius: 40, rotate: `${(f / fps / 5) * 90}deg` }} />
      <At x={960} y={440} style={{ ...big(180), opacity: a, translate: `${(a - 1) * 600}px 0` }}>JSON in.</At>
      <At x={960} y={640} style={{ ...big(180, C.primary), opacity: b, translate: `${(1 - b) * 600}px 0` }}>MP4 out.</At>
    </Scene>
  );
};

const S4 = () => {
  const f = useCurrentFrame(), { fps } = useVideoConfig();
  return (
    <Scene>
      <At x={960} y={500} style={big(180)}><Chars text="Anikit Video" kind="popIn" stagger={0.03} /></At>
      <At x={960} y={660} style={{ ...sub, ...fadeUp(f, fps, 0.8) }}>JSON in. MP4 out.</At>
      <At x={960} y={740} style={{ width: 240, height: 10, borderRadius: 5, background: C.accent, scale: `${prog(f, fps, 1.2, 0.8, outExpo)} 1` }} />
    </Scene>
  );
};

export const Kinetic: React.FC = () => (
  <AbsoluteFill>
    {[S1, S2, S3, S4].map((S, i) => (
      <Sequence key={i} from={i * 150} durationInFrames={150}><S /></Sequence>
    ))}
  </AbsoluteFill>
);
