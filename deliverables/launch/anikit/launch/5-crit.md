# 5 · Outside critique (fresh subagent, given only the brief and launch/pass1.png)

Verdict, pasted:

> **1. Idea:** a line of words on a paper card under a desk lamp turns into a film, and the same words always give the same film. Without the product's name it shrinks to "text becomes video", which every generative tool claims. It would work just as well with Sora, Runway or Veo on it. The one thing only Anikit can say is "byte-for-byte the same", and the film never shows it; it only captions it ("Same words. Same film.", frame 8).
>
> **2. Checks:** 1 YES with caveat (amber → grey-white is an arc; frames 9, 11 black voids) · 2 NO (inside the cards the material changes: ocean, glass on oak, ink; 4, 9, 11 have no world) · 3 NO (lamp is a glowing sprite, cards lit almost flat; 4, 9, 11 black) · 4 NO (lighthouse at the same angle in 2, 3, 7, 8, 10) · 5 NO (4, 9, 11 are 80–100% black; 6 is a collage with no focal subject) · 6 NO · 7 NO · 8 NO (frame 6 is a range montage; captions read as prompt demos) · 9 YES (captions printed on the lit cards) · 10 NO (frame 12: centred serif headline + mono command on a blank card) · 11 YES · 12 NO (frame 3 crops the caption; frame 7 overlapping captions) · 13 NO (9 and 11 dead black; static end card) · 14 NO (logo-and-command slide) · 15 NO (≈5 changes in the first half).
>
> **3. Weakest:** frame 9 (pure black), frame 11 (blank card corner in a void), frame 4 (lamp, black, blank card edge); runner-up frame 6, the sampler collage — a feature reel.
>
> **4. Single change:** the idea is the problem. The ownable truth is determinism: the same words give the same film, every time, byte for byte. Hand the same card to several places and show identical films landing in perfect sync, every rain streak and the boat in the same spot. Make the sameness the spectacle and the visual punchline; let the last card's words render into the name itself. Give the lamp real falloff on the paper; drop the black voids.
>
> **5. Cut:** the black and blank-card beats (4, 9, 11); two of the five lighthouse repeats; the frame-6 collage; the mono command line on the end card.

## Changes for pass 2
- **Idea sharpened (device unchanged, its meaning changed):** the darkroom stays, but what escalates is *sameness*, not variety. One sentence → one print (bars 1–3) → a second sheet of the same sentence develops beside it and the two play in lockstep (bar 4–5) → a tray of nine sheets of the same sentence, every beam, wave and rain streak in sync (bars 5½–6) → a second print slides onto the first as a ghost and snaps into one image (bars 7–8). The promise now reads "exactly" from the picture, not only the caption.
- **Cut the weakest third:** the glass print, the ink and "Rain." prints and the four-film collage (feature reel); the empty sheet-entry frames (sheets now enter already in frame); the mono end-card line.
- **No black voids:** the hard stop is a 1.25 s *dimming* of the safelight over the twin print (its lamp reflection dies last), not a cut to black.
- **Payoff in room light is a new composition:** the tray of identical prints in true colour, camera close and rotated, not the lighthouse framed like before.
- **Ending through the device:** the last print develops like every other, and the name is *inside its picture* (standing in the dusk sea, crossed by the beam); its caption, the words that "made" it, is the command `anikit-video render spec.json`. No separate logo card.
- **Light:** steeper falloff of the safelight across the paper (darker far corners), lower ambient floor.
- **Captions:** no accidental crops; overlapping captions removed.

## Pass 2 check (contact.png = launch/pass2.png, one frame every 2.5 s of final.mp4)
1 YES: one safelight logic → the same light in white (arc) · 2 YES: paper under developer in a black tray in every frame · 3 YES: hard lamp reflection plus steep falloff across the paper (no radial bloom) · 4 YES: macro words / print / lantern close / two in lockstep / tray of nine / twin snap / colour tray turned / name print · 5 mostly: cells 9 and 11 land on cut frames (the room-light switch-on at 20.0 s, the sheet entering before it develops at 25.0 s) and are on screen for 1–2 frames · 6 YES: "a sentence develops into a living print, and every print of it is identical" survives without the name · 7 YES: a generative tool can't show nine identical prints of one sentence · 8 YES: no technique sampler, no feature captions · 9 YES: all type is printed on paper under the liquid · 10 YES: no kit labels; the end is a print like the others · 11 YES · 12 YES: crops are deliberate camera framing, no overlapping captions · 13 YES: liquid, rain and beam alive to the last frame; last two frames differ · 14 YES: the name develops inside the last print, the command is its caption · 15 YES (≈8 major changes in the first 15 s).
Shipped after pass 2 (the idea changed once: variety → sameness). Delivery note: the engine's MP4 (114 MB, grain-heavy) was transcoded with x264 CRF 19 to fit GitHub's 100 MB file limit (audio stream copied untouched).
