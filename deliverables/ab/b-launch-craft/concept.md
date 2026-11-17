# Railyard — 15 s launch film · concept

## Single-minded proposition
With Railyard, money arrives the second it leaves.
(Supporting, not a second message: you hear about every step — a webhook at every state change.)

## Blind brainstorm (clichés I already believe — the "obvious" list)
1. Money as glowing particles flying between two bank icons. 2. A globe with arcs.
3. Code editor beside a headline. 4. A stopwatch reading 0.00 s. 5. "Fast" = speed lines, motion blur, rockets.
6. Dashboard with a green tick. 7. Trains: a bullet train carrying coins. 8. Indigo gradient + glass cards.
9. Credit cards spinning in 3D chrome. 10. "Payments, reimagined." 11. Lightning bolt. 12. Two phones pinging.
13. A terminal typing `curl`. 14. Neon webhooks as cables. 15. Money falling like confetti. 16. Railway tracks
converging to a logo. 17. Notification toasts stacking. 18. "Built for developers."

## Concrete facts (raw material)
1. ACH transfers commonly settle in 1–3 business days. 2. ACH moves in batches, at set windows through the day.
3. Wires have cut-off times; miss one and it goes tomorrow. 4. Weekends and bank holidays don't settle.
5. A US routing number is 9 digits. 6. Account numbers are usually masked to the last 4.
7. A webhook is an HTTP POST your server receives. 8. A transfer has states: created → pending → settled (or failed).
9. Railyard: one API call moves money between bank accounts. 10. Railyard settles instantly.
11. Railyard sends a webhook at every state change. 12. A railyard is where cars are sorted onto the right track.
13. A hump yard sorts freight cars by gravity. 14. Signal boxes route trains with lever frames.
15. Station departure boards (Solari, Udine, from the 1950s) use split-flap displays.
16. A split-flap cell flips through its alphabet in order; it can't skip — it clatters.
17. Each flap flip is ~a twentieth of a second; a whole board resetting is a famous sound.
18. A board column reads SCHEDULED / DELAYED / ON TIME / BOARDING / ARRIVED.
19. Station chimes (ding-dong) announce a change before the voice.
20. Finance people still say money is "in transit". 21. "The cheque is in the post." 22. Treasury teams wait on a
cut-off clock. 23. Payroll is held until funds clear. 24. The phrase "funds available" vs "funds settled".
25. Timetables print departure AND arrival times — the gap between them is the product's enemy.

## 33 one-line concepts (1–10 are the obvious wave, discarded)
1. ~~Particles fly from bank A to bank B.~~ 2. ~~Globe with glowing arcs.~~ 3. ~~Code beside the headline.~~
4. ~~A stopwatch stops at 0.00.~~ 5. ~~A bullet train of coins.~~ 6. ~~Toast notifications stack up.~~
7. ~~Tracks converge into the logo.~~ 8. ~~Terminal types curl, a tick appears.~~ 9. ~~Two phones ping.~~
10. ~~A chrome card spins.~~
11. A hump yard sorting freight cars, each car a transfer, rolling by gravity onto the right track.
12. A signal box: one lever pull (the API call) turns a whole line of signals green, one per webhook.
13. A cheque in the post, overtaken by its own receipt.
14. A timetable where departure and arrival columns are the same number.
15. A split-flap departure board of money, every row DELAYED — then one row that isn't.
16. Each clack of a flap is a webhook: the board *is* the event stream.
17. An hourglass whose sand never falls because the bottom's already full.
18. A weekend calendar with money stuck in Saturday.
19. A pneumatic tube canister arriving before it's sent.
20. A telegraph operator tapping "SETTLED" as the key goes down.
21. A railway clock that never ticks during the whole film: it all happened in one second.
22. Freight manifest stamps: RECEIVED, PENDING, CLEARED, stamped in rhythm.
23. A station announcer chime for each state change.
24. Lost-property office full of payments "in transit".
25. A model railway where the train is already at the end of the line when we look.
26. A ticket punched at both ends at once.
27. A points switch thrown: the money changes track without slowing.
28. A queue at a bank counter that dissolves.
29. A ledger written with two hands at once, debit and credit simultaneously.
30. Semaphore arms dropping in sequence along a line, each one a webhook.
31. A board that reports delays, forced to report none.
32. The board's clock: the entire film between 09:41:06 and 09:41:08.
33. "Next departure: now."

## The turn (de Bono provocation)
Obvious: "money moving between banks, shown moving fast" (particles, trains, speed).
Po — ESCAPE: remove the movement. Show no train, no money, no motion lines. Show only the place that reports
on movement: the departure board. Po — REVERSAL: a departure board exists because departure and arrival are
different times. On Railyard's row they are the same time. The speed is shown by a number, not by blur.
And the flaps clattering at every state change *are* the webhooks — the board is literally an event stream.
Chosen: 15 + 16 + 14 + 21 → "A departure board for money, where one row departs and arrives in the same second."
Swap test: a generic fintech can't use it — it depends on settlement-as-arrival and a board that speaks at
every state change (webhooks). The name Railyard makes the station world native, not borrowed.

## Who made these images, with what, in what mood?
The board itself: a 1970s split-flap board in a freight railyard's night office, built in Udine, maintained by a
dispatcher who has watched DELAYED for twenty years. Made of painted aluminium flaps, a hinge in every cell, a
mechanical clatter. Mood: dry, nocturnal, deadpan — the board is slightly amazed at itself.

## Structure
Story through an interface (#4) with a reversal: the board, built to report waiting, has nothing to wait for.
Hook (0–1 s): macro on flaps mid-clatter, landing on DELAYED. Develop (1–5.5 s): pull back — a board of money
waiting (FRI, +2 DAYS, HELD, QUEUED); one row clatters POST /V1/TRANSFERS. Turn (6 s, downbeat): that row
reads SENT 09:41:07 · ARRIVES 09:41:07. Status flips CREATED → PENDING → SETTLED, a chime and a lamp per
webhook. Payoff (10–13 s, held 2.5 s): the whole board clatters into RAILYARD + the line.
Button (13.4 s): the lines clear; under RAILYARD one row flips to DELAYS: NONE — rhyming with the DELAYED of frame 0. (The board clock, a plant from 1 s, reads 09:41:06 → 09:41:07: the whole story happens inside one second.)

## Treatment
**Railyard — "Arrivals"**. A departure board for money, where one row departs and arrives in the same second.
**Why:** the enemy of a payment is the gap between departure and arrival; a timetable is the one object that
prints that gap. Close the gap on the board and the claim needs no adjective.
**World rules**
- Palette: housing enamel black-green #0C100E (ground); flap charcoal #1E2120/#171918 (material); flap ink warm
  white #EEE7D7 (all words); signal amber #F2A93B (waiting states only); signal green #39D37E (settled only).
- Type: the flaps are the only type. One condensed-mono face, uppercase, one size per board; printed column labels
  in small spaced caps on the housing. No floating captions, ever.
- Material: the flap — top half lighter, hinge slit, each cell a hair different. Pushed to the limit: real flap
  order (cells must cycle through the alphabet, they can't skip), per-cell stop times, a clack per flip.
- Camera: a dispatcher's view — locked off, telephoto, one long pull back, one push to the row, no whips.
- Light: one warm practical above the board (falloff top → bottom), lamps glow by their own light.
- Sound: the clatter is the rhythm section; a sparse low pulse and piano; station chime = webhook.
- Never: particles, coins, trains, globes, gradients, UI cards, logos floating on empty space.
**Style frames:** sf1 hook macro (t=0.4), sf2 the turn (t=7.9, SENT = ARRIVES, PENDING), sf3 payoff (t=11.5), sf4 button (t=14.6) — in frames/.
**References (outside tech):** Solari di Udine boards (Milano Centrale, 30th St Philadelphia) — the material and
sound; Tati's *Playtime* — deadpan machine comedy; railway timetable typography (Deutsche Bundesbahn
Kursbuch) — the grid.
