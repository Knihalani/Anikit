import React from "react";
import { Composition } from "remotion";
import { Kinetic } from "./Kinetic";
import { Product } from "./Product";
import { Explainer } from "./Explainer";

const common = { durationInFrames: 600, fps: 30, width: 1920, height: 1080 };
export const Root: React.FC = () => (
  <>
    <Composition id="kinetic" component={Kinetic} {...common} />
    <Composition id="product" component={Product} {...common} />
    <Composition id="explainer" component={Explainer} {...common} />
    <Composition id="long" component={Kinetic} defaultProps={{ times: 3 }} {...common} fps={60} durationInFrames={3600} />
  </>
);
