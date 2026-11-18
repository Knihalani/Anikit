# 1b · The subject's world

Two worlds meet in this film: the people the product is for, and the world the film borrows its device from. No web access was used for this. The details below are from knowledge and are marked **[sure]** or **[guess]**.

## The audience: developers building agents, and designers working with them
- **Ritual [sure]:** the agent receives a request, writes a spec (JSON), calls a tool, and gets a file back. Then the next step in its loop starts. Nobody watches the render; they look at the result.
- **What they care about [sure]:** reproducibility (same input, same output, so the result can be tested, diffed and cached); speed of the loop; no human in the middle.
- **Their words [sure]:** "spec", "deterministic", "byte for byte", "same seed", "diff", "hash", "CI", "tool call", "render", "ship".
- **Insider detail [sure]:** a deterministic render can be checked by hash: render twice, `sha256sum`, and the two lines match. Generative video can't do that: the same prompt gives a different clip every time.

## The device's world: the competitive marching band (field show)
- **The drill writer [sure]:** one person writes the whole show months ahead, as numbered *sets* (formations), each reached in a given number of *counts*. Today it's written in drill software (Pyware) and printed out.
- **The dot book [sure]:** every performer carries their own coordinates on a card on a lanyard or in a flip folder: "Set 14 · 8 counts · 2 steps outside the 35 · 4 behind the front hash." Each player reads only their own line, and the whole picture comes out of that.
- **Steps [sure]:** "8 to 5" = eight steps per 5 yards (one step per beat); "mark time" = stepping in place; "glide step / roll step" = the smooth heel-to-toe walk that keeps the horn still; "horns up" is the snap of the horns into playing position.
- **Field details [sure]:** yard lines every 5 yards and hash marks; the drum major on the podium; the "front sideline"/"home side" faces the press box; the "company front" is a long line that advances on the audience for the big hit; "park and play" is standing still to play.
- **Rehearsal [sure]:** the field crew sprays **coloured dots** on the practice field for sets that are hard to find; "cleaning" a set means fixing everyone's spacing until no one is off.
- **Judging [sure]:** in competition, visual judges watch from the press box and **mark "ticks"**, one for every error: a foot out, a line bent, an interval off. A "clean" show has no ticks.
- **Sound [sure]:** the snare line "taps off" the show; a cadence plays on the move; brass "hits" land with horns up; the pit (front ensemble) carries glockenspiel and timpani; there is silence after the big hit before the closer.
- **Insiders know [guess]:** the same show is performed every week of a season; bands are judged on how identically they can repeat it.

## What this gives the film
- The **dot book** is a spec: one written page, read by many performers, each executing only their own line, and the result is a picture that moves. That's what an agent's spec does inside Anikit Video, without the metaphor being a diagram.
- **Ticks** are the measure of error. A deterministic render has none, and the same show on another night matches the first to the step.
- **The field show is literally an animation drawn with people.** Sets are keyframes, counts are frames, and the performers are pixels.
