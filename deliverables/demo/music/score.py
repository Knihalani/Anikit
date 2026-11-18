"""Writes score.json: the music spec and the lyric's syllable timing (python3 score.py)."""
import json
B = 60 / 84
m = json.load(open("music.json"))
lyric = [("Hold", 4), ("me", 5), ("at", 5.5), ("low", 6), ("tide", 7),
         ("when", 10), ("the", 10.5), ("har-", 11), ("-bour", 11.5), ("lights", 12), ("are", 13), ("gone", 13.5),
         ("we", 16.5), ("were", 17), ("ne-", 17.5), ("-ver", 18), ("meant", 18.5), ("to", 19.5), ("stay", 20),
         ("but", 22.5), ("we", 23), ("stayed", 24), ("on", 25)]
out = {
    "title": "Low Tide", "artist": "Harbour Lights", "bpm": 84, "key": "D major", "meter": "4/4", "seconds": 20.0,
    "form": "intro bar (lamps switch on per piano eighth) · line 1 (Bm→G) · line 2 (G→A) · line 3 (Bm G → Gm, the borrowed iv on 'stay') · a half-beat breath · line 4 resolves to D; 'on' rings",
    "melody": "cello section, one note per syllable; piano (Splendid) broken chords; string sections, contrabass, a soft kick, a crash on the arrival, celesta for the lights, and a synthesised sea that drains with the tide and surges back",
    "lyric": [{"syllable": w, "beat": b, "seconds": round(b * B, 3)} for w, b in lyric],
    "spec": m,
}
json.dump(out, open("score.json", "w"), indent=1)
print(len(out["lyric"]), "syllables")
