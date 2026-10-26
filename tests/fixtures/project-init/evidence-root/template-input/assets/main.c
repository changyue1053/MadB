#include <stdio.h>
#include <string.h>
#include "@@MADB:namespace@@.h"
int main(void) {
    @@MADB:namespace@@_Reading input = {0};
    input.value = 7; input.label.byte_count = 5;
    memcpy(input.label.bytes, "  x  ", 5);
    @@MADB:namespace@@_ReadingResult result = @@MADB:namespace@@_normalize(input);
    if (result.kind != MADB_VALUE_CALL_OK || result.value.value != 7 || result.value.label.byte_count != 1 || result.value.label.bytes[0] != 'x') return 1;
    puts("7:x"); return 0;
}
