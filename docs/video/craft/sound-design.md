# Sound design for motion graphics, ads, product videos and title sequences

Research notes for Anikit Video (JSON spec → MP4, written by agents). The aim is to turn the craft into numbers an engine can apply by default and a linter can check. Every claim from a source cites it inline. Numbers marked **(rule of thumb)** are common practice or worked out from physics; no single source states them, so tune them by ear.

Contents: 1 Element taxonomy · 2 Layering · 3 Sync to picture · 4 Mixing short-form · 5 Premium vs cheap · 6 Free/CC0 sources and licences · 7 Procedural recipes · 8 Distilled principles, studio vs amateur, gap list against Anikit.

---

## 1. Element taxonomy

Sound for motion design has a handful of building blocks. Composer Jochen Mader (ZENTHING) says the term "sound design" covers everything from foley to synthetic textures to final delivery, and adds "we're not making art… it's still design and that always comes with a purpose" ([Stash, 2026](https://www.stashmedia.tv/guest-editorial-the-state-of-sound-in-moving-images-part-1/)). Each element below gives its job, its spectrum and its timing.

### 1.1 Whooshes, swishes, passbys

| Kind | Length | Spectrum | Shape | Use |
|---|---|---|---|---|
| **Swish/swoosh** (small object, UI card, text slide) | 120–400 ms | band 1–6 kHz, little below 300 Hz | fast attack (20–60 ms), peak at 50–70 % | object moves, quick cuts |
| **Whoosh** (camera move, scene transition) | 0.4–1.5 s | broad, 150 Hz–8 kHz, centre swept | slow swell, peak at 55–70 %, tail 30–40 % | scene transitions, camera moves |
| **Passby** (object crosses the frame) | 0.5–3 s | swept band + Doppler pitch drop | symmetric swell, pitch falls at the closest point | fly-throughs, logo zooms past camera |
| **Whip** | 60–200 ms | high, 2–10 kHz crack at the end | crescendo into a sharp end transient | snap transitions, whip pans |

- Whooshes are "broader and more atmospheric… for scene transitions and camera moves". Swooshes are "sharper and more defined… for object movements or quick cuts" ([Sonilo guide](https://sonilo.com/ai-music/whoosh-sound-effect-guide)).
- **Length follows the move.** "A whoosh has to match the move it covers, so length is everything" ([Sonilo](https://sonilo.com/ai-music/whoosh-sound-effect-guide)). Make the sound as long as the visible motion, from its start to its fastest point plus the tail. Scale it to the object too: "bigger means lower and slower" ([Krotos, 4 whoosh tips](https://krotos.studio/guide/4-whoosh-sound-effects-tips)).
- **Speed → pitch and brightness (rule of thumb).** Map the object's on-screen speed (px/s normalised to frame width) to the band-pass centre: slow is about 300–800 Hz, fast is 2–5 kHz. Map peak velocity to peak amplitude. Krotos recommends "extreme bands or simple filter sweeps" and layered volume automation for the rise and the tail ([Krotos](https://krotos.studio/guide/4-whoosh-sound-effects-tips)).
- **Doppler for passbys:** f' = f · c/(c ∓ v), with c = 343 m/s. Going from approach to recede scales the pitch by (c+v)/(c−v): about 1.19 (≈3 semitones) at 30 m/s and about 1.42 (≈6 semitones) at 60 m/s. A motion-graphics passby reads well with a fall of 2–5 semitones centred on the closest point, a level peak at the same moment, and a −6 to −12 dB high-shelf roll-off as the object recedes (air absorption) **(physics + rule of thumb)**.
- **Restraint:** "Not every cut is a moment… If the music already swells or resolves at the cut, an added whoosh can be redundant" ([Sonilo](https://sonilo.com/blog/guides/transition-effect-sound-video-edits)). Whooshes "should support motion quietly… Overuse reduces impact. Constant motion sound tires the ear" ([soundgrail](https://soundgrail.com/blog/shape-movement-and-flow-in-modern-sound-design)).

### 1.2 Risers and downers

- **Riser:** 1–8 s, usually 1, 2 or 4 bars of the music (2 s at 120 bpm = 1 bar). Pitch climbs 1–2 octaves, a low-pass or band-pass opens from about 200 Hz to 8–12 kHz, and LFO rate and depth rise together. Ableton's trailer guide says to "automate pitch and filter… increase LFO speed and intensity together" and to end with "a hit or a brief silence to build suspense" ([Ableton](https://www.ableton.com/en/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers/)).
- **The riser's peak is its hit point.** Align the end of the riser, not its start, to the impact. Either cut it dead 0–2 frames before the hit or let it peak exactly on the hit.
- **Reverse cymbal / reverse reverb:** 0.5–2 s, an exponential swell (most of the energy in the last 20 %), 4–12 kHz.
- **Downer/downlifter:** 0.5–3 s, falling 1–3 octaves after an impact. It marks a release or a scene end.
- **Shepard tone (endless riser):** sine or saw layers an octave apart. "The middle octave remains at constant volume while the top one fades out and the bottom one fades in" ([Kottke on Dunkirk](https://kottke.org/18/05/hans-zimmers-clever-use-of-the-shepard-scale-in-dunkirk); also [Titeux on Shepard/Risset illusions](https://www.nicolastiteux.com/en/blog/shepard-and-risset-audio-illusions/)). Practical parameters: 6–8 octave layers, a raised-cosine gain window over log-frequency centred near 500 Hz–1 kHz, and a glide rate of 1 octave per 4–10 s.
- **Sub-drop:** a sine falling from 60–120 Hz to 25–40 Hz over 0.5–2 s, placed under a hit.

### 1.3 Impacts and hits

Treat the anatomy as four layers, each with its own band:

| Layer | Band | Envelope | Notes |
|---|---|---|---|
| **Sub** | 30–60 Hz fundamental (sine/kick with pitch drop 80→40 Hz) | 0 ms attack, 300–1500 ms decay | keep it **dry** ([Violet Recording](https://violetrecording.com/how-to-make-impact-sounds/)); one layer only owns the sub ([Morphic](https://morphic.com/resources/how-to/how-to-layer-sound-effects)) |
| **Body** | 100–500 Hz (taiko, kick, tom, slammed object) | 1–5 ms attack, 150–600 ms decay | carries the "weight"; reverb here |
| **Transient / crack** | 2–8 kHz | 0.5–10 ms | "the ear responds to its speed rather than its volume" so it can sit lower than expected ([Morphic](https://morphic.com/resources/how-to/how-to-layer-sound-effects)) |
| **Tail / debris** | broadband, darkening | 1–4 s | "in most cases… a huge reverb that decreases in volume over time" ([Ableton](https://www.ableton.com/en/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers/)); metal or debris for realism |

- Nick Metcalfe's car explosion in Sound On Sound has five elements: bass sweetener, impact onset, sustain/release, metal tearing and debris tail. A distant version drops the close-up detail (metal, debris) ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)).
- **The "suck" (pre-impact silence):** "Leave a beat of silence just before the impact as this will accentuate the impact sound itself… Drop the mix for a moment before a loud sound" ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)). In practice, gap or duck the whole mix (music too) by 6–20 dB for 80–300 ms before the hit (2–9 frames at 30 fps) **(rule of thumb)**.
- **Braam:** low brass at C1–C2, with layers an octave apart (C0, C1, C2) and automated distortion, then reverb on the group ([Ableton](https://www.ableton.com/en/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers/)).
- **Tune hits to the score:** pitch the body and sub layers to the music's root or fifth (§5).

### 1.4 UI clicks, ticks, pops, dings

| Sound | Duration | Energy | Character |
|---|---|---|---|
| Tick | 2–10 ms | 3–8 kHz | filtered click, no body; counters, clock hands |
| Click | 5–30 ms | transient 2–6 kHz + small 600–1500 Hz body | buttons, toggles, cursor |
| Pop | 20–80 ms | sine 300–1200 Hz with a downward pitch envelope (×0.5 in 30 ms) + 1 ms noise | bubbles, items appearing, scale-in |
| Snap/shutter | 20–120 ms | 1–6 kHz, two transients 15–40 ms apart | screenshot, photo, toggle on |
| Ding/chime | 0.3–2 s | tonal 1–4 kHz, partials 1, 2.76, 5.4 (bar) or bell-like | success, notification |
| Typing | per key 10–40 ms, 6–12 keys/s with ±30 % jitter | 1–5 kHz clack + 200–400 Hz thock | text reveals |

(Rule of thumb, drawn from the existing Anikit generators and the Kenney packs. Kenney's [Interface Sounds](https://kenney.nl/assets/interface-sounds) has about 100 CC0 files and [UI Audio](https://www.kenney.nl/assets/ui-audio) about 50, both useful references.) Mix UI sounds 12–20 dB below hero impacts. They are punctuation, not events.

### 1.5 Foley materials: what defines each

Struck objects are sums of decaying sinusoids (modes). Two things define the material: **the ratios between the modal frequencies** and **the damping**. Van den Doel & Pai describe "higher damping values as characteristic of materials such as plastic and wood, while lower values are typical of metal", and higher modes decay faster ([van den Doel & Pai, modal synthesis paper](https://persianney.com/kvdoelcsubc/publications/modalpaper.pdf); [CCRMA modal slides](https://ccrma.stanford.edu/workshops/dsp2008/prc/modal.pdf)).

A useful relation **(physics)**: with loss factor η, a mode at f decays as e^(−π η f t), so **T60 ≈ 2.2 / (η · f)**.

| Material | Mode ratios | Loss factor η (approx.) | T60 at 1 kHz | Recognisable by |
|---|---|---|---|---|
| **Metal bar** (glockenspiel, rod) | 1, 2.756, 5.404, 8.933 (free-free bar) ([CCRMA](https://ccrma.stanford.edu/workshops/dsp2008/prc/modal.pdf), [Csound modal table](https://csound.com/manual/MiscModalFreq.html)) | 1e-4–1e-3 | 2–20 s (radiation-limited in practice: 1–6 s) | long ring, beating between close modes, sparse inharmonic partials |
| **Metal plate/sheet** | dense inharmonic (round plate: Bessel zeros) ([CCRMA](https://ccrma.stanford.edu/workshops/dsp2008/prc/modal.pdf)) | 1e-4–1e-3 | 1–5 s | shimmering wash, many modes |
| **Glass (wine glass/bottle)** | thin-shell law f_n ∝ n(n²−1)/√(n²+1) → ≈ 1 : 2.83 : 5.42 : 8.9 (n = 2…5) (worked out from that law) | ~1e-3 | 1–3 s | high fundamental 500 Hz–2 kHz (glass) or 2–5 kHz (small pieces), pure, bright, few modes |
| **Wood** | low, dense, quickly damped; wood plate 1, 1.47, 2.09, 2.56 ([Csound table](https://csound.com/manual/MiscModalFreq.html)) | 1e-2–3e-2 | 40–200 ms | "short thud" ([modal demo](https://richoncode.github.io/webartests/sound-synthesis/modal-synthesis)), energy 200 Hz–2 kHz, noisy attack |
| **Plastic** | like wood but higher, fewer modes | 2e-2–1e-1 | 10–60 ms | dull click, 1–4 kHz |
| **Membrane** (drum, cardboard box) | 1, 1.59, 2.14, 2.30, 2.65, 2.92 (ideal circular membrane) | 1e-2 (+air damping) | 100–500 ms | pitched thump with fast pitch drop |
| **Paper** | no stable modes: crackle = sparse impulses 1–8 kHz | n/a | n/a | crumple = Poisson impulses 50–400/s; page turn = 150–400 ms band noise 1–6 kHz with a swish |
| **Cloth** | no modes: soft band noise 500 Hz–5 kHz, tiny transients | n/a | n/a | rustle level follows velocity, quiet (−30 dB under hero) |

Breaking glass layers the strike transient (1–3 ms, broadband), several short glass partials (3–8 kHz, random pitches) and a debris tail of 20–80 small tinkles over 0.3–1.5 s.

### 1.6 Magic and sparkle, glitch, swells

- **Sparkle/shimmer:** Poisson-triggered sine "grains" at 20–60/s, with pitches drawn from a pentatonic set of the music's key in octaves 6–8 (1–8 kHz). Each grain has a 2 ms attack and 30–200 ms decay and gets a random pan, all through a bright reverb (2–4 s, pre-delay 20–40 ms) **(rule of thumb)**.
- **Glitch:** stutter retriggers of 1/32–1/16 slices (20–60 ms), bit-crush (4–8 bits), sample-rate reduction (2–8 kHz), and 1–5 ms digital clicks. It tends to read as intended when it is locked to the edit grid (frame or beat); free-running glitch tends to sound like a fault.
- **Swell:** a reversed tonal pad or cymbal, 1–3 s, peaking on the reveal. It is a gentler riser.

### 1.7 Ambience beds

Beds make space continuous across cuts. Their levels sit well under the foreground. In a study of documentary mixing, 12 engineers chose a **dialogue-to-background difference of 9 ± 3 LU** ([Salford repository](https://salford-repository.worktribe.com/OutputFile/1488623)).

| Bed | Spectrum | Motion | Level (rule of thumb) |
|---|---|---|---|
| **Room tone** | pink-ish noise, low-passed 2–6 kHz, slight 50–200 Hz hum optional | static | −50 to −60 dBFS RMS: felt, not heard. Its job is to stop the mix from "dropping out" |
| **City** | low rumble 30–200 Hz (traffic) + mid wash; occasional distant horns and voices | slow, 5–30 s | 15–25 dB under VO |
| **Nature/forest** | quiet broadband with sparse birds 2–8 kHz | random events 0.1–1/s | 15–25 dB under VO |
| **Wind** | low-passed noise, cutoff wandering 200–800 Hz; whistles from narrow bands | gusts every 2–10 s | 12–20 dB under VO |
| **Ocean** | noise 100 Hz–3 kHz | swell period 6–12 s | 12–20 dB under VO |
| **Rain** | dense high-frequency crackle 1–8 kHz + low wash | stationary | 12–20 dB under VO |

---

## 2. Layering

- **Role per layer:** sub, body, detail and tail. "Simple cues may need one or two, while a hero impact often uses three or four" ([Morphic](https://morphic.com/resources/how-to/how-to-layer-sound-effects)). SOS builds a hero explosion from five ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)), and Ableton's trailer hit from five ([Ableton](https://www.ableton.com/en/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers/)).
- **Frequency slotting:** one layer owns everything below 80–100 Hz and every other layer is high-passed. "EQ every layer individually to avoid clashes when stacking low-frequency sounds" ([Ableton](https://www.ableton.com/en/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers/)). Typical slots: sub < 80 Hz, body 80–500 Hz, mid/character 500 Hz–2 kHz, transient/air 2–10 kHz.
- **Time slotting:** the transient leads (0 ms), the body comes 0–5 ms later, the tail swells over 50–200 ms. Offset the layers by a few ms so the attack stays sharp and doesn't flam. More than about 10 ms between attacks starts to read as two events **(rule of thumb)**.
- **Subtract:** "We can better focus on the important layers by removing less essential ones" ([Krotos interview, via search](https://www.krotosaudio.com/?p=1068840)). Mute each layer in turn and keep only the ones you miss.
- **Mono sources, stereo placement:** Ryan Hobler (Gramercy Park Studios) "used mostly mono sounds so he could move many elements around the stereo field" ([Krotos/Hobler](https://krotos.studio/blog/sound-effects-motion-graphics-tips)).
- **Variation and round-robin against the "machine gun":** repeated identical samples (typing, counters, staggered list reveals, particles) give themselves away. Per instance, randomise the pitch by ±0.5–2 semitones (±3–12 %), the gain by ±1–3 dB, the start offset by 0–10 ms, and the filter cutoff by ±10–20 %. Rotating 3–8 variants so the same one rarely plays twice in a row keeps it from sounding mechanical. SOS alternates two tracks for footsteps so each one tails out ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)). Game audio varies footsteps "by surface… to avoid repetition" (same).
- **Glue:** a shared short reverb or room on all layers of one event, then light bus compression (2:1–4:1, 10–30 ms attack so the transient passes, 100–200 ms release) and limiting at the end ([Ableton](https://www.ableton.com/en/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers/): "compress individual elements, then limit the overall mix").

---

## 3. Sync to picture

### 3.1 Tolerances

| Source | Audio early (leads) | Audio late (lags) |
|---|---|---|
| ITU-R BT.1359-1 detectability | 45 ms | 125 ms |
| ITU-R BT.1359-1 acceptability | 90 ms | 185 ms |
| EBU R37, end-to-end | 40 ms | 60 ms |
| ATSC IS-191 | 15 ms | 45 ms |
| Dixon & Spitz 1980, sharp transient (hammer) | 75 ms | 188 ms |

Sources: [ITU-R BT.1359-1](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.1359-1-199811-I!!PDF-E.pdf), with the summary table in [Fora Soft](https://www.forasoft.com/learn/audio-for-video/articles-audio/lip-sync-itu-r-bt-1359-tolerance-windows) and background in [TV Technology](https://www.tvtechnology.com/opinions/av-synchronization-how-bad-is-bad). Late sound is tolerated about 2.7× more than early sound, because in the real world sound travels slower than light. One frame is 41.7 ms at 24 fps, 40 ms at 25 fps and 33.3 ms at 30 fps.

**What this means for an engine:**
- **A transient that lands more than about 1 frame (33–42 ms) before its visual event tends to read as wrong**, because that is already near the 45 ms detection limit. Prefer 0 to +1 frame late. Sample-accurate placement on the contact frame is ideal. Sound On Sound: audio "two or three frames adrift of the visual edit will look and sound sloppy" ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)).
- Editors disagree on placing hits a frame early "to feel responsive" ([search summary of editing guides](https://sfxengine.com/blog/sound-design-tips-for-video-editors)). The standards above argue against going more than 1 frame early. Default to 0 frames.
- Include the encoder's audio priming (AAC adds about 1024–2112 samples of delay). Measure the end-to-end offset in the MP4 and correct it, or every hit ships 20–45 ms late on top of any intentional offset **(engineering note)**.

### 3.2 Where the peak goes

| Element | Align this point to the visual event |
|---|---|
| Impact/hit | first transient sample → contact frame (the frame where the object first touches or stops) |
| Whoosh/swish | **loudest point → the cut / fastest frame / closest pass.** Pre-roll = peak position × length (≈ 55–70 % of the sound, so a 1.2 s whoosh starts ≈ 0.75 s early) |
| Riser | end/peak → hit; optional 80–300 ms gap ("suck") before the hit |
| Reverse cymbal/swell | peak → reveal frame |
| Pop/scale-in | transient → first frame the element is visible at scale > 0 |
| Typing | each keystroke → the frame its glyph appears |
| Logo sting | transient or first note → first frame of the logo resolve |

### 3.3 When not to sound something (density)

- "Decide which visual elements actually need a sound, and which don't… Often less is more and the sound design does not always have to be super-detailed" ([It's Nice That, David Kamp](https://www.itsnicethat.com/articles/david-kamp-interview/)).
- "When every transition has a sweep, impact, reverse sound, fill, and burst of noise, nothing feels important anymore" ([electronicgroove](https://electronicgroove.com/how-electronic-music-producers-use-sound-design-to-build-impact/)).
- **Density budget (rule of thumb):**
  - 1 hero hit per 4–8 s of a 15–60 s spot, plus 1 hero moment per spot (the logo or product reveal);
  - no more than 2–3 distinct SFX onsets per second, even in fast montage;
  - staggered reveals of N items: sound the group, or sound 1 of every 2–4 items (sounding all N tends to become a machine gun);
  - leave 30–50 % of animated elements unsounded;
  - if the music has an accent on the cut, either drop the whoosh or tune it to the music.
- **Hard cuts:** "change most of your content at the same point. Avoid overlaps, which weaken the transition" ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)).
- **Silence as contrast:** use near-silence after long loud stretches so the ear can rest. "Don't overuse these techniques, or their impact weakens" (same).

---

## 4. Mixing short-form

### 4.1 Loudness targets (all ITU-R BS.1770 K-weighted, gated)

| Destination | Integrated | True peak | Source |
|---|---|---|---|
| Spotify (normal / loud / quiet) | −14 / −11 / −19 LUFS | ≤ −1 dBTP; ≤ −2 dBTP if master > −14 LUFS | [Spotify for Artists](https://support.spotify.com/us/artists/article/loudness-normalization/) |
| YouTube | ≈ −14 LUFS (turns loud audio down, does not boost quiet audio) | −1 dBTP | not officially published; [Sage Audio](https://www.sageaudio.com/articles/mastering-for-streaming-platform-loudness-and-normalization-explained), [Fora Soft](https://www.forasoft.com/learn/audio-for-video/articles-audio/lufs-targets-per-platform-2026) |
| Apple (Podcasts spec; Music Sound Check ≈ −16) | −16 LKFS ±1 dB | ≤ −1 dBFS TP | [Apple Podcasts audio requirements](https://podcasters.apple.com/support/audio-requirements) |
| TikTok / Instagram Reels | no published target; practice is −14 to −10 LUFS | −1 dBTP | [Fora Soft](https://www.forasoft.com/learn/audio-for-video/articles-audio/lufs-targets-per-platform-2026), sources disagree |
| EBU R128 broadcast | −23 LUFS ±1 LU (±0.5 for ads in many countries) | −1 dBTP | [EBU R128 s1](https://tech.ebu.ch/docs/r/r128s1.pdf), [Auphonic](https://auphonic.com/blog/2012/08/02/loudness-measurement-and-normalization-ebu-r128-calm-act/) |
| ATSC A/85 (US TV, CALM Act) | −24 LKFS ±2 | −2 dBTP | [TC Electronic](https://www.tcelectronic.com/broadcast-standards), [Fora Soft](https://www.forasoft.com/learn/audio-for-video/articles-audio/loudness-normalization-ebu-r128-bs1770-atsc-a85) |
| Netflix | −27 LKFS ±2 LU **dialogue-gated** | −2 dBTP | [Netflix Sound Mix Specs](https://backlothelp.netflix.com/hc/en-us/articles/360001794307-Netflix-Sound-Mix-Specifications-Best-Practices-v1-5) |

**Default for Anikit:** −14 LUFS integrated with a −1 dBTP ceiling (−2 dBTP if a lossy encode at ≤ 128 kbps follows, since AAC/MP3 overshoot). Offer the presets `web` (−14), `apple` (−16), `broadcast-eu` (−23/−1), `broadcast-us` (−24/−2) and `social-loud` (−11/−1). Apple notes that codecs "generally don't change loudness" but can clip if you ignore the true-peak ceiling ([Apple](https://podcasters.apple.com/support/audio-requirements)).

### 4.2 Dynamics and LRA for short-form
- EBU R128 sets no mandatory loudness range ([forasoft R128 summary](https://www.forasoft.com/learn/audio-for-video/articles-audio/loudness-normalization-ebu-r128-bs1770-atsc-a85)). For 6–60 s phone-first content, aim for **LRA 4–8 LU** **(rule of thumb)**. Under 4 sounds flat and crushed. Over about 10 makes quiet parts vanish on phone speakers and the hits too loud after normalisation.
- Peak-to-loudness ratio: keep the hero hits' short-term peaks (3 s window) about 4–8 LU above the integrated level, with true peaks held by the limiter. Limiters with 1–5 ms look-ahead and 50–200 ms release, and no more than 3–4 dB gain reduction on hits.

### 4.3 Ducking
- **Music under voice-over:** −6 to −12 dB is "a gentle dip, with music still clearly audible under speech", −20 is typical for strong separation, and −30 makes the music nearly vanish. Attack 10–30 ms (1–10 ms ducks "instantly"), release 200–500 ms ([Vindral sidechain docs](https://composer-docs.vindral.com/operators/operators-sidechain-ducking.html), [ffmpeg ducking cookbook](https://ffmpeg-cookbook.pages.dev/en/articles/audio-ducking/), [Corel](https://help.corel.com/videostudio/v24/en/official-help/videostudio/h2-audio-ducking.html)). Look-ahead ducking, which starts the fade 50–150 ms before the first word, avoids the clipped first syllable.
- **Music under hero SFX (rule of thumb):** a short dip of 2–6 dB, attack 2–10 ms, hold the length of the transient (30–100 ms), release 150–400 ms. Use a frequency-selective duck where you can: dip the music's 40–120 Hz by 4–8 dB under sub hits instead of the whole band.
- **Balance targets (rule of thumb, short-form ad):** VO is the loudest element. Music bed sits 8–14 LU under VO while VO speaks and comes up 4–6 dB in the gaps. UI SFX sit 10–18 dB under VO, hero hits at or near VO level. Beds sit 9 ± 3 LU under dialogue ([Salford study](https://salford-repository.worktribe.com/OutputFile/1488623)).

### 4.4 Panning and stereo
- **Pan follows the picture**, with a narrower stage than the frame: map the screen's x from −1…1 to a pan of about ±0.5–0.7. Full hard pans sound detached on headphones and collapse oddly on phones. Ryan Hobler pans mono elements to follow the on-screen perspective ([Krotos/Hobler](https://krotos.studio/blog/sound-effects-motion-graphics-tips)).
- **Pan law:** equal-power (sin/cos, −3 dB at centre) for moving sources. Use −4.5 dB at centre if the mono fold-down matters more.
- **Movement:** whooshes and passbys travel across the stereo field along with the motion (left→right as the element moves left→right). Keep the sub and body centred and let only the high layers move.
- **Width:** keep everything below about 150 Hz mono. Widen only the tails and ambiences. Check mono compatibility: the L+R sum should lose ≤ 3 dB, with no comb-filter hollowness (avoid Haas delays of 1–30 ms on centred elements). Many phones play a single speaker or a mono sum.

### 4.5 Space and reverb matching the image

| Image space | RT60 | Pre-delay (rule of thumb) | Notes |
|---|---|---|---|
| Dry UI / abstract flat graphics | 0.2–0.5 s or none | 0–10 ms | tiny "room" glue only |
| Small room / product on a table | 0.3–0.6 s | 5–15 ms | studio 0.3 s; "dead" < 0.3 s ([NTi Audio / Illinois slides](https://timan.cs.illinois.edu/pdfjs_csintro//static/slides/PHYS406/P406POM_Lect10_Part2/PHYS406---P406POM_Lect10_Part2---slide0.pdf)) |
| Large interior / showroom | 0.8–1.5 s | 15–30 ms | |
| Hall / epic 3D space | 1.4–2.0 s | 20–40 ms | concert halls 1.7–2.05 s ([HyperPhysics](https://hyperphysics.gsu.edu/hbase/Acoustic/revtim.html)) |
| Cathedral / "infinite" title space | 3–8 s | 30–80 ms | St Paul's ≈ 11 s empty at 500 Hz (same) |

- **Rule:** one space per scene. All SFX in a shot share one reverb (as a send), and the dry/wet ratio follows camera distance. Closer sounds are drier and brighter; far ones are wetter, darker (−3 to −9 dB high-shelf above 4 kHz) and quieter. SOS uses delays to simulate reflections off buildings and reverb "in small doses" ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)).
- **Pre-delay ≈ distance to the nearest wall:** 1 ms ≈ 34 cm of extra path. Longer pre-delay keeps the transient clear of the tail.
- **Keep the sub out of the reverb** (high-pass the send at 150–250 Hz).

### 4.6 EQ carving and phone translation
- **Carve:** dip music at 1–4 kHz by 2–4 dB under VO (presence). Dip music at the hit's body frequency by 2–4 dB under impacts. High-pass every non-sub layer at 60–120 Hz.
- **Phone speakers:** most roll off below about 200 Hz and "almost all of them completely roll off by 100 Hz" ([LANDR](https://blog.landr.com/make-bass-audible-phone-speakers)). Add harmonics to sub hits so the brain rebuilds the missing fundamental, a technique that "decodes the missing fundamental frequency" ([AES e-lib 19278](https://www.aes.org/e-lib/browse.cfm?elib=19278)). **Recipe:** duplicate the sub layer, saturate it (tanh drive 6–12 dB), band-pass 100–400 Hz, and blend it 6–12 dB under the sub. High-pass the master around 25–30 Hz so no headroom goes on content nobody can hear.
- **Translation check:** solo a 200 Hz high-pass plus 8 kHz low-pass ("phone sim") on the master. Every hero event must still read.

---

## 5. Premium vs cheap

**What makes sound feel premium**
1. **Restraint and hierarchy:** few sounds, each one meaning something ([Kamp](https://www.itsnicethat.com/articles/david-kamp-interview/), [TCD dissertation on "Less is More" in animation sound](https://publications.scss.tcd.ie/theses/diss/2021/TCD-SCSS-DISSERTATION-2021-015.pdf)).
2. **Tuned to the score:** tonal SFX (dings, sparkles, braams, sub hits, risers' end pitch) are pitched to the key, usually root, fifth or pentatonic set, and rhythmic SFX fall on the music's grid. Hobler "pitched swirly synths to match the spins" and built "a warm, musical bed" under the transformation ([Krotos/Hobler](https://krotos.studio/blog/sound-effects-motion-graphics-tips)).
3. **One palette, i.e. sonic branding:** studios like [Antfood](https://antfood.com/about/) (Netflix Animation, IBM, Crunchyroll with BUCK) treat a brand's sound as a system: the same materials, synth timbres and reverb across every cue. Audio logos run **2–3 s**, textural ones 1–2 s ([Listen](https://wearelisten.com/what-is-sonic-branding-pt-2)). Netflix's "ta-dum" is about 2 s and Intel's mnemonic about 3 s, five notes ([Creative Bloq](https://creativebloq.com/features/audio-logos)).
4. **Silence before the hit** and dynamic contrast ([SOS](https://www.soundonsound.com/techniques/sound-design-visual-media-radio)).
5. **Matched space:** one reverb per scene, dry/wet by distance.
6. **Bespoke sources:** Ben Burtt's Star Wars sounds came from "unassuming everyday objects… recontextualized" (projector motor hum + TV picture-tube buzz for the lightsaber; elephant + tyres on wet road for the TIE fighter; varispeed pitching) ([No Film School](https://nofilmschool.com/star-wars-sound-design), [filmsound.org](https://www.filmsound.org/starwars/)). Antfood "extracts musicality from objects" with hand percussion and foley ([LBB](https://lbbonline.com/news/camera-obscura-extracting-musicality-and-meaning-from-objects-with-bennett-eiferman)). A procedural engine gets the same effect from seeded synthesis plus per-brand parameters.
7. **Micro-variation:** no two instances identical.
8. **Clean low end and translation:** one sub owner, harmonics for phones, headroom for true peak.

**What makes it feel cheap/stock**
- Every move whooshed, every cut hit, everything at the same loudness.
- Recognisable library clichés (the same "cinematic boom", the "Inception" braam on a SaaS explainer).
- SFX out of key with the music: dissonant dings, risers ending a semitone off.
- Mismatched spaces: a dry click next to a cathedral-tailed whoosh.
- Machine-gun repetition (identical typing clicks, identical pops on 20 staggered icons).
- Clipping or over-limiting (−8 LUFS, crushed transients). Platforms turn it down, so it ends up quieter **and** duller.
- Too much sub (inaudible on phones, eats headroom, pumps the limiter).
- Late or early hits by 2+ frames.
- SFX fighting the VO: unducked music, or sibilant SFX at 4–8 kHz under speech.

---

## 6. Free / CC0 sample sources and licence status

| Source | Licence | Commercial | Attribution | Can an open-source renderer **bundle** files? | Fetch on demand? |
|---|---|---|---|---|---|
| **Freesound** (CC0 subset) | per-sound: CC0, CC-BY, CC-BY-NC (search filter `license:"Creative Commons 0"`) ([API resources](https://freesound.org/docs/api/resources_apiv2.html)) | CC0/BY yes; NC no | CC0 none; BY required | **CC0 sounds yes** (public domain dedication); BY yes with credits; NC no | **Needs an API key** (`token=` param) even for search and previews. Original files need **OAuth2** ([API overview](https://www.freesound.org/docs/api/overview.html)). Previews (mp3 ~128 kbps/ogg ~192 kbps) need only the key. Limits: 60 req/min, 2000/day (originals 30/min, 500/day). API ToS: keys "are our property and may be revoked if you share them", commercial API use "negotiated on a case by case basis", no full database copies ([API ToS](https://freesound.org/help/tos_api/)). So the project **cannot ship a key**; the user supplies their own (env var), and the renderer caches only what a render needs |
| **Kenney.nl** audio (Interface Sounds ~100 files, UI Audio ~50, others) | **CC0** ([Interface Sounds](https://kenney.nl/assets/interface-sounds), [UI Audio](https://www.kenney.nl/assets/ui-audio)) | yes | none | **yes**, safest choice | static zips; mirror once and vendor |
| **OpenGameArt** | per-asset (CC0, CC-BY, CC-BY-SA, GPL, OGA-BY) ([example CC0 page](https://opengameart.org/content/interface-sounds)) | depends | depends | CC0 items yes; check each page | manual; no stable API |
| **Sonniss GDC Game Audio Bundle** | royalty-free EULA, **not CC0** | yes, "completely royalty-free for any commercial work" | not required | **no**: redistribution "not as sound effects… original files and any edited or re-designed versions, on their own or inside a sound library"; allowed only inside a finished game/film/app. **AI/ML training strictly prohibited** ([Sonniss GDC page](https://sonniss.com/gameaudiogdc)) | no API; manual download. A user may use it in *their* renders, but the repo must not include the files |
| **BBC Sound Effects** (RemArc) | personal, educational, research only | **no** (commercial licensing via Pro Sound Effects) | BBC credit required | **no** ([TechRadar](https://techradar.com/news/the-bbc-is-letting-you-download-more-than-16000-free-sound-effect-samples), [Swarmify summary](https://swarmify.com/blog/free-sound-effects-websites/)) | manual; not for an ad renderer |
| **Pixabay** sounds | Pixabay Content License (not CC0) | yes | optional | **no standalone redistribution**: you may not "sell or distribute Content… on a Standalone basis… audio" ([Pixabay licence summary](https://pixabay.com/pt/service/license), [ToS](https://pixabay.com/service/terms/)). Using them inside a rendered video is fine; a sample pack in the repo is not | API exists (key); fine for user-side fetch into a render, not for vendoring |
| **Mixkit** | Mixkit Sound Effects Free License | yes ([terms](https://mixkit.co/terms/); secondary: [review](https://kripeshadwani.com/mixkit-review/)) | not required | treat as **no** (stock-site licence, redistribution as a library not granted) | no public API |

**Bottom line for Anikit:** only **CC0** (Kenney, CC0-filtered Freesound and OpenGameArt items) can be **vendored** into the repo or the npm/PyPI packages. Everything else can only be fetched **by the user, with their key, into their own render**. Because of that, procedural synthesis (§7) should stay the default path: it carries no licence risk, it is deterministic, and it can be tuned to key and tempo.

---

## 7. Procedural synthesis recipes

These follow Andy Farnell's *Designing Sound* (MIT Press, 2010), which treats sounds as processes rather than samples ([Farnell Practical 15: Rain code](https://mitp-content-server.mit.edu/books/content/sectbyfn/books_pres_0/8375/designing_sound.zip/practical15.html); [Designing Sound: procedural audio now](https://designingsound.org/2010/09/24/audio-implementation-greats-8-procedural-audio-now/); [QMUL procedural audio review](https://www.eecs.qmul.ac.uk/~josh/documents/2023/Procedural%20audio%20review.pdf)). Parameters are starting points at 48 kHz. `noise` = white unless stated; `pink` = −3 dB/oct. `env(a,d)` = linear attack, exponential decay. Every recipe takes `seed`, `pitch` (semitones), `size` (0–1 → longer, lower, wetter) and `intensity`.

### 7.1 Motion
- **Whoosh:** noise → 2 band-passes in series (Q 1.5–3) whose centre sweeps exponentially 300 Hz → 3 kHz → 800 Hz (rise to the peak at 60 %, then fall). Amplitude = speed curve: raised-cosine^1.5, peak at 0.55–0.7·L. Add 10–20 % amplitude jitter (low-passed noise at 20–40 Hz) for "air turbulence". Stereo: pan travel −0.6 → +0.6 along the motion. Length L = motion duration (0.15–1.5 s). For heavy objects, add a second band 80–250 Hz at −10 dB.
- **Swish:** like the whoosh but L = 120–400 ms, sweep 1 → 6 kHz, Q 2–4, sharper attack.
- **Passby:** whoosh plus Doppler. Pitch multiplier (c/(c − v·cosθ(t))) on any tonal layer (engine hum, 80–300 Hz saw through LP 1 kHz). Distance gain 1/r with r_min clamp. High-shelf −6 dB per doubling of distance above 4 kHz.
- **Whip:** noise band-pass sweep 500 Hz → 8 kHz over 60–150 ms, ending in a 1–3 ms broadband click at full scale.

### 7.2 Builds
- **Riser:** saw stack (3–7 voices detuned ±10–25 cents) + noise. Pitch glides up 12–24 semitones exponentially over L. Low-pass opens 200 Hz → 10 kHz (resonance Q 1–4). Amplitude climbs as a ramp^2. Tremolo LFO speeds from 2 Hz to 16 Hz. End: hard cut at L, or a 20 ms fade. The hit point is L.
- **Shepard riser:** 8 sines at f0·2^(k + φ(t)), φ rising 0→1 per period T (4–10 s). Gain per voice = 0.5 − 0.5·cos(2π·(log2(f/20 Hz)/10)) across 20 Hz–20 kHz.
- **Reverse cymbal:** noise → high-pass 3 kHz → resonant band-pass 6–9 kHz. Envelope e^(+5t/L) (an exponential swell), cut at L.
- **Downlifter:** riser mirrored, falling 12–36 semitones, with the low-pass closing.
- **Sub-drop:** sine at 90 Hz → 30 Hz (exponential, 0.6–1.5 s), env(0, L), plus a saturated copy band-passed 100–300 Hz at −10 dB for phones.

### 7.3 Hits
- **Impact:** (a) sub: sine f(t) = 40 + 60·e^(−t/0.04) Hz, amplitude env(0, 0.6–1.2 s); (b) body: noise burst band-passed at 120–250 Hz (Q 1) mixed with a membrane modal bank (1, 1.59, 2.14 × 90–180 Hz), env(1 ms, 200–400 ms); (c) transient: noise HP 2 kHz, env(0, 3–8 ms); (d) tail: everything except the sub into a reverb with RT60 1.5–3 s and pre-delay 20–40 ms, plus 6–12 dB of tanh drive on the bus. Tune (a) and (b) to the key root.
- **Pre-hit suck:** a gain automation on the music/mix of −10 to −20 dB from hit−200 ms to hit−10 ms (reverse-exponential down, instant restore at the hit).
- **Braam:** 3 saws at root (C1–C2 range), +12, +19 semitones, low-pass at 400 → 1500 Hz over 300 ms, tanh drive 12 dB, 2–4 s, hall reverb.

### 7.4 UI
- **Tick:** 1 sample impulse → resonant band-pass 4–6 kHz (Q 8) → env(0, 3 ms).
- **Click:** impulse → 2 modes (≈ 1.8 kHz, 3.7 kHz, T60 15 ms) + noise HP 3 kHz env(0, 2 ms). Vary ±1 semitone per instance.
- **Pop:** sine with f(t) = f0·(1 + 1.5 e^(−t/8 ms)), f0 = 300–700 Hz, env(1 ms, 40 ms), plus a 0.5 ms click.
- **Ding/chime:** additive modes 1, 2.756, 5.404 (bar) with amplitudes 1, 0.4, 0.15 and T60 1.2 s, 0.5 s, 0.2 s. f0 = key root or third in octave 5–6 (520–1760 Hz). Chime = 2–4 notes of the key's pentatonic, 80–150 ms apart.
- **Typing:** per key: noise env(0, 6 ms) band-pass 2.5 kHz + mode at 250–400 Hz (T60 25 ms). Inter-key interval 70–160 ms log-normal, gain ±3 dB, pitch ±1.5 semitones, space bar one octave lower and 2× longer.

### 7.5 Foley / materials (modal bank: Σ a_k · e^(−t/τ_k) · sin(2π f_k t), τ_k = T60_k/6.91)
- **Glass tap/clink:** f0 1.5–4 kHz, ratios 1, 2.83, 5.42, 8.9, T60 0.8–2.5 s scaled by f^−1, excitation 0.5 ms noise. Clink of two glasses = 2 banks with f0 1–3 % apart (beating 5–30 Hz).
- **Glass shatter:** initial broadband crack (2 ms) + 30–80 tinkles (each a 2–3 mode glass bank, f0 3–9 kHz random, T60 50–300 ms) scattered with Poisson rate decaying from 200/s to 0 over 0.3–1.5 s, plus noise HP 4 kHz env(0, 400 ms).
- **Metal hit/clang:** 8–20 inharmonic modes (bar ratios for rods; random ratios 1–12 with density increasing upwards for plates), f0 150–800 Hz, T60 1–5 s, η 1e-4–1e-3, excitation 1–3 ms (hard mallet) or 5–10 ms (soft). Add slow 0.5–3 Hz beating by doubling modes at +0.2–0.5 %.
- **Wood knock:** 4–6 modes, ratios 1, 1.47, 2.09, 2.56, 3.3, f0 150–600 Hz, T60 40–150 ms (η ≈ 0.02 → T60 ≈ 2.2/(0.02·f)), plus a noise attack band-passed 1–3 kHz, 3 ms.
- **Paper crumple:** Poisson impulses 50–400/s (rate following hand speed), each band-passed at random 1–8 kHz (Q 2–5), env(0, 1–4 ms). Page turn: noise band-pass sweep 800 → 3500 Hz over 200–350 ms plus a 5 ms "flick" at the end.
- **Cloth rustle:** pink noise band-pass 500 Hz–4 kHz, amplitude = |velocity| low-passed at 15 Hz, plus sparse ticks (10–30/s) at −20 dB.

### 7.6 Magic / glitch
- **Sparkle:** Poisson grains at 20–60/s over 0.5–2 s (density envelope as a swell). Grain = sine at a random key-pentatonic pitch in 1–8 kHz, env(2 ms, 30–200 ms), random pan ±0.8. Shimmer reverb 3 s, pre-delay 30 ms, high-passed at 1 kHz.
- **Glitch:** take the previous 20–60 ms of the mix (or a tone), retrigger it 3–8 times at 1/32–1/16 beat, bit-crush to 4–6 bits, sample-and-hold downsample to 2–6 kHz, and add 1–3 ms DC-offset clicks. Quantise onsets to frames or beats.

### 7.7 Ambience beds (all loopable via seeded, non-repeating noise; crossfade loops ≥ 2 s)
- **Room tone:** pink noise → LP 3–5 kHz (12 dB/oct) → HP 40 Hz, at −55 dBFS RMS. Optionally add a 50/60 Hz hum with 2nd–3rd harmonics at −70 dBFS for an "electrical" room. Keep a tiny 0.05–0.2 Hz level drift of ±1 dB.
- **Wind:** white noise → resonant LP whose cutoff = 200 + 600·g(t) Hz (Q 0.7–2), where g(t) is smoothed random (0.05–0.3 Hz) plus gust events every 2–10 s (attack 0.5–1.5 s, decay 1–4 s). Whistle layer: 1–2 band-passes Q 15–40 at 400–1500 Hz, centre tracking g(t)·±20 %, at −12 dB. Stereo: decorrelated L/R noise.
- **Rain:** (a) wash: pink noise HP 500 Hz LP 8 kHz; (b) drops: Poisson 200–3000/s (light → heavy), each a 1–2 ms click + Minnaert bubble chirp. For a drop of radius r, f ≈ 3.26 / r(m) Hz (r = 1 mm → 3.3 kHz), rising ~20 % over 5–15 ms, env(0, 10–20 ms). Single drops last 10–20 ms ([Raindrop-Generator after DAFx'04](https://github.com/747745124/Raindrop-Generator), [DAFx'04 P_169](https://dafx.de/paper-archive/2004/P_169.PDF)). Wwise's rain prototype layers "light high-frequency boiling, mid-frequency boiling, stereo noise, and individual raindrops" ([Audiokinetic](https://audiokinetic.com/fr/blog/generating-rain-with-pure-synthesis)). Farnell's Practical 15 provides pulse, impulse and droplet-on-water models ([MIT code](https://mitp-content-server.mit.edu/books/content/sectbyfn/books_pres_0/8375/designing_sound.zip/practical15.html)).
- **Ocean:** pink noise → LP whose cutoff follows the swell s(t) (400 Hz trough → 2.5 kHz crest). Amplitude follows s(t) = a slow asymmetric envelope with period 6–12 s (±20 % jitter): rise 2–4 s, crash peak, 3–6 s decay with a "hiss" layer (HP 3 kHz) that lags the crest by 0.5–1 s (foam retreat). Two or three overlapping wave voices, panned apart.
- **Fire:** roar: noise LP 150–300 Hz, slow AM 0.5–3 Hz; hiss: noise HP 2–3 kHz at −15 dB; crackle: Poisson 3–30/s impulses, each band-passed at random 1.5–6 kHz (Q 3) with env(0, 2–8 ms), occasional "pops" (2–4 % of events) at +8 dB.
- **City:** brown noise LP 200 Hz (traffic rumble) at −30 dBFS + pink noise band 300 Hz–2 kHz at −40 dBFS. Add occasional passby events (§7.1 with an engine tone, 1 per 5–20 s) and distant horn events (two saws 350/440 Hz, LP 1.2 kHz, heavy reverb, −20 dB).
- **Forest:** very quiet pink wash (−50 dBFS) + bird chirps: FM sine 2–6 kHz with fast pitch sweeps (±1 octave in 30–80 ms), in bursts of 3–8 chirps every 3–15 s, reverb 1 s.

---

## 8. Synthesis

### 8.1 Distilled principles

1. **Sound only what matters.** Leave 30–50 % of motion silent. One hero moment per spot; no more than 2–3 SFX onsets/s.
2. **Peak on the event:** transients on the contact frame (0 to +1 frame late; more than 1 frame early tends to read as wrong; detection starts at 45 ms early, 125 ms late). Whoosh peak on the cut (pre-roll ≈ 0.6 × length). Riser end on the hit.
3. **Silence before the hit:** dip everything 10–20 dB for 80–300 ms.
4. **Layer by band:** sub 30–60 Hz (dry, one owner), body 100–500 Hz, transient 2–8 kHz, tail. Two to four layers for heroes, one or two for the rest.
5. **Vary repeats:** an exact repeat tends to sound like a machine gun; ±0.5–2 semitones, ±1–3 dB, 3–8 variants, round-robin.
6. **Tune tonal SFX to the music's key** and quantise rhythmic SFX to its grid.
7. **One space per scene:** shared reverb send, dry/wet by distance, sub kept out of the reverb.
8. **Pan with the picture** at ≈ ±0.6 of the frame, equal-power. Keep lows below 150 Hz mono.
9. **Master to the destination:** −14 LUFS / −1 dBTP for web (−16 Apple, −23 EBU, −24/−2 ATSC). Measure with BS.1770 and limit true peak.
10. **Keep the dynamics phone-friendly:** LRA 4–8 LU for short-form, ≤ 3–4 dB limiter gain reduction.
11. **Duck music** 6–12 dB under VO (attack 10–30 ms, release 200–500 ms, with look-ahead) and 2–6 dB under hero hits.
12. **Make the bass translate:** add saturated 100–400 Hz harmonics to sub hits and high-pass the master at 25–30 Hz.
13. **Keep one palette:** shared timbres, one reverb character, and an audio logo of 2–3 s.
14. **Match the sound to scale and speed:** bigger means lower, slower and wetter; faster means brighter and shorter.
15. **Procedural first, CC0 second:** vendor only CC0, and fetch everything else per user with their own key.

### 8.2 Studio vs amateur

| Aspect | Studio | Amateur / stock |
|---|---|---|
| Density | 1 hero per 4–8 s; most motion silent | every move whooshed, every cut hit |
| Sync | sample-accurate, peak on the frame, encoder offset compensated | sound file *starts* on the cut, so the peak lands 0.3–0.8 s late |
| Pitch | tuned to the score's key, rhythm on the grid | random pitches clash with music |
| Layering | sub/body/transient/tail, EQ-slotted, 2–4 layers | one library file, or 6 stacked unfiltered files |
| Repetition | per-instance variation, round-robin | identical clicks ("machine gun") |
| Space | one reverb per scene, dry/wet by distance | dry and huge-tail sounds side by side |
| Pre-hit | 80–300 ms suck/silence | wall of sound into the hit |
| Low end | one sub owner, harmonics for phones, mono < 150 Hz | sub on every layer; inaudible on phones and pumps the limiter |
| Loudness | −14 LUFS, −1 dBTP, LRA 4–8 | −6 to −9 LUFS, clipped, gets turned down and sounds crushed |
| Ducking | look-ahead, 6–12 dB, smooth release | none, or a hard 20 dB pump |
| Stereo | follows on-screen position, ≈ ±0.6 | everything centred, or hard-panned gimmicks |
| Identity | consistent palette, sonic logo | a different library per cue |

### 8.3 Gap list against Anikit (prioritised)

Based on `crates/anikit-video/src/audio/{sfx,mix}.rs`, `crates/anikit-video/src/spec/mod.rs` (`AudioCue`), `crates/anikit-video/src/eval/compile.rs` (around line 3724), `crates/anikit-video/src/review.rs`, `crates/anikit-music/src/sfx.rs` and `crates/anikit-music/src/dsp/*`.

**P0: correctness and table stakes**
1. **Hit-point alignment for video SFX cues.** `AudioCue.at` sets the *buffer start*, so a whoosh peaks about 0.4–0.8 s after the intended moment. `anikit-music::sfx` already returns `Sfx.hit` (and the music cue's `sfx` list aligns by it: "a riser PEAKS at 'at'"), but the video `audio/sfx.rs` path has no hit point. Add `align: "hit"` as the default for whoosh/riser/swell (with `"start"` as the opt-out), and place by `at − hit`.
2. **Loudness-normalised master.** The video mixer (`audio/mix.rs`) ends in a peak limiter only. `review.rs` estimates loudness with a gated RMS ("≈ LUFS"), not K-weighted. `anikit-music/src/dsp/loudness.rs` already has a full BS.1770-4 / R128 meter with true peak, plus `dsp/master.rs` with a limiter. Wire them into the final video mix: `audio.master: { lufs: -14, truePeak: -1, preset: "web"|"apple"|"broadcast-eu"|"broadcast-us" }`. Report the measured LUFS, dBTP and LRA in review.
3. **Per-effect relative levels.** `audio/sfx.rs` normalises every effect to a −1 dBFS peak, so a `tick` is as loud as an `impact`. Adopt the per-kind level table of `anikit-music::sfx` (UI sounds 12–20 dB under hits).
4. **One SFX library.** There are two generators: `anikit-video::audio::sfx` (13 names; params `duration`, `pitch` multiplier, `seed`) and `anikit-music::sfx` (24 names; `pitch` in semitones, `intensity`, `size`, `width`, `pan`, `motion`, `gain`, `count`, `bpm`, hit points, true-peak-safe). Route `AudioCue.sfx` to `anikit-music::sfx` and expose its params in `AudioCue` (keep the old names as aliases).
5. **Automatic variation.** `compile.rs` builds `SfxParams { …, seed: 0 }`, so every repeated cue is sample-identical (machine gun). Derive the seed from the cue index or path, and add default micro-variation (±1 semitone, ±1.5 dB, 0–8 ms).

**P1: craft features the spec needs**
6. **Pan and motion per cue:** `pan`, plus `follow: "<layer id>"` to auto-pan from the layer's on-screen x (scaled ±0.6) and auto-set whoosh `motion`, duration and peak from the layer's tween (speed → band centre, peak velocity → hit).
7. **Key/tempo-aware tonal SFX:** inherit `key` and `bpm` from the `music` cue. Tune ding, chime, success, braam, sub-drop and riser end pitch to the root, fifth or pentatonic set. Optionally quantise `at` to the beat grid (`beats.rs` already exists).
8. **Pre-hit "suck" and SFX-keyed ducking:** `Duck.under` can technically be `sfx`, but there is no short-transient preset. Add `suck: { ms: 150, db: 12 }` on hero cues and a `duck: "hit"` preset (3–6 dB, 5 ms attack, 250 ms release), ideally low-band only.
9. **Scene space / reverb bus:** a per-scene `space: "dry"|"room"|"hall"|"cathedral"` that sends all SFX through one shared reverb (`anikit-music::dsp::reverb` already has presets), with a dry/wet derived from layer scale/z (distance).
10. **Ambience beds:** room tone, wind, rain, ocean, fire, city and forest generators (§7.7), loopable and non-repeating. None of these exist in either crate.
11. **Material foley:** a modal-bank generator with material presets (glass, metal, wood, plastic, membrane) and noise-texture generators (paper, cloth, shatter). Today there is only `click`, `tick`, `pop` and `snap`.

**P2: polish, lint and assets**
12. **Phone translation:** an automatic harmonic layer on sub content (`dsp/saturate.rs` exists), a master high-pass at 25–30 Hz, and a review check that the hero events still read through a 200 Hz–8 kHz "phone sim".
13. **Density and sync linter in `review.rs`:** warn on more than 3 SFX onsets/s, whooshes on more than 60 % of cuts, more than 4 identical cues within 2 s, tonal SFX out of the music's key, a hero hit with no preceding gap, and transients more than 1 frame from the nearest layer keyframe or cut. Also measure and compensate the AAC encoder priming offset.
14. **Missing elements:** passby (Doppler), Shepard riser, sparkle/magic, page turn, glass shatter. `glitch` and `tape-stop` exist only in the music crate.
15. **Asset path:** vendor a curated **CC0-only** pack (Kenney Interface/UI audio) under `assets/sfx/` with its licence file. Add an optional `freesound:` source that requires the *user's* `FREESOUND_API_KEY`, filters `license:"Creative Commons 0"`, downloads previews only, caches per render, and records attribution in the render report. Do not vendor Sonniss, BBC, Pixabay or Mixkit files.
16. **Sonic-logo / palette preset:** a `brand.sound` block (palette of timbre, reverb and key) applied to all cues, plus a 2–3 s logo sting generator.

### 8.4 Status (v-sound branch, 2026-11-18)

Done in `crates/anikit-video/src/audio/design/` (`anikit-video docs sound`):

- **1** — hit alignment: design cues and `audio[].sfx` with design names land their hit on `at`; the old 13 names keep their old placement for compatibility.
- **2** — LUFS master: the bus masters to −14 LUFS / −1 dBTP (configurable), and review reports BS.1770 LUFS, true peak and LRA of the real render mix.
- **3** — per-kind levels.
- **4** — one registry: design + music library + UI library behind one name space.
- **5** — round-robin seeds with ±1 st / ±1.5 dB variation, and alternating families on runs of moves.
- **6** — pan and motion are derived per layer from the picture (`"sound": "auto"`, per-layer `sfx`).
- **7** — key-tuned tonal effects. Beat quantising is not done: sync to picture wins.
- **8** — hit ducking with look-ahead and the pre-hit `suck`.
- **9** — one shared space per film. Per-scene spaces are not done.
- **10** — 11 ambience beds.
- **11** — modal foley plus paper, cloth and shatter textures.
- **12** — partial: thump harmonics and the master low cut; there is no phone-sim review check.
- **14** — passby, Shepard, sparkle/magic, page and shatter.
- **15** — partial: CC0 packs are fetched on demand rather than vendored; there is no Freesound source.
- **Beyond the list:**
  - physics collisions (scene3d) are heard with the body's material;
  - SFX and beds step back under voice-over.

Still open:

- **13** — a density and sync linter in review.
- **16** — a brand sound palette and logo sting.
- Per-scene spaces.
- Beat quantising as an option.

---

### Sources (beyond those inline)
[Morphic: how to layer sound effects](https://morphic.com/resources/how-to/how-to-layer-sound-effects) · [Violet Recording: impact sounds](https://violetrecording.com/how-to-make-impact-sounds/) · [Ableton: trailer impacts](https://www.ableton.com/en/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers/) · [SOS: Sound design for visual media](https://www.soundonsound.com/techniques/sound-design-visual-media-radio) · [Krotos/Hobler: SFX for motion graphics](https://krotos.studio/blog/sound-effects-motion-graphics-tips) · [Stash: State of Sound in Moving Images](https://www.stashmedia.tv/guest-editorial-the-state-of-sound-in-moving-images-part-1/) · [ITU-R BT.1359-1](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.1359-1-199811-I!!PDF-E.pdf) · [EBU R128 s1](https://tech.ebu.ch/docs/r/r128s1.pdf) · [Spotify loudness](https://support.spotify.com/us/artists/article/loudness-normalization/) · [Apple Podcasts audio requirements](https://podcasters.apple.com/support/audio-requirements) · [Netflix sound mix spec](https://backlothelp.netflix.com/hc/en-us/articles/360001794307-Netflix-Sound-Mix-Specifications-Best-Practices-v1-5) · [Freesound API overview](https://www.freesound.org/docs/api/overview.html) · [Freesound API ToS](https://freesound.org/help/tos_api/) · [Sonniss GDC bundle](https://sonniss.com/gameaudiogdc) · [Kenney Interface Sounds](https://kenney.nl/assets/interface-sounds) · [Pixabay licence](https://pixabay.com/pt/service/license) · [CCRMA modal synthesis](https://ccrma.stanford.edu/workshops/dsp2008/prc/modal.pdf) · [Csound modal ratios](https://csound.com/manual/MiscModalFreq.html) · [Farnell, Designing Sound, Practical 15](https://mitp-content-server.mit.edu/books/content/sectbyfn/books_pres_0/8375/designing_sound.zip/practical15.html) · [LANDR: bass on phones](https://blog.landr.com/make-bass-audible-phone-speakers) · [AES: bass enhancement for small speakers](https://www.aes.org/e-lib/browse.cfm?elib=19278) · [Listen: sonic branding](https://wearelisten.com/what-is-sonic-branding-pt-2) · [Antfood](https://antfood.com/about/) · [No Film School: Star Wars sounds](https://nofilmschool.com/star-wars-sound-design) · [Kottke: Shepard tone in Dunkirk](https://kottke.org/18/05/hans-zimmers-clever-use-of-the-shepard-scale-in-dunkirk) · [Salford: dialogue-to-background levels](https://salford-repository.worktribe.com/OutputFile/1488623) · [Vindral: sidechain ducking](https://composer-docs.vindral.com/operators/operators-sidechain-ducking.html)
