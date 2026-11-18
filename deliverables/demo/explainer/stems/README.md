Stems are not committed (≈17 MB); regenerate them with:

    AK=anikit-video
    $AK sound render city -o city.wav duration=30 seed=3
    $AK sound render crowd -o crowd.wav duration=30 seed=4
    $AK sound render room -o room.wav duration=30
    $AK sound render droplet -o drop.wav pitch=-5
    $AK sound render metal -o metal.wav intensity=0.5 size=0.5
    $AK sound render whoosh-air -o rise.wav duration=1.4 speed=0.3
    for i in 1 2 3; do $AK sound render blip -o blip$i.wav seed=$i pitch=-12; done
    $AK sound render glass -o glass.wav intensity=0.8 size=0.8 tune=587.33
    $AK sound render thump -o thump.wav intensity=0.3

then `python3 ../mix.py` writes ../soundtrack.wav.
