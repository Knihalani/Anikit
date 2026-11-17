# `anikit-video board` examples

Two boards from public-domain references outside tech. Each has `*-board.json` (palette, accents, value, grade, texture, composition, line character, tokens, spec patch, summary) and `*-board.jpg` (the sheet).

- `night/`: three photographs.
  - `aurora2.jpg`: aurora and city lights from the ISS, NASA image iss035e008716.
  - `earthris.jpg`: a crescent Earth, NASA image art002e009280b.
  - `pillars.jpg`: the Pillars of Creation, NASA image GSFC_20171208_Archive_e000842.

  All three come from the NASA Image and Video Library (images.nasa.gov) and are NASA works in the public domain. They're reduced to 800 px here.
  - Board: low-key (mean L 0.17), low contrast, near monochrome, visible grain, 59% negative space, organic and curving lines.
  - Tokens: bg #040407, text #E0D1C3, primary #5A78C2 (the blue of the aurora and nebula), accent #8D5432 (city-light amber).
  - Post: grain 0.02, vignette 0.45 (measured on the 800 px copies here).
- `print/hokusai-great-wave.jpg`: Katsushika Hokusai, *The Great Wave off Kanagawa* (c. 1831), public domain, from Wikimedia Commons. Reduced to 800 px here.
  - Board: high-key paper ground (#F2EAD1), Prussian-blue text and primary (#24405F / #234162), skin-tone accent (#E5B595), clean, hard graphic edges, diagonal and dynamic line character, lifted blacks.

`board` reads colour, value, texture, composition and line character. Use those, not the subject matter or the artwork itself.
