#include "main_gate.h"
#include <pthread.h>
#include <unistd.h>
extern "C" int madb_kn_is_main_thread(void) {
#if defined(__linux__)
    const pid_t tid = gettid();
    const pid_t pid = getpid();
    return tid > 0 && pid > 0 && tid == pid;
#elif defined(__APPLE__)
    return pthread_main_np() == 1;
#else
#error "Native main-thread gate is not implemented for this target"
#endif
}
