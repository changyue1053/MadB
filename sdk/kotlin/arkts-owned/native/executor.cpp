#include "executor.h"
#include <pthread.h>
#include <atomic>
#include <new>
#include <cerrno>

struct madb_kn_executor {
    uint64_t generation;
    pthread_t owner{}, worker{};
    pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
    pthread_mutex_t joinMutex = PTHREAD_MUTEX_INITIALIZER;
    pthread_cond_t changed = PTHREAD_COND_INITIALIZER;
    bool started=false, stopped=false, wake=false, joined=false;
    std::atomic<bool> retired{false};
    std::atomic<uint64_t> steps{0}, bodyExited{0}, joinedCount{0}, errors{0};
    explicit madb_kn_executor(uint64_t g):generation(g),owner(pthread_self()){}
};
namespace {
thread_local const madb_kn_executor *currentExecutor=nullptr;
void *run(void *opaque) noexcept {
    auto *e=static_cast<madb_kn_executor*>(opaque);currentExecutor=e;
    for (;;) {
        pthread_mutex_lock(&e->mutex);
        if(e->stopped){pthread_mutex_unlock(&e->mutex);break;}
        // Clear before inspecting Kotlin's queue: a concurrent submission after
        // that inspection cannot lose its wake. No business value is stored here.
        e->wake=false;pthread_mutex_unlock(&e->mutex);
        const int result=madb_kn_business_step(e->generation);
        if(result==1){++e->steps;continue;}
        if(result<0){if(result!=-1)++e->errors;break;}
        if(result!=0){++e->errors;break;}
        pthread_mutex_lock(&e->mutex);
        while(!e->stopped&&!e->wake) {
            if(pthread_cond_wait(&e->changed,&e->mutex)!=0){++e->errors;e->stopped=true;break;}
        }
        pthread_mutex_unlock(&e->mutex);
    }
    ++e->bodyExited;
    // A CName-created Kotlin runtime TLS destructor may still run after return.
    // Only pthread_join's successful completion is the physical exit witness.
    return nullptr;
}
}
extern "C" madb_kn_executor *madb_kn_executor_create(uint64_t generation) {
    if(!generation)return nullptr;
    return new(std::nothrow)madb_kn_executor(generation);
}
extern "C" int madb_kn_executor_start(madb_kn_executor *e) {
    if(!e||e->retired||!pthread_equal(pthread_self(),e->owner))return EINVAL;
    pthread_mutex_lock(&e->mutex);
    if(e->started||e->stopped){pthread_mutex_unlock(&e->mutex);return EINVAL;}
    const int result=pthread_create(&e->worker,nullptr,run,e);
    if(!result)e->started=true;
    pthread_mutex_unlock(&e->mutex);return result;
}
extern "C" int madb_kn_executor_wake(madb_kn_executor *e) {
    if(!e||e->retired)return EINVAL;
    pthread_mutex_lock(&e->mutex);
    const bool stopped=e->stopped;
    if(!stopped)e->wake=true;
    pthread_mutex_unlock(&e->mutex);
    if(!stopped)pthread_cond_signal(&e->changed);
    return stopped?ECANCELED:0;
}
extern "C" int madb_kn_executor_request_stop(madb_kn_executor *e) {
    if(!e||e->retired)return EINVAL;
    pthread_mutex_lock(&e->mutex);e->stopped=true;pthread_mutex_unlock(&e->mutex);
    return pthread_cond_signal(&e->changed);
}
extern "C" int madb_kn_executor_join(madb_kn_executor *e) {
    if(!e||e->retired||currentExecutor==e||pthread_equal(pthread_self(),e->owner))return EDEADLK;
    pthread_mutex_lock(&e->joinMutex);
    pthread_mutex_lock(&e->mutex);
    if(!e->stopped){pthread_mutex_unlock(&e->mutex);pthread_mutex_unlock(&e->joinMutex);return EINVAL;}
    // Creation rollback: no pthread exists, so retire the reservation without
    // incrementing the real-join witness. The lifecycle reserved its job first.
    if(!e->started){e->joined=true;pthread_mutex_unlock(&e->mutex);pthread_mutex_unlock(&e->joinMutex);return 0;}
    if(e->joined){pthread_mutex_unlock(&e->mutex);pthread_mutex_unlock(&e->joinMutex);return 0;}
    pthread_mutex_unlock(&e->mutex);
    const int result=pthread_join(e->worker,nullptr);
    if(!result){pthread_mutex_lock(&e->mutex);e->joined=true;++e->joinedCount;pthread_mutex_unlock(&e->mutex);}
    else ++e->errors;
    pthread_mutex_unlock(&e->joinMutex);return result;
}
extern "C" int madb_kn_executor_destroy(madb_kn_executor *e) {
    if(!e||e->retired)return EINVAL;
    // The owner calls only after cleanup-work completion, with no other API lease.
    if(!pthread_equal(pthread_self(),e->owner))return EINVAL;
    pthread_mutex_lock(&e->mutex);const bool safe=!e->started||e->joined;pthread_mutex_unlock(&e->mutex);
    if(!safe)return EBUSY;
    // A platform destroy error retains this poisoned allocation. No later API
    // may touch a possibly destroyed primitive or pretend successful disposal.
    bool expected=false;if(!e->retired.compare_exchange_strong(expected,true))return EBUSY;
    const int condition=pthread_cond_destroy(&e->changed);
    const int join=pthread_mutex_destroy(&e->joinMutex);
    const int state=pthread_mutex_destroy(&e->mutex);
    if(condition||join||state){++e->errors;return condition?condition:join?join:state;}
    delete e;return 0;
}
extern "C" uint64_t madb_kn_executor_metric(madb_kn_executor *e,int index) {
    if(!e)return 0;
    if(index==4){if(e->retired)return 0;pthread_mutex_lock(&e->mutex);const uint64_t started=e->started;pthread_mutex_unlock(&e->mutex);return started;}
    switch(index){case 0:return e->steps.load();case 1:return e->bodyExited.load();case 2:return e->joinedCount.load();case 3:return e->errors.load();default:return 0;}
}
