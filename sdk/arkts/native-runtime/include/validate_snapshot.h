#pragma once
#include "provider_abi.h"
/* Pure boundary validation only; checked increment remains in each provider. */
static inline int mp_valid_snapshot(mp_snapshot s) {
    if (s.size > 1024 || (s.size && !s.text))
        return 0;
    size_t at = 0;
    while (at < s.size) {
        uint32_t a = s.text[at++], code = 0, minimum = 0;
        unsigned count = 0;
        if (a < 0x80)
            continue;
        if (a >= 0xc2 && a <= 0xdf) {
            code = a & 31;
            count = 1;
            minimum = 0x80;
        } else if (a >= 0xe0 && a <= 0xef) {
            code = a & 15;
            count = 2;
            minimum = 0x800;
        } else if (a >= 0xf0 && a <= 0xf4) {
            code = a & 7;
            count = 3;
            minimum = 0x10000;
        } else
            return 0;
        if (s.size - at < count)
            return 0;
        while (count--) {
            uint32_t b = s.text[at++];
            if ((b & 0xc0) != 0x80)
                return 0;
            code = (code << 6) | (b & 63);
        }
        if (code < minimum || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff))
            return 0;
    }
    return 1;
}
