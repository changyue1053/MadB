#pragma once
#define NAPI_CHECK(env,status,tag,msg,return_stmt) do { if ((status)!=napi_ok) { return_stmt; } } while(0)
