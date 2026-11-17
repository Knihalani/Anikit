import React from "react";
import { AbsoluteFill, Sequence, useCurrentFrame, useVideoConfig } from "remotion";
import { At, C, Stage, display, fadeUp, prog, inOutCubic } from "./common";

const SLIDES: [string, string[]][] = [
  ["Why native rendering", ["Browsers lay out a DOM per frame", "Screenshots cost more than drawing", "Native code uses every core"]],
  ["Frames are pure functions", ["f(spec, t) gives a display list", "No shared state between frames", "Render frames in any order"]],
  ["The encoder knows what changed", ["Unchanged blocks are skipped", "Moved layers become motion vectors", "Static slides cost almost nothing"]],
  ["JSON in, MP4 out", ["One file describes the video", "Validate before you render", "Agents write it in one shot"]],
];

const Slide: React.FC<{ k: number }> = ({ k }) => {
  const f = useCurrentFrame();
  const { fps } = useVideoConfig();
  const [title, bullets] = SLIDES[k];
  return (
    <Stage style={{ opacity: k === 0 ? 1 : prog(f, fps, 0, 0.5, inOutCubic) }}>
      <At x={160} y={120} anchor="l" style={{ fontSize: 22, fontWeight: 600, color: C.muted }}>{`0${k + 1} / 04`}</At>
      <At x={160} y={260} anchor="l" style={{ ...display, fontSize: 72, fontWeight: 700, ...fadeUp(f, fps, 0) }}>{title}</At>
      {bullets.map((b, i) => (
        <div key={i} style={{ position: "absolute", inset: 0, ...fadeUp(f, fps, 0.5 + i * 0.4) }}>
          <div style={{ position: "absolute", left: 168, top: 413 + i * 100, width: 14, height: 14, borderRadius: 7, background: C.primary }} />
          <At x={210} y={420 + i * 100} anchor="l" style={{ fontSize: 36 }}>{b}</At>
        </div>
      ))}
      <div style={{ position: "absolute", left: 1160, top: 360, width: 560, height: 400, borderRadius: 24, background: C.surface }} />
      <div style={{ position: "absolute", left: 1400, top: 520 + 12 * Math.sin((f / fps) * 2), width: 80, height: 80, borderRadius: 40, background: C.accent }} />
      <div style={{ position: "absolute", left: 160, top: 998, width: 1600, height: 4, borderRadius: 2, background: "#ffffff14" }} />
    </Stage>
  );
};

export const Explainer: React.FC = () => (
  <AbsoluteFill>
    {SLIDES.map((_, k) => (
      <Sequence key={k} from={k * 150} durationInFrames={k === 3 ? 150 : 165}><Slide k={k} /></Sequence>
    ))}
  </AbsoluteFill>
);
