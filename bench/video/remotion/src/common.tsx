import React from "react";
import { Easing, interpolate, staticFile } from "remotion";
import { loadFont } from "@remotion/fonts";

loadFont({ family: "Inter", url: staticFile("InterVariable.ttf"), weight: "100 900" });

export const C = { bg: "#0b0b12", text: "#f5f5fa", muted: "#9a9ab0", primary: "#7c5cff", accent: "#ff5ca8", surface: "#171725" };
export const outCubic = Easing.out(Easing.cubic);
export const outExpo = Easing.out(Easing.exp);
export const inOutCubic = Easing.inOut(Easing.cubic);
export const outBack = Easing.out(Easing.back(1.70158));

/** Eased 0..1 progress of a tween that starts at `at` seconds and lasts `dur`. */
export const prog = (frame: number, fps: number, at: number, dur: number, easing = outCubic) =>
  interpolate(frame / fps, [at, at + dur], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing });

/** fadeUp: opacity 0 → 1 and 40 px rise over 0.6 s, outCubic. */
export const fadeUp = (frame: number, fps: number, at: number): React.CSSProperties => {
  const p = prog(frame, fps, at, 0.6);
  return { opacity: p, translate: `0 ${(1 - p) * 40}px` };
};

export const Stage: React.FC<{ children: React.ReactNode; style?: React.CSSProperties }> = ({ children, style }) => (
  <div style={{ position: "absolute", inset: 0, background: C.bg, color: C.text, fontFamily: "Inter", fontOpticalSizing: "none", lineHeight: 1.2, ...style }}>
    {children}
  </div>
);

/** Absolutely positioned box; anchor "c" centres it on (x, y), "l" puts its left edge at x. */
export const At: React.FC<{ x: number; y: number; anchor?: "c" | "l"; style?: React.CSSProperties; children?: React.ReactNode }> = ({ x, y, anchor = "c", style, children }) => (
  <div style={{ position: "absolute", left: x, top: y, whiteSpace: "pre", transform: anchor === "c" ? "translate(-50%, -50%)" : "translateY(-50%)" }}>
    <div style={style}>{children}</div>
  </div>
);

export const display: React.CSSProperties = { fontVariationSettings: '"opsz" 32' };
