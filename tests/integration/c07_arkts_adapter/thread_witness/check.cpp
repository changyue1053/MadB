#include "thread_probe.h"
#include "main_gate.h"
#include <uv.h>
#include <pthread.h>
#include <thread>
#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cerrno>
#include <cstring>
struct fake_env {} envObject;
struct fake_hook { napi_async_cleanup_hook callback; void *data; } hook;
static pthread_t mainThread;
static uv_loop_t loop;
static std::thread cleanup;
static uv_work_t *queued=nullptr;
static uv_after_work_cb after=nullptr;
static std::atomic<bool> jobDone{false};
static bool removed=false;
static madb_kn_executor *executor=nullptr;
static uint64_t finalFrame[21]{};
static int checks=0;
#define CHECK(x) do { ++checks; if(!(x)){std::fprintf(stderr,"FAIL:%d:%s\n",__LINE__,#x);std::abort();} } while(0)
extern "C" int madb_kn_is_main_thread(){return pthread_equal(pthread_self(),mainThread);}
extern "C" int madb_kn_business_step(uint64_t){return 0;}
extern "C" napi_status napi_get_uv_event_loop(napi_env,uv_loop_t **out){*out=&loop;return napi_ok;}
extern "C" napi_status napi_add_async_cleanup_hook(napi_env,napi_async_cleanup_hook cb,void *data,napi_async_cleanup_hook_handle *out){hook={cb,data};*out=&hook;return napi_ok;}
extern "C" napi_status napi_remove_async_cleanup_hook(napi_async_cleanup_hook_handle handle){CHECK(handle==&hook);removed=true;return napi_ok;}
extern "C" int uv_queue_work(uv_loop_t *,uv_work_t *work,uv_work_cb cb,uv_after_work_cb done){
 CHECK(!cleanup.joinable());queued=work;after=done;jobDone=false;
 cleanup=std::thread([=]{cb(work);jobDone.store(true);});return 0;
}
int gate(void*){return 0;}
int ready(void*,int){return executor?0:1;}
int joined(void*,uint64_t generation,int result,int){
 CHECK(generation==7&&result==0&&executor);
 CHECK(madb_kn_executor_test_command(executor,0,finalFrame,21)==0);
 CHECK(finalFrame[4]==0&&finalFrame[5]==0&&finalFrame[6]==0);
 CHECK(finalFrame[7]==UINT64_MAX&&finalFrame[8]==UINT64_MAX&&finalFrame[9]==UINT64_MAX);
 CHECK(finalFrame[13]==1&&finalFrame[14]==1&&finalFrame[15]==1);
 CHECK(finalFrame[16]==1&&finalFrame[17]==1&&finalFrame[18]==1&&finalFrame[20]==0);
 CHECK(madb_kn_executor_destroy(executor)==0);executor=nullptr;finalFrame[19]=1;return 0;
}
void finish(){cleanup.join();CHECK(jobDone);auto callback=after;auto work=queued;after=nullptr;queued=nullptr;callback(work,0);}
void waitHeld(madb_kn_lifecycle *owner){
 uint64_t out[21]{};auto deadline=std::chrono::steady_clock::now()+std::chrono::seconds(3);
 for(;;){
  CHECK(madb_kn_lifecycle_test_command(owner,0,out,21)==0);
  if(out[5]&&out[6]){
   CHECK(out[0]==70604&&out[2]==7&&out[3]==1&&out[4]==1);
   CHECK(out[7]==1&&out[8]==1&&out[9]==1);
   CHECK(out[10]==1&&out[11]==1&&out[12]==0&&out[13]==0);
   CHECK(out[14]==1&&out[15]==0&&out[16]==1&&out[17]==0&&out[18]==0);
   CHECK(!jobDone);return;
  }
  CHECK(std::chrono::steady_clock::now()<deadline);std::this_thread::yield();
 }
}
int main(int argc,char **argv){
 mainThread=pthread_self();CHECK(argc==2);
 madb_kn_lifecycle *owner=nullptr;madb_kn_lifecycle_callbacks callbacks{gate,joined,ready};
 CHECK(madb_kn_lifecycle_create(&envObject,&callbacks,nullptr,&owner)==0);
 executor=madb_kn_executor_create(7);CHECK(executor);
 CHECK(madb_kn_lifecycle_attach(owner,executor,7)==0);
 CHECK(madb_kn_executor_start(executor)==0);
 uint64_t out[21]{};
 CHECK(madb_kn_lifecycle_test_command(owner,1,out,21)==0);
 CHECK(madb_kn_lifecycle_test_command(owner,1,out,21)==EBUSY);
 CHECK(madb_kn_lifecycle_test_command(owner,9,out,21)==EINVAL);
 CHECK(madb_kn_lifecycle_test_command(owner,0,out,20)==EINVAL);
 if(std::strcmp(argv[1],"foreign")==0){
  int status=0;std::thread wrong([&]{status=madb_kn_executor_test_command(executor,0,out,21);});wrong.join();CHECK(status==EPERM);
 }
 CHECK(madb_kn_lifecycle_request_join(owner)==0);waitHeld(owner);
 if(std::strcmp(argv[1],"hook-fixture")==0){
  // Explicit NAPI/uv fixture, not evidence of real platform env teardown.
  hook.callback(&hook,hook.data);finish();CHECK(removed);
 } else {
  CHECK(madb_kn_lifecycle_test_command(owner,2,out,21)==0);
  CHECK(madb_kn_lifecycle_test_command(owner,2,out,21)==EINVAL);
  finish();CHECK(!removed);CHECK(madb_kn_lifecycle_retire(owner)==0);CHECK(removed);
 }
 CHECK(finalFrame[12]==1&&finalFrame[19]==1);
 CHECK(madb_kn_lifecycle_metric(MADB_KN_LIFE_JOB_STARTED)==1);
 CHECK(madb_kn_lifecycle_metric(MADB_KN_LIFE_JOB_FINISHED)==1);
 CHECK(madb_kn_lifecycle_metric(MADB_KN_LIFE_CONTEXT_LIVE)==0);
 std::printf("PASS %s checks=%d actual-pthreads/TLS/join; NAPI+uv fixture\n",argv[1],checks);
}
