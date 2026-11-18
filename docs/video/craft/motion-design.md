# Motion design craft: what makes motion feel expensive

Research for Anikit Video. It goes deeper on **motion** than [`../taste/TASTE.md`](../taste/TASTE.md) §5–6 (read those first: Lasseter's in-between ladder, Material/Carbon registers, Apple spring bounce, Murch and Cutting on editing are covered there and not repeated here). Read it against [`../quality/BAR.md`](../quality/BAR.md): the user rejected work as "boring", "AI slop typography", "not impressive". Motion is where template work gives itself away first, because templates ship one curve, one duration and one stagger.

Conventions: frames are given at 24/30/60 fps. Bézier values are CSS `cubic-bezier(x1, y1, x2, y2)`, which Anikit takes as `"ease": [x1, y1, x2, y2]`. Numbers marked *(heuristic)* are our synthesis from the sources, to tune by eye; everything else carries a link (key list at the end).

---

## 1. Principles (short)

1. **Spacing carries weight; timing carries meaning.** The same two poses read as a hit, a nod or a stretch depending only on the in-betweens ([L87]). An ease is a spacing chart: where the copies bunch is where the object is slow.
2. **Asymmetry is the signature of intent.** Fast attack, long settle; exits shorter than entrances and accelerating away ([M1]; Carbon splits curves into *entrance*, *exit* and *standard* for this reason, [CARBON]). AE's Easy Ease sets 33.33% influence on both sides of every key ([AE-SPEED]): that symmetric, generic shape is why it reads as "default" ([VDCI]).
3. **Nothing stops all at once.** Parts lag the part that drives them and settle after it (follow-through), and the next action starts before the last one has finished (overlap) ([SOM-FT], [L87]).
4. **One thing leads.** Staging means one idea at a time ([L87]); everything else follows later or stays still. Animating everything erodes hierarchy ([PAXTON]).
5. **Stillness is a beat.** A pose has to *read*, so studios hold. But a dead hold looks broken, so character animators use a *moving hold*: the pose is kept while a tiny drift or overshoot keeps it alive ([AM-HOLD]).
6. **Motion is designed, not added.** Studio pieces plan the transitions with the boards. Buck worked out "the transitions between" scenes in storyboards and a boardomatic and chose the technique per shot: "some needed 3D, some pencil tests and some only needed a few keyframes" ([BUCK-MM]). Elastic's *Game of Thrones* camera is meant to feel like motion control, not a "magic camera" ([GOT]).
7. **Imperfection is a material.** Imaginary Forces matched *Stranger Things* to 1983 opticals by "looking for the mistakes": grain on the letters, imperfect edges, jitter in the light ([ST]). Ordinary Folk's Greg Stewart exported "at 12 frames a second" with "wiggles on a lot of little things" so it felt "handmade" ([GS12]).

## 2. Techniques with numbers

### 2.1 Spacing: the curves studios actually use

| Name | cubic-bezier | Feel / use | Source |
|---|---|---|---|
| expo out | `0.16, 1, 0.3, 1` | Very fast attack, long glide; hero arrivals, camera settles | [EASINGS] |
| quint out | `0.22, 1, 0.36, 1` | A little less snap than expo; type and UI arrivals | [EASINGS] |
| quart out | `0.25, 1, 0.5, 1` | Firm, versatile default arrival | [EASINGS] |
| cubic out | `0.33, 1, 0.68, 1` | Soft; the "safe" curve (Anikit's default `outCubic`) | [EASINGS] |
| expo in | `0.7, 0, 0.84, 0` | Exits that accelerate away, wind-up into a whip | [EASINGS] |
| expo in-out | `0.87, 0, 0.13, 1` | Snap transitions: almost still at both ends, violent middle | [EASINGS] |
| back out | `0.34, 1.56, 0.64, 1` | Overshoot; Penner's s = 1.70158 ([PENNER-JS]) works out to a **10%** overshoot (our derivation) | [EASINGS] |
| M3 standard / emphasized | `0.2, 0, 0, 1` | Material's current default | [M3] |
| M3 emphasized decelerate | `0.05, 0.7, 0.1, 1` | Entrances that should feel deliberate | [M3] |
| M3 emphasized accelerate | `0.3, 0, 0.8, 0.15` | Exits | [M3] |
| Carbon productive / expressive standard | `0.2, 0, 0.38, 0.9` / `0.4, 0.14, 0.3, 1` | Task UI vs "significant moments" | [CARBON] |
| Carbon expressive entrance / exit | `0, 0, 0.3, 1` / `0.4, 0.14, 1, 1` | | [CARBON] |

**AE influence mapping.** Material 1 published its curves as AE influences: standard = 40% out / 80% in (`0.4, 0, 0.2, 1`), deceleration = 0% / 80%, acceleration = 40% / 0%, sharp = 40% / 40% ([M1]). In CSS terms the influence is roughly the length of the handle: the incoming handle `1 − x2` ≈ incoming influence. So "70–90% ease-in on landings" means `x2` ≈ 0.1–0.3 with `y2 = 1`. AE's default is 33.33% ([AE-SPEED]). Pushing it towards 90–100% gives a long, slow arrival; check the result in the graph editor, because very high values look like the object never quite arrives *(heuristic)*.

**Overshoot through the value graph.** Practitioners build overshoot by pulling the incoming handle *past* the target value in the value graph, and set its reach with the handle length ([MTM-OS]). That shapes the settle with a single key instead of three.

### 2.2 Timing: durations by move class

| Move | ms | @24 | @30 | @60 | Source |
|---|---|---|---|---|---|
| Micro feedback (press, toggle) | 70–110 | 2–3 | 2–3 | 4–7 | [CARBON] |
| Small move, fade | 150–200 | 4–5 | 5–6 | 9–12 | [CARBON], [M1] desktop |
| Standard UI transition | 250–300 | 6–7 | 8–9 | 15–18 | [M1], [WIN] (250 ms "normal") |
| Element enters / exits | 225 / 195 | 5 / 5 | 7 / 6 | 14 / 12 | [M1] |
| Large / full-frame change | 375–500 | 9–12 | 11–15 | 23–30 | [M1], [M3] long tokens |
| Expressive hero move | 500–1000 | 12–24 | 15–30 | 30–60 | [M3] extra-long tokens; TASTE §5 |
| Background dim / mood change | 700 | 17 | 21 | 42 | [CARBON] |
| One-word kinetic card | — | 5–7 frames per word | | | [DONTBLINK] |
| Whip pan (whole transition) | ~300–400 | 7–10 | | | [WHIP-PF] 5 frames either side of the cut; [WHIP-SC] ~7 frames |

- **Duration scales with distance and screen size.** Bigger travel and bigger screens get longer moves, or the peak speed gets unnatural ([M1]). Carbon uses a non-linear scale for this ([CARBON]). Material scales tablet +30% and wearables −30% ([M1]). For a 1920 px frame (a "large screen"), a full-frame crossing needs the long end of these ranges *(heuristic)*.
- **Productive vs expressive is a ratio, not a style.** Carbon reserves expressive motion for occasional moments so that it works as "a rhythmic break" from productive motion ([CARBON]). For video *(heuristic)*: about 80% of moves in the 6–12 frame band, with one 18–30 frame hero move per scene.
- **Frame rate is a look.** *Spider-Verse* kept a 24 fps base but animated characters "on 2s" (12 poses/s) and turned motion blur off, drawing in-betweens instead, to push the 2D feel ([BLOOP-SV]). Greg Stewart's 12 fps export is the motion-design version ([GS12]). A cheap engine runs everything on 1s.

### 2.3 Overlap, follow-through, drag, offsets

| Relationship | Offset | Source |
|---|---|---|
| Parts of one object (arm → hand → stick) | 1–2 frames per link | [L87] (overlap "1 to 2 frames"); chain order from [SOM-FT] |
| Secondary layer behind a leader | 2–4 frames; tertiary 4–8 | [SKL-OFF] (practitioner rule; weak source) |
| Separate items in a set | 20–80 ms (1–2 frames @24) | TASTE §5 |
| Match-cut momentum | Cut on frame 6 of a 12-frame move; next shot resumes at frame 7 | [SOM-MC] |

- **Uniform offsets read as lag, not overlap.** Delaying every layer by the same amount "looks like a technical lag, not intentional overlap" ([SKL-OFF]). GSAP's staggers distribute delays with an `ease`, from `center`/`edges`/`random`, across a `grid`, and its `distribute()` varies the *amounts* per item as well as the delays ([GSAP-ST], [GSAP-DIST]).
- **Follow-through tells you about the stop and the medium.** A rider pitching over the handlebars shows how hard the bike stopped; in water, hair stops faster and drifts up ([SOM-FT]). In motion graphics the "hair" is a tag, a shadow, an underline or a secondary shape. Its lag says how heavy the leader is.
- **Automatic follow-through** in AE is an expression that reads the leader's velocity and adds a decaying oscillation after the last key ([SOM-AUTO]). Dan Ebberts' *inertial bounce* derives the amplitude from `velocityAtTime()` at the last keyframe, with `freq` and `decay` as the controls ([EBBERTS], [COW-BOUNCE]). The settle comes from the actual motion instead of a per-layer keyframe.

### 2.4 Arcs, squash and stretch, anticipation, secondary motion

- **Arcs.** Natural movement travels on arcs, not straight lines ([12P]). In motion graphics, a straight X+Y tween with the same ease on both axes is the tell. Separate the dimensions and give X and Y different eases (e.g. X `outQuart`, Y `outExpo`) so the path curves *(heuristic, standard AE "separate dimensions" practice)*.
- **Squash and stretch preserve volume.** Scale one axis and apply the inverse to the other ([SIG-SS]). Pixar parameterises the amount of preservation (0 = none, 1 = full) ([PIXAR-SQ]); Cavalry ships it as a behaviour with area preservation ([CAV-SQ]). The amount depends on the material: a rubber ball squashes a lot, a bowling ball barely ([CG-SS]). For graphic shapes *(heuristic)*: 5–15% stretch along the velocity at peak speed, 5–10% squash on impact for 2–3 frames, `scaleX·scaleY ≈ 1`. Use 0% on type that is being read.
- **Anticipation** is a small reverse move before a fast one, and lets a fast action read ([L87]). Practitioner figure: 2–4 frames of reverse motion ([SKL-OFF]; weak source). Size *(heuristic)*: 5–15% of the main travel, and longer for heavier objects. `easeInBack` `0.36, 0, 0.66, −0.56` bakes in about a 10% wind-up ([EASINGS]).
- **Secondary action** supports the main action without competing with it ([12P]). Example: a card lands (primary), its shadow tightens, and the label inside settles 2 frames later (secondary).
- **Moving holds.** Keep the pose but let something keep travelling a little, for example a 1–3% drift or the tail of an overshoot ([AM-HOLD]). This is different from the idle wobble TASTE flags as a tell: the drift continues the energy of the last move instead of cycling at random.

### 2.5 2D / 2.5D camera

- **Parallax.** Disney's multiplane camera moved up to seven glass planes past the lens at different speeds, the farthest slowest ([MULTI]). For 2.5D, foreground layers move and scale more than background layers, and "small and subtle movements work best" ([PB-25D], [VM-25D]).
- **Ratio** *(heuristic, from pinhole geometry)*: a layer at distance *d* moves ∝ 1/*d*. With three planes at 1:2:4, the background moves at ¼ of the foreground. Anikit's `1/(1+depth)` with depth 0/1/3 gives exactly this.
- **Push-in vs zoom.** A dolly push changes parallax (the foreground grows faster than the background); a zoom scales every plane the same, so it reads flat. A 2.5D push must move the camera *through* the depth planes *(heuristic, geometry)*.
- **Whip pan.** Blur builds up to a peak at the cut over about 5 frames either side ([WHIP-PF], [WHIP-MC]). Trim each shot to where its blur starts or ends ([WHIP-NFS]). Ease the pan in hard (expo in) into the cut and out hard (expo out) after it, so the two halves read as one continuous move.
- **Handheld / drift.** School of Motion's subtle camera shake is `wiggle(1, 25)` with its values on sliders so they can be keyframed ([SOM-SHAKE]); camera shakes in forums run to `wiggle(5,15)` ([COW-SHAKE]). Use low frequency (≤ 1 Hz) for "alive", and 5–8 Hz only for impacts, decaying within 0.3–0.5 s *(heuristic)*.
- **Rack focus** pulls the eye between planes; depth-of-field changes are "one of the most subtle and powerful" 2.5D tools ([PB-25D]). TASTE forbids defocusing text while it is read: rack *to* the type, never away from it mid-read.

### 2.6 Transitions as ideas

School of Motion's essentials are the hard cut, dissolve, cut on action and match cut; master these before decorative ones ([SOM-6T]).

| Idea | How it is built | Source |
|---|---|---|
| **Match cut** (shape, colour, position) | The subject keeps its screen position; cut between similar shapes; motion continues over the cut | [SOM-MC] |
| **Match move / carry-over** | One element's motion continues across the cut, but it becomes something else in the next scene | [CURSA-TR] |
| **Morph / shape transition** | Buck's Good Books: the book → flowers transition was morphed in 3D; the rest were pencil-tested 2D | [BUCK-MM] |
| **Matte wipe that follows motion** | A shape animates across the frame and serves as the matte that reveals the next scene; keep the wipe single-purpose and on-style | [CURSA-TR], [DEV-MASK] |
| **Zoom-through** | The camera pushes into a detail (a letter counter, a screen, an eye) that becomes the next frame | *(standard practice; no numeric source)* |
| **Liquid / ink / luma** | A noise-driven threshold of a luma or ink map | gl-transitions convention ([GLT]) |
| **Type → shape** | Type is the image and transforms into the subject (Bass titles carry "the story… in some metaphorical way", TASTE §1) | TASTE §1 |

A transition is an *idea* when it says that A *becomes* B or A *causes* B. It is decoration when it would work between any two scenes (slide, spin, generic glitch), and pro editors described those as "edited by a teen in iMovie" (TASTE §6).

### 2.7 Rigs and procedural motion

- **Oscillators.** `wiggle(freq, amp)` with values on sliders so the amplitude can be keyframed in and out ([SOM-SHAKE]). Ramp the amplitude, never leave it constant *(heuristic)*.
- **Overshoot rigs** read the incoming velocity ([EBBERTS]): any key, at any speed, gets a matching settle.
- **Systems, not one-offs.** Gunner animated Dropbox's product logos and defined "an animation system for them to follow", and the studio's mantra was "no bad frames" ([GUNNER-POD]). Tendril set brand *motion principles* mirroring its design principles ("precision & wonder", "balance & skew") ([TENDRIL]); a peer review praised its "smooth landings and high precision" ([TENDRIL-PR]).
- **Smear/echo rigs.** AE uses Echo for multiples, CC Smear for elongation, and plugins such as Cartoon Moblur (shutter, colour, samples) and Battle Axe's Schmeeer for automatic smears ([MDS-ECHO], [MDS-SMEAR], [AES-CMB], [SCHMEER]).

### 2.8 Rhythm with music

- **BPM math.** Frames per beat = fps × 60 / BPM ([BPM-CALC]). 120 BPM is 12 f @24 and 15 f @30; 150 BPM is 12 f @30; 128 BPM @24 = 11.25 f, so round per beat and never accumulate, or the edit drifts ([BPM-CALC]). Click tracks: 24 frames = 60 BPM ([PHAEDRUS]).
- **Tolerance.** A hit must land within about 25 ms (1–2 frames) to read as synced ([ABSIL]).
- **Early, on, or late.** No fixed rule: on, before or after the beat each gives a different feel ([ADOBE-BEAT]). One frame late lets you hear both the impact and the music; one frame early jolts ([EOSHD-BEAT]). Cut on the beat only once or twice; offset others by a few frames ([PB-MUSIC]).
- **Accents and holds.** Use the music's structure, not every kick: accents are on downbeats and fills, scene changes come on 4- or 8-bar phrases (TASTE §6), and a hold lands on a rest or a sustained note. Ordinary Folk's Jorge Canedo Estrada is known for "audio-driven motion design" ([FITC-JORGE]). *Speed ramps* put the slowest part of the ramp on the impact or the drop ([KAPWING-RAMP]).

### 2.9 Smears, streaks, motion blur

- **Smear frames** are in-betweens that fake motion blur, used since the 1940s (*The Dover Boys*, 1942). Types: elongated in-betweens and **multiples** along the path ([SMEAR-WP]). *Wolfwalkers* used pseudo-smears and multiples as motion blur ([WOLF]).
- **Motion blur practice.** 180° is the film look. Shutter phase = −½ angle centres the blur on the frame. AE's default of 5 samples shows echoes, and about 12 is a reasonable balance ([PVC-MB]).
- **Blur or smear?** Motion blur is a camera property and looks photographic. A smear is drawn and graphic, so it lets the animator put the energy where they want; *Spider-Verse* dropped blur for drawn in-betweens for exactly this control ([BLOOP-SV]). Pick one per piece (TASTE §5 "grammars").

## 3. Studio work vs template work

| Template tell | What studios do instead | Source |
|---|---|---|
| Linear moves, or Easy Ease 33/33 on every key | Asymmetric custom curves: hard attack, 70–90% landing influence | [AE-SPEED], [VDCI], [M1] |
| One curve and one duration everywhere | Durations scaled to distance; productive moves with a rare expressive one | [CARBON], [M1] |
| Everything moves at once | One leader; others overlap by 1–4 frames or hold | [L87], [PAXTON] |
| Uniform stagger (same delay, same amount) | Eased, centre- or edge-out, grid-aware distributions; varied amounts | [GSAP-ST], [GSAP-DIST], [SKL-OFF] |
| Symmetric in-out timing | Fast in, slow settle; exits shorter and accelerating | [M1], [CARBON] |
| No holds, or dead holds | Moving holds; held stills timed for reading | [AM-HOLD], TASTE §1 |
| Floaty springs (39%-overshoot "bouncy") | Bounce ≤ 0.3, settle from real velocity; "use caution above 0.4" | [WWDC23], [EBBERTS] |
| Scale-from-0 pop and fade-up on every element | Element-specific entrances: matte reveals, carry-overs, type built from shapes | [BUSHELL], TASTE §5 |
| Decorative slide/spin/glitch transitions | Match cuts, carry-overs and morphs planned in the boards | [SOM-MC], [BUCK-MM] |
| Camera zoom as a "push" | Parallax through depth planes; motion-control camera | [MULTI], [GOT] |
| Fast moves strobing on 1s with no blur | Blur at 180°, smears, or deliberately stepped 12 fps | [PVC-MB], [SMEAR-WP], [BLOOP-SV] |
| Cuts on every kick | Beat cuts reserved for accents; offset by frames; phrases for scenes | [PB-MUSIC], [ADOBE-BEAT] |
| Perfectly clean vector | Designed imperfection: grain, jitter, 12 fps wiggles | [ST], [GS12] |

## 4. How to express this in Anikit today

| Technique | Anikit now |
|---|---|
| Studio curves | `"ease": [0.16, 1, 0.3, 1]` (expo out); M3 `[0.05, 0.7, 0.1, 1]`. Set per tween, not once globally. |
| Asymmetric enter/exit | `enter: {preset, duration: 0.5, ease: [0.16,1,0.3,1]}`, `exit: {duration: 0.3, ease: [0.7,0,0.84,0]}` |
| Overlap of parts | Put the parts in a `group` with `"stagger": {"each": 0.042, "ease": "outQuad"}` (1 frame @24); give the parts *different* durations and eases too. |
| Non-uniform stagger | `stagger: {from: "center" | "random", ease, total}`. There is no grid, no distance-based order and no per-item amount. |
| Arcs | Separate `x`/`y` tweens with different eases, or a `path` with `progress` eased. |
| Overshoot | `keyframes` with a per-key ease, e.g. `{scale: [[0,0.9],[0.6,1.03,[0.16,1,0.3,1]],[1,1,"inOutSine"]]}`. Springs: prefer `stiff` (5% overshoot) or `snappy` (2.8%). `gentle` is 7.4%, `wobbly` 20.8%, `bouncy` 38.8% (our computation from `anikit-core/src/easing.rs` stiffness/damping, ζ = c / 2√k). |
| Anticipation | A `keyframes` dip: `{x: [[0,0],[0.2,-24,"outSine"],[1,600,[0.7,0,0.84,0]]]}`. |
| Squash/stretch | `scaleX`/`scaleY` keyframes kept at a product ≈ 1, by hand. |
| Follow-through | Not automatic: hand-key a later, smaller tween on the follower, or an expression like `"=… + 8*exp(-6*t)*sin(18*t)"` after the stop. |
| Echo / multiples | `echo: {count, delay, decay}` (time-based, not velocity-based). |
| Motion blur | `"motionBlur": {"shutter": 180}`; scene-wide. Keep text sharp by timing reads after moves end. |
| Parallax push | Camera `x`/`y` with layer `depth`; `zoom` scales every plane the same (zoom, not dolly). |
| Rack focus | `camera.aperture` + animate `focus`. |
| Shake/drift | `camera.shake {amount, frequency, at, duration}`; for slow drift, `"=noise(t*0.3)*20"` expressions. |
| Matte reveal | `matte` with an animated shape layer, inside a scene (not as a scene transition). |
| Idea transitions | `crossfade`/`wipe`/`iris`/`zoom`/`morphCut`, shader `burn`, `ink-bleed`, `luma-melt`, `warp-wipe`, `zoom-blur`. Match cuts by hand: put the subject at the same `x,y` in both scenes, `cut`, and continue the eased move. |
| Beat sync | `"@beat17"`, `"@bar5+0.25"` markers; `iBeat`/`iAudio` in shaders only. |
| 12 fps look | `"ease": "steps(n)"` per tween (no per-layer frame rate). |
| Speed ramp | Composition `speed` is constant; ramps need hand-keyed property curves. |

## 5. Gap list (ranked by impact on "feels like a studio made it")

1. **Hierarchy with lag and springs (follow-through for free).** `follow: {layer, lag: frames, spring}` or `parent` with per-channel lag and overshoot, so children and parts inherit the leader's motion late and settle themselves. Add `velocity`, `speed` and `valueAt(prop, t)` to expressions so AE-style inertial bounce ([EBBERTS]) and drag rigs can be written. Overlap is the single biggest gap between template and studio motion (§2.3), and today every follower is hand-keyed.
2. **Transitions as ideas, driven by layers.** (a) A **matte transition** where any layer or shape animation of scene B is the reveal mask, so mask reveals follow motion. (b) **Carry-over / shared element**: a layer `id` in both scenes is interpolated across the cut (position, scale, shape morph), giving match moves and type-to-shape. (c) **Zoom-through** into a layer's bounds or a glyph counter. (d) **Match-cut helper**: cut at the mid-frame of a move and resume it on the next frame ([SOM-MC]). Today's scene transitions are image operations that know nothing about the content.
3. **Velocity-aware smear and stretch.** Per-layer `stretch: {amount, preserve}` that scales along the velocity with volume preserved ([PIXAR-SQ], [CAV-SQ]); a `smear` mode (elongated in-between or multiples, [SMEAR-WP]) that turns on only above a speed threshold; per-layer `motionBlur`/shutter (blur shapes, never reading text). Fixes strobing and adds snap without the photographic blur look.
4. **Motion-grammar presets and better defaults.** A curated, named curve library (`expoOut [0.16,1,0.3,1]`, `m3Emphasized`, `carbonExpressive`, `landing(0.85)` that takes an AE-style influence); `anticipate` and `overshoot` as tween options in % of travel; `inertia` after any tween. Retune `popIn` (scale-from-0 + `outBack` 10%) and `bounceIn` (`bouncy`, 39% overshoot) toward 1–5% (TASTE §5), and add a per-piece `motion: heavy | elastic | editorial | fluid` that sets these together.
5. **Beat-driven layers, not just shaders.** Expression variables `beat`, `bar`, `beatPhase`, `env("bass")`; `"at": "@beat"` quantising with an `offset` in frames (early or late, [EOSHD-BEAT]); accent and phrase markers (`@phrase2`); speed-ramp curves aligned to a hit ([KAPWING-RAMP]). Today `iAudio`/`iBeat` reach shaders only.
6. **A true 2.5D camera.** A camera `z` (dolly) separate from `zoom`, so a push changes parallax ([MULTI]); presets for whip pans between scenes (directional blur that peaks at the cut, ~10 frames, [WHIP-PF]), crane and orbit moves; a handheld drift layer separate from impact shake ([SOM-SHAKE]).
7. **Time remapping and speed ramps.** Eased `speed` curves on compositions and scenes, plus freeze-and-ramp around a hit ([KAPWING-RAMP]).
8. **Richer stagger distributions.** `grid`, order by distance from a point or path, per-item jitter, and distributed *amounts* (scale, distance, duration) as well as delays ([GSAP-ST], [GSAP-DIST]).
9. **Procedural rig kit.** Oscillators with amplitude envelopes (fade in and out, never constant), path chains (trailing followers on a path, rope or tail), pendulum or jiggle secondary motion, and noise drift that continues the last move (a moving hold, [AM-HOLD]).
10. **Per-layer frame rate ("on 2s").** `fps: 12` or `posterize` on a layer or group while the camera and other layers stay on 1s ([BLOOP-SV], [GS12]). Designed imperfection: boiling lines and stroke jitter on 2s ([ST]).
11. **Critique checks for motion tells.** Flag symmetric curves, linear spatial moves, uniform staggers, more than N layers starting on the same frame, overshoot over 10% on non-cartoon pieces, holds with zero motion over 2 s, and cuts on every beat. These extend TASTE's ⚙ checks with the numbers above.

### Status (overnight build, MOGRAPH)

Built (`anikit-video docs mograph`, `docs/video/spec-reference.md` → Motion graphics):
- **1 (partly):** `inertia` (a spring that chases any layer's own motion; presets `soft`/`bouncy`/`tight`/`heavy`) and group `drag` (child i follows the group i × drag s later). `velocity`/`valueAt` in expressions are still open.
- **2 (partly):** `zoomThrough` (fly into a layer of the outgoing scene, the next scene shows through its shape), shape `reveal` (circle/diamond/square/star/blob + spin), `blinds`, `split`, `whip`, and track mattes (`matte`) inside a scene. Carry-over/shared elements and a match-cut helper are still open.
- **3:** `smear` (speed-proportional stretch along the travel, volume kept). Per-layer motion blur is still open.
- **4 (partly):** named curves `glide`, `snap`, `land`, `launch`, `material`, `emphasizedDecelerate/Accelerate`, `productive`, `expressive`, `expressiveEntrance/Exit`, and `"ease": [x1, y1, x2, y2]` with per-keyframe ease. Not yet retuned: `popIn`/`bounceIn` overshoot (a change to shared presets, flagged for the parent).
- **8 (partly):** repeater stagger `order` (`center`, `reverse`, `random` + seed) and the text `animator` range selector.
- **10:** `posterizeTime` on a layer or group (plus `wigglePath` for boiling lines).
- **11 (partly):** critique tips for linear spatial moves, four or more layers starting on one frame, and no follow-through anywhere.

Proof pieces in four registers: `examples/video/motion/` (velo-snap, rainwell-fluid, noodle-handmade, atlas-precision; full-size stills in `stills/`).

---

## Sources

[12P]: https://en.wikipedia.org/wiki/Twelve_basic_principles_of_animation
[ABSIL]: https://fransabsil.nl/htm/eventhit.htm
[ADOBE-BEAT]: https://community.adobe.com/t5/video-lounge-discussions/editing-with-music/m-p/12801378/highlight/true
[AE-SPEED]: https://helpx.adobe.com/after-effects/using/speed.html
[AES-CMB]: https://aescripts.com/learn/create-quick-custom-smears-or-motion-trails-in-after-effects
[AM-HOLD]: https://animationmentor.com/blog/why-all-animators-need-to-master-the-moving-hold
[BLOOP-SV]: https://www.bloopanimation.com/spiderman-into-the-spider-verse-2d-or-3d/
[BPM-CALC]: https://silvermansound.com/bpm-to-fps-calculator
[BUCK-MM]: https://motionographer.com/2012/03/26/interview-buck-good-books-metamorphosis/
[BUSHELL]: https://engineered.at/articles/death-to-scroll-fade-via-david-bushell
[CARBON]: https://carbondesignsystem.com/elements/motion/overview/
[CAV-SQ]: https://docs.cavalry.scenegroup.co/nodes/behaviours/squash-and-stretch
[CG-SS]: https://blog.cg-wire.com/squash-stretch-principle
[COW-BOUNCE]: https://creativecow.net/?p=1864073
[COW-SHAKE]: https://creativecow.net/?p=1844515
[CURSA-TR]: https://cursa.app/en/page/transitions-and-scene-building-cuts-slides-wipes-and-match-moves
[DEV-MASK]: https://dev.to/techluca_034/how-do-i-make-one-shape-reveal-another-using-a-mask-in-a-motion-graphic-1ig2
[DONTBLINK]: https://motionographer.com/2016/09/14/4-ways-dont-blink-is-a-masterclass-of-type-and-rhythm/
[EASINGS]: https://github.com/ai/easings.net/blob/master/src/easings.yml
[EBBERTS]: http://www.motionscript.com/articles/bounce-and-overshoot.html
[EOSHD-BEAT]: https://www.eoshd.com/comments/topic/46696-editing/
[FITC-JORGE]: https://fitc.ca/speaker/jorge-r-canedo-e
[GLT]: https://gl-transitions.com
[GOT]: https://www.artofthetitle.com/title/game-of-thrones/
[GS12]: https://www.schoolofmotion.com/tutorials/behind-the-keyframes-greg-stewart
[GSAP-DIST]: https://gsap.com/docs/v3/GSAP/UtilityMethods/distribute()
[GSAP-ST]: https://gsap.com/resources/getting-started/Staggers/
[GUNNER-POD]: https://www.everand.com/podcast/419102145/Episode-40-Gunner-Ian-Nick-are-the-founders-of-Gunner-a-Detroit-based-animation-and-illustration-studio
[KAPWING-RAMP]: https://www.kapwing.com/resources/how-to-use-the-speed-ramp-effect/
[L87]: https://www.ics.uci.edu/~fowlkes/class/ics162/exercises/lasseter.pdf
[M1]: https://m1.material.io/motion/duration-easing.html
[M3]: https://github.com/material-components/material-web/blob/main/tokens/versions/v0_192/_md-sys-motion.scss
[MDS-ECHO]: https://motiondesign.school/blog/smears-with-echo-effect/
[MDS-SMEAR]: https://motiondesign.school/blog/cc-smear-effect
[MTM-OS]: https://mtmograph.com/blogs/tools/the-bounce-and-overshoot-animation-trick-every-motion-designer-should-know
[MULTI]: https://en.wikipedia.org/wiki/Multiplane_camera
[PAXTON]: https://medium.com/@sophie_paxtonUX/9ece9aa9eb97
[PB-25D]: https://www.premiumbeat.com/blog/animate-2d-photo-in-after-effects
[PB-MUSIC]: https://www.premiumbeat.com/blog/how-to-edit-a-film-to-music-without-it-becoming-a-music-video
[PENNER-JS]: https://github.com/danro/jquery-easing/blob/master/jquery.easing.js
[PHAEDRUS]: https://phaedrus-audio.com/music_2_picture.htm
[PIXAR-SQ]: https://patents.google.com/patent/US7221379
[PVC-MB]: https://www.provideocoalition.com/cmg_hidden_gems_chapter_8_motion_blur_and_more/
[SCHMEER]: https://help.battleaxe.co/freebies/schmeeer
[SIG-SS]: https://education.siggraph.org/static/HyperGraph/animation/character_animation/principles/squash_and_stretch.htm
[SKL-OFF]: https://www.skills.lc/dylantarre/animation-principles/dylantarre-animation-principles-skills-01-by-domain-video-motion-graphics-skill-md
[SMEAR-WP]: https://en.wikipedia.org/wiki/Smear_frame
[SOM-6T]: https://www.schoolofmotion.com/blog/six-essential-motion-design-transitions-tutorial
[SOM-AUTO]: https://www.schoolofmotion.com/blog/automatic-follow-through-after-effects
[SOM-FT]: https://www.schoolofmotion.com/blog/follow-through-tutorial
[SOM-MC]: https://www.schoolofmotion.com/blog/match-cuts
[SOM-SHAKE]: https://www.schoolofmotion.com/blog/how-to-simulate-camera-shake-adobe-after-effects
[ST]: https://www.artofthetitle.com/title/stranger-things/
[TENDRIL]: https://www.stashmedia.tv/?p=93812
[TENDRIL-PR]: https://shots.net/news/view/peer-review-jackson-armstrong
[VDCI]: https://vdci.edu/learn/after-effects/mastering-motion-animations
[VM-25D]: https://www.videomaker.com/article/f2/13742-tutorial-25d-animation
[WHIP-MC]: https://miracamp.com/learn/premiere-pro/whip-transition
[WHIP-NFS]: https://nofilmschool.com/how-to-do-a-whip-pan
[WHIP-PF]: https://photofocus.com/software/tutorials-software/easy-whip-swish-pan-in-after-effects/
[WHIP-SC]: https://forum.shotcut.org/t/motion-blur-pan-transition-whip-pan-tutorial-part-one-now-completed/20760/6
[WIN]: https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing
[WOLF]: https://blog.frame.io/2021/05/03/made-in-frame-wolfwalkers/
[WWDC23]: https://developer.apple.com/videos/play/wwdc2023/10158/

Source-quality notes. The studio studies (Buck, Elastic, Imaginary Forces, Ordinary Folk, Gunner, Tendril) are qualitative; none of them publishes frame counts, so every frame figure above comes from design systems (Material, Carbon, Apple, Microsoft), Lasseter, or practitioner tutorials. [SKL-OFF] and [KAPWING-RAMP] are weak, aggregator-grade sources and are used only for rules of thumb. Some reference bodies were not visible in fetch (Ebberts, the School of Motion auto-follow-through video), so their claims rest on summaries and forum excerpts.
