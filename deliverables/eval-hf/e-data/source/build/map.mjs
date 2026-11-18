// Bakes the polar map (Natural Earth via world-atlas) into SVG path data, Lambert azimuthal equal-area on the North Pole.
import { geoAzimuthalEqualArea, geoPath, geoGraticule, geoCircle } from "d3-geo";
import { feature } from "topojson-client";
import fs from "fs";
const land = JSON.parse(fs.readFileSync("node_modules/world-atlas/land-50m.json"));
const countries = JSON.parse(fs.readFileSync("node_modules/world-atlas/countries-50m.json"));
const PX_PER_KM = 0.19, R_EARTH = 6371.0088;
const CX = 600, CY = 540; // map centre inside the 1920x1080 frame
const proj = geoAzimuthalEqualArea().rotate([0, -90, 0]).scale(PX_PER_KM * R_EARTH).translate([CX, CY]).clipAngle(30).precision(0.1);
// Rotate so Greenland/Canada sit top-left-ish and Siberia bottom-right: rotate around pole by lambda
proj.rotate([-30, -90, 0]);
const path = geoPath(proj).digits(1);
const landF = feature(land, land.objects.land);
const green = feature(countries, countries.objects.countries).features.find(f => f.properties.name === "Greenland");
const grat = geoGraticule().step([30, 10]).extentMajor([[-180, 50], [180, 90]]).extentMinor([[-180, 50], [180, 89.999]]);
const lines = [];
for (const lat of [80, 70, 60]) lines.push(path(geoCircle().center([0, 90]).radius(90 - lat).precision(1)()));
const merid = [];
for (let lon = -180; lon < 180; lon += 30) merid.push(path({ type: "LineString", coordinates: [[lon, 89.5], [lon, 50]] }));
const arctic = path(geoCircle().center([0, 90]).radius(90 - 66.5634).precision(0.5)());
const out = {
  PX_PER_KM, CX, CY,
  land: path(landF), greenland: path(green), rings: lines, meridians: merid, arcticCircle: arctic,
  greenlandAreaKm2: null,
};
fs.writeFileSync("build/map.json", JSON.stringify(out));
console.log(Object.fromEntries(Object.entries(out).map(([k,v])=>[k, typeof v==="string"? v.length : Array.isArray(v)? v.length : v])));
