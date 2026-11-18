import fs from "fs";
const m = JSON.parse(fs.readFileSync("build/map.json"));
const data = fs.readFileSync("data/n09.csv", "utf8").trim().split("\n").slice(1).map(l => parseFloat(l.split(",")[4]));
let t = fs.readFileSync("build/map.template.html", "utf8");
t = t.replace("__RINGS__", m.rings.map(d => `<path class="m-grid" d="${d}" />`).join("\n          "))
 .replace("__MERIDIANS__", m.meridians.map(d => `<path class="m-grid" d="${d}" />`).join("\n          "))
 .replace("__ARCTIC__", m.arcticCircle).replace("__LAND__", m.land).replace("__GREENLAND__", m.greenland)
 .replace("__DATA__", JSON.stringify(data));
fs.writeFileSync("film/compositions/map.html", t);
console.log("map.html", t.length, "years", data.length);
