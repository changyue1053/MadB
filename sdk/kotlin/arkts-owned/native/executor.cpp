#include "executor.h"
#include <pthread.h>
#include <atomic>
#include <new>
#include <cerrno>
#ifdef MADB_KN_TEST_SUPPORT
#include "thread_probe.h"
#endif

struct madb_kn_executor {
    uint64_t generation;
    pthread_t owner{}, worker{};
    pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
    pthread_mutex_t joinMutex = PTHREAD_MUTEX_INITIALIZER;
    pthread_cond_t changed = PTHREAD_COND_INITIALIZER;
    bool started=false, stopped=false, wake=false, joined=false;
    std::atomic<bool> retired{false};
    std::atomic<uint64_t> steps{0}, bodyExited{0}, joinedCount{0}, errors{0};
#ifdef MADB_KN_TEST_SUPPORT
    pthread_cond_t exitChanged = PTHREAD_COND_INITIALIZER;
    pthread_key_t tlsKey{};
    pthread_t cleanupThread{};
    bool tlsKeyCreated=false, bodyActive=false, holdArmed=false, exitHeld=false, cleanupActive=false;
    uint64_t arms=0, holds=0, releases=0, tlsAttached=0, tlsDestructed=0;
    uint64_t joinCalls=0, joinReturned=0, probeErrors=0;
#endif
    explicit madb_kn_executor(uint64_t g):generation(g),owner(pthread_self()){}
};
namespace {
thread_local const madb_kn_executor *currentExecutor=nullptr;
#ifdef MADB_KN_TEST_SUPPORT
void testTlsDestructor(void *opaque) noexcept {
    auto *e=static_cast<madb_kn_executor*>(opaque);
    pthread_mutex_lock(&e->mutex);++e->tlsDestructed;pthread_mutex_unlock(&e->mutex);
}
#endif
void *run(void *opaque) noexcept {
    auto *e=static_cast<madb_kn_executor*>(opaque);currentExecutor=e;
#ifdef MADB_KN_TEST_SUPPORT
    pthread_mutex_lock(&e->mutex);e->bodyActive=true;
    if(pthread_setspecific(e->tlsKey,e)==0)++e->tlsAttached;
    else {++e->probeErrors;++e->errors;e->stopped=true;}
    pthread_mutex_unlock(&e->mutex);
#endif
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
#ifdef MADB_KN_TEST_SUPPORT
    pthread_mutex_lock(&e->mutex);
    if(e->holdArmed){
        e->exitHeld=true;++e->holds;
        while(e->holdArmed){
            if(pthread_cond_wait(&e->exitChanged,&e->mutex)!=0){++e->probeErrors;++e->errors;break;}
        }
        e->exitHeld=false;
    }
    e->bodyActive=false;++e->bodyExited;pthread_mutex_unlock(&e->mutex);
#else
    ++e->bodyExited;
#endif
    // A CName-created Kotlin runtime TLS destructor may still run after return.
    // Only pthread_join's successful completion is the physical exit witness.
    return nullptr;
}
}
extern "C" madb_kn_executor *madb_kn_executor_create(uint64_t generation) {
    if(!generation)return nullptr;
    auto *e=new(std::nothrow)madb_kn_executor(generation);
#ifdef MADB_KN_TEST_SUPPORT
    if(e){
        if(pthread_key_create(&e->tlsKey,testTlsDestructor)!=0){
            pthread_cond_destroy(&e->exitChanged);pthread_cond_destroy(&e->changed);
            pthread_mutex_destroy(&e->joinMutex);pthread_mutex_destroy(&e->mutex);
            delete e;return nullptr;
        }
        e->tlsKeyCreated=true;
    }
#endif
    return e;
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
#ifdef MADB_KN_TEST_SUPPORT
    e->cleanupThread=pthread_self();e->cleanupActive=true;++e->joinCalls;
#endif
    pthread_mutex_unlock(&e->mutex);
    const int result=pthread_join(e->worker,nullptr);
    pthread_mutex_lock(&e->mutex);
#ifdef MADB_KN_TEST_SUPPORT
    e->cleanupActive=false;++e->joinReturned;
#endif
    if(!result){e->joined=true;++e->joinedCount;}else ++e->errors;
    pthread_mutex_unlock(&e->mutex);
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
#ifdef MADB_KN_TEST_SUPPORT
    const int key=e->tlsKeyCreated?pthread_key_delete(e->tlsKey):0;
    const int exitCondition=pthread_cond_destroy(&e->exitChanged);
    if(key||exitCondition){++e->errors;return key?key:exitCondition;}
#endif
    const int condition=pthread_cond_destroy(&e->changed);
    const int join=pthread_mutex_destroy(&e->joinMutex);
    const int state=pthread_mutex_destroy(&e->mutex);
    if(condition||join||state){++e->errors;return condition?condition:join?join:state;}
    delete e;return 0;
}
#ifdef MADB_KN_TEST_SUPPORT
namespace {
void snapshot(madb_kn_executor *e,uint64_t *out) noexcept {
    out[0]=70604;out[1]=0;out[2]=e->generation;
    out[3]=pthread_equal(pthread_self(),e->owner)?1:0;
    out[4]=e->bodyActive;out[5]=e->exitHeld;out[6]=e->cleanupActive;
    const bool simultaneous=e->bodyActive&&e->exitHeld&&e->cleanupActive;
    out[7]=simultaneous?!pthread_equal(e->owner,e->worker):UINT64_MAX;
    out[8]=simultaneous?!pthread_equal(e->owner,e->cleanupThread):UINT64_MAX;
    out[9]=simultaneous?!pthread_equal(e->worker,e->cleanupThread):UINT64_MAX;
    out[10]=e->arms;out[11]=e->holds;out[12]=e->releases;out[13]=e->bodyExited.load();
    out[14]=e->tlsAttached;out[15]=e->tlsDestructed;out[16]=e->joinCalls;
    out[17]=e->joinReturned;out[18]=e->joinedCount.load();out[19]=0;out[20]=e->probeErrors;
}
}
extern "C" int madb_kn_executor_test_command(madb_kn_executor *e,int action,uint64_t *out,int capacity){
    if(!e||e->retired||!pthread_equal(pthread_self(),e->owner))return EPERM;
    if(!out||capacity!=MADB_KN_THREAD_FRAME_SIZE||action<0||action>2)return EINVAL;
    pthread_mutex_lock(&e->mutex);int result=0;
    if(action==1){
        if(!e->started||e->stopped||e->joined||e->arms!=0)result=EBUSY;
        else {e->holdArmed=true;++e->arms;}
    } else if(action==2){
        if(!e->holdArmed)result=EINVAL;
        else {e->holdArmed=false;++e->releases;pthread_cond_signal(&e->exitChanged);}
    }
    if(result==0)snapshot(e,out);
    pthread_mutex_unlock(&e->mutex);return result;
}
extern "C" int madb_kn_executor_test_cleanup_release(madb_kn_executor *e){
    if(!e||e->retired||!pthread_equal(pthread_self(),e->owner))return EPERM;
    pthread_mutex_lock(&e->mutex);
    if(e->holdArmed){e->holdArmed=false;++e->releases;pthread_cond_signal(&e->exitChanged);}
    pthread_mutex_unlock(&e->mutex);return 0;
}
#endif
extern "C" uint64_t madb_kn_executor_metric(madb_kn_executor *e,int index) {
    if(!e)return 0;
    if(index==4){if(e->retired)return 0;pthread_mutex_lock(&e->mutex);const uint64_t started=e->started;pthread_mutex_unlock(&e->mutex);return started;}
    switch(index){case 0:return e->steps.load();case 1:return e->bodyExited.load();case 2:return e->joinedCount.load();case 3:return e->errors.load();default:return 0;}
}
