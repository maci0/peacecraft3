# Synthetic MPEG audio fixtures

These files are generated tones, not game recordings. No supplied game asset is
an input. They exercise mono/stereo and presence/absence of a Xing/LAME header.
Tests derive frame counts and hashes from the decoded input instead of relying
on one encoder build's output bytes.

Generation uses Python's `wave`/`struct` modules and LAME 3.100. Write 11,025
signed little-endian 16-bit frames at 44,100 Hz. For channel `c` and frame `n`,
the sample is `round(12000 * sin(2*pi*(440 + 220*c)*n/44100))`. The mono files
have one channel; stereo has two. Encode with:

```sh
lame --silent --noreplaygain --resample 44100 -b 128 -m m mono.wav mono-xing.mp3
lame --silent --noreplaygain --resample 44100 -b 128 -m m -t mono.wav mono-noxing.mp3
lame --silent --noreplaygain --resample 44100 -b 128 -m s stereo.wav stereo-xing.mp3
```

LAME is needed only to regenerate these fixtures, not to compile or run tests.
The rates, tones and lengths above are synthetic test choices.
