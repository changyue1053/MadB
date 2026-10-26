package com.madb.c07.executor
import kotlinx.cinterop.*
import cnames.structs.*
object Fixture { var wakes=0; var wakeError=0; var joined=false }
fun madb_kn_executor_create(gen:ULong):CPointer<madb_kn_executor> = CPointer(madb_kn_executor())
fun madb_kn_executor_destroy(p:CPointer<madb_kn_executor>):Int { check(Fixture.joined);return 0 }
fun madb_kn_executor_start(p:CPointer<madb_kn_executor>):Int { (p.owner as madb_kn_executor).started=true;return 0 }
fun madb_kn_executor_request_stop(p:CPointer<madb_kn_executor>):Int { (p.owner as madb_kn_executor).stopped=true;return 0 }
fun madb_kn_executor_wake(p:CPointer<madb_kn_executor>):Int { Fixture.wakes++; return if ((p.owner as madb_kn_executor).stopped) 125 else Fixture.wakeError }
fun madb_kn_lifecycle_attach(p:CPointer<madb_kn_lifecycle>,e:CPointer<madb_kn_executor>,gen:ULong):Int=0
fun madb_kn_lifecycle_request_join(p:CPointer<madb_kn_lifecycle>):Int=0
fun madb_kn_lifecycle_metric(index:Int):ULong=0uL
fun madb_kn_lifecycle_retire(p:CPointer<madb_kn_lifecycle>):Int=0
fun madb_kn_executor_metric(p:CPointer<madb_kn_executor>,index:Int):ULong = when(index) { 4->1uL; 2->if(Fixture.joined)1uL else 0uL; else->0uL }
fun madb_kn_executor_test_command(p:CPointer<madb_kn_executor>,action:Int,out:CPointer<ULongVar>,cap:Int):Int=0
fun madb_kn_lifecycle_test_command(p:CPointer<madb_kn_lifecycle>,action:Int,out:CPointer<ULongVar>,cap:Int):Int=2
fun madb_kn_provider_napi_bind(env:COpaquePointer?) { }
fun madb_kn_is_main_thread():Int=1
