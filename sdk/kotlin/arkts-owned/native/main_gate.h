#ifndef MADB_KN_MAIN_GATE_H
#define MADB_KN_MAIN_GATE_H
#ifdef __cplusplus
extern "C" {
#endif
/* Pure native check. Invoke before any Kotlin CName, registration or init. */
int madb_kn_is_main_thread(void);
#ifdef __cplusplus
}
#endif
#endif
