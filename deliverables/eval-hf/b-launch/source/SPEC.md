# Railyard — 15 s launch film (SPEC)
GOAL: the viewer leaves knowing one API call moves money between bank accounts instantly, with a webhook at every state change.
OUTPUT: HyperFrames 1920x1080, 30 fps, 15 s = 450 frames.
LOOK: railway signal-box diagram. Ground #E7E9E4 (yard grey), ink #121311, one accent: signal orange #FF4F1A (the money token, the lit lamps, the end card).
      Display Inter 900, -4.5% tracking, 96–200 px; code/webhooks JetBrains Mono 38–56 px.
OBJECTS: track = two ink rails + grey sleepers; signals = post + ring lamp that lights orange; the transfer = orange token "$2,400.00" riding the rails.
MOTION: arrivals expo.out/power4.out; camera/flood power2.inOut; exits power3.in. Titles land and hold still.
SHOTS:
  0.0–2.4  hook: "$2,400.00 to Acme Supply · Processing", "Arrives Tuesday→Wednesday→Thursday→Friday" (slot roll), "3–5 business days." Orange band whips through (train).
  2.4–4.0  "Railyard moves it now." per-word blur in on orange.
  3.9–5.6  one call: code panel morphs open, POST /v1/transfers typed, 200 response.
  5.6–6.0  carry: the "2400.00" value lifts out and becomes the token; ground floods to yard grey.
  6.0–8.0  the yard builds; token runs the track in 1.1 s; each signal it passes lights and fires a webhook (created, pending, settled); balances move.
  8.2–11.0 "Settled instantly." / "A webhook at every state change."
  11.0–11.6 token swells past the frame → orange world.
  11.6–15  mark laid like track (two rails + a switch), wordmark wipes in on the impact, "One call. Settled instantly.", CTA "POST /v1/transfers" holds 2 s.
SOUND: rail-clack bed (tools/bed.py, built from bundled samples at 122 BPM), slot clicks, whip whoosh, typing, enter, pops on webhooks, riser into impact on the mark.
BANNED: dark void, glow/particles, purple gradients, glass cards, cream+serif, three-card feature rows, docked logo, fade-and-slide-up on everything.
