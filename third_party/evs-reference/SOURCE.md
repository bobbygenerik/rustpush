# EVS reference decoder source

Unmodified `lib_com`, `lib_dec`, and `readme.txt` from the 3GPP TS 26.443
floating-point reference codec, 2021-11-04 (version 16.3.0 / earlier releases).
Retrieved from https://github.com/old6ix/EVS_codec at
`1bff9329ea91bc5a3ba869006051ae8223ddb3a9`.
Specification: https://www.3gpp.org/dynareport/26443.htm.
Original file notices are preserved. `obim_decoder.c` is this fork's adapter.
Only decoder/common sources are built; no EVS encoder is included.
`lib_enc` contains only the upstream headers required by the common prototypes.
The build defines prefixed names for `exp2_tab_long`, `exp2w_tab_long`, and
`exp2x_tab_long` to coexist with FDK-AAC without changing upstream source.

`test-tone-framed.bin` is synthetic, not recorded call audio: the reference
encoder encoded one second of 440Hz, 48kHz mono signed-16 PCM, amplitude 8000,
with `EVS_cod -q -mime 24400 48 tone.48k tone.evs`. The 16-byte MIME header was
removed; each primary-mode ToC byte (`06`) was replaced with the 61-byte access
unit length (`3d`) used by the observed FaceTime compact framing.
