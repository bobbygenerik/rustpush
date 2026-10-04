#include <stdlib.h>
#include <stdint.h>
#include <math.h>
#include "options.h"
#include "cnst.h"
#include "prot.h"

/* Compact EVS primary-mode frame. FaceTime's AU length byte is stripped by
 * Rust; no RTP ToC byte is present in this compact form. */
void *obim_evs_create(void) {
    Decoder_State *st = calloc(1, sizeof(*st));
    if (!st) return NULL;
    st->output_Fs = 48000;
    st->bitstreamformat = VOIP_RTPDUMP;
    init_decoder(st);
    reset_indices_dec(st);
    return st;
}

int obim_evs_decode(void *state, const uint8_t *bytes, int bits, int16_t *pcm) {
    if (!state || !bytes || !pcm || bits < 48 || bits > 2560) return -1;
    Decoder_State *st = state;
    float output[L_FRAME48k];
    read_indices_from_djb(st, (unsigned char *)bytes, bits, 0, 0, 1, 0, 0);
    evs_dec(st, output, FRAMEMODE_NORMAL);
    syn_output(output, L_FRAME48k, pcm);
    if (st->ini_frame < MAX_FRAME_COUNTER) st->ini_frame++;
    return L_FRAME48k;
}

void obim_evs_destroy(void *state) {
    if (!state) return;
    destroy_decoder(state);
    free(state);
}
