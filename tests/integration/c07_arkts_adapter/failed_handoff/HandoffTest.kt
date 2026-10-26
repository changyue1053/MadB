package com.madb.c07

import cnames.structs.madb_kn_lifecycle
import kotlinx.cinterop.CPointer
import com.madb.c07.executor.Fixture
import org.cpf.kotlin.akinterop.generate.*
import org.cpf.kotlin.akinterop.types.*

private class Observer : RuntimeDispatcher, ArkProxy {
    override val ref = Ref()
    val closed = ArrayList<String>()
    override fun onReady() { }
    override fun onValue(handle:String,snapshot:Snapshot) { error("unexpected business callback") }
    override fun onClosed(handle:String,status:Int) { closed.add(handle) }
}
private fun <T> call(argument:Any?=null, block:()->T):T {
    val scope=ManagedOwnedCall.begin()
    try { scope.trackArgument(argument); val result=block(); scope.encodingSucceeded(); return result }
    finally { check(scope.close()) }
}
private fun errorCode(code:String, block:()->Unit) {
    try { block(); error("expected $code") }
    catch (e:ManagedBoundaryFailure) { check(e.code==code) { "${e.code} != $code" } }
}
private fun progress() { tsfnRegister.flush(); OwnedRuntime.progress() }
private fun finishJoin() { Fixture.joined=true; check(OwnedRuntime.afterJoin(OwnedRuntime.metric(28),0,false)==0); progress(); progress() }
private fun awaitCheck(predicate:()->Boolean) { val end=System.nanoTime()+3_000_000_000; while(!predicate()){ check(System.nanoTime()<end); Thread.sleep(1) } }
private fun phase() = ExecutionProbe.frame().split(',')[3]
private fun finalCheck(observer:Observer) {
    progress(); progress()
    check(OwnedRuntime.metric(18)==0uL && OwnedRuntime.metric(19)==0uL)
    check(OwnedRuntime.metric(3)==0uL && OwnedRuntime.metric(27)==0uL && OwnedRuntime.metric(31)==0uL)
    check(OwnedRuntime.metric(0)==0uL && OwnedRuntime.metric(30)==0uL) // not fake env exit
    check(OwnedRuntime.prepareManagedShutdown())
    check(observer.ref.closes==1 && OwnedRuntime.metric(32)==0uL)
}
fun main(args:Array<String>) {
    val scenario=args.single(); val observer=Observer()
    OwnedRuntime.beginEpoch(1uL); OwnedRuntime.lifecycle=CPointer(madb_kn_lifecycle())
    val runtime=call(observer){ownedOpen(observer)}
    val engine=call{ownedConnect(runtime)}
    fun completed():String {
        val op=call{ownedSubmit(Snapshot(41,"abc"),engine)}
        check(OwnedRuntime.step(OwnedRuntime.metric(28))==1); progress(); return op
    }
    when(scenario) {
        "healthy", "upgrade", "repeat", "no-new-wait" -> {
            val op=completed();call{ownedClaimResult(op,"1")}
            call{ownedRuntimeClose(runtime,0)}
            check(OwnedRuntime.metric(18)==1uL && OwnedRuntime.metric(19)==3uL)
            // Healthy close may still register an independent close observation.
            check(call{ownedWatchClose(op,"2")} == -1)
            finishJoin();check(OwnedRuntime.metric(18)==2uL)
            if(scenario=="healthy") {
                check(call{ownedTakeSnapshot(runtime,"1")}.value==42)
                check(call{ownedTakeStatus(runtime,"2")}==OK)
            } else {
                call{ownedRuntimeClose(runtime,1)}
                if(scenario=="no-new-wait") {
                    errorCode("Closing"){call{ownedClaimResult(op,"3")}}
                    errorCode("Closing"){call{ownedWatchClose(op,"3")}}
                }
                check(OwnedRuntime.metric(18)==0uL && OwnedRuntime.metric(19)==0uL)
                if(scenario=="repeat") { call{ownedRuntimeClose(runtime,0)};call{ownedRuntimeClose(runtime,1)} }
            }
            finalCheck(observer)
            check(engine in observer.closed && op in observer.closed)
        }
        "full32" -> {
            val op=completed();call{ownedClaimResult(op,"1")}
            for(i in 2..32) check(call{ownedWatchClose(op,i.toString())} == -1)
            check(OwnedRuntime.metric(18)==32uL)
            call{ownedRuntimeClose(runtime,1)}
            check(OwnedRuntime.metric(18)==0uL && OwnedRuntime.metric(19)==0uL)
            finishJoin(); finalCheck(observer)
        }
        "not-ready" -> {
            ExecutionProbe.set(2,1)
            val op=call{ownedSubmit(Snapshot(41,"abc"),engine)}
            val worker=Thread{OwnedRuntime.step(OwnedRuntime.metric(28))};worker.start()
            awaitCheck{phase()=="2"}
            call{ownedClaimResult(op,"1")};check(call{ownedWatchClose(op,"2")} == -1)
            call{ownedRuntimeClose(runtime,1)};call{ownedRuntimeClose(runtime,0)};progress()
            check(OwnedRuntime.metric(18)==2uL && OwnedRuntime.metric(19)==3uL)
            check(OwnedRuntime.metric(7)==0uL && OwnedRuntime.metric(31)==1uL)
            ExecutionProbe.set(2,0);worker.join(3000);check(!worker.isAlive)
            // afterJoin itself must run the cleanup; do not hide it behind poll.
            Fixture.joined=true;check(OwnedRuntime.afterJoin(OwnedRuntime.metric(28),0,false)==0)
            check(OwnedRuntime.metric(18)==0uL && OwnedRuntime.metric(19)==0uL)
            finalCheck(observer)
        }
        "entered-success", "entered-failure" -> {
            val op=completed();call{ownedClaimResult(op,"1")}
            val scope=ManagedOwnedCall.begin()
            val value=ownedTakeSnapshot(runtime,"1");check(value.value==42)
            call{ownedRuntimeClose(runtime,1)};progress()
            check(OwnedRuntime.metric(18)==1uL && OwnedRuntime.metric(27)==1uL && OwnedRuntime.metric(19)==3uL)
            if(scenario=="entered-success")scope.encodingSucceeded() else check(scope.deliveryFailed)
            check(scope.close())
            check(OwnedRuntime.metric(18)==0uL && OwnedRuntime.metric(27)==0uL && OwnedRuntime.metric(19)==0uL)
            finishJoin();finalCheck(observer)
        }
        "pre-guard" -> {
            val op=completed();call{ownedClaimResult(op,"1")}
            call{ownedTestTransfer(runtime,2)}
            errorCode("BridgeFailure"){call{ownedTakeSnapshot(runtime,"1")}}
            check(OwnedRuntime.metric(18)==1uL && OwnedRuntime.metric(19)==3uL)
            call{ownedRuntimeClose(runtime,1)};finishJoin();finalCheck(observer)
        }
        "invalid-mode", "stale-epoch" -> {
            val op=completed();call{ownedClaimResult(op,"1")}
            if(scenario=="invalid-mode")for(mode in listOf(-1,2,Int.MAX_VALUE))errorCode("InvalidArgument"){call{ownedRuntimeClose(runtime,mode)}}
            else errorCode("InvalidHandle"){call{ownedRuntimeClose("kn:2",1)}}
            check(OwnedRuntime.metric(29)==0uL && OwnedRuntime.metric(18)==1uL)
            check(call{ownedTakeSnapshot(runtime,"1")}.value==42)
            call{ownedRuntimeClose(runtime,0)};finishJoin();finalCheck(observer)
        }
        "abort-stopped", "reject-commit-stopped", "commit-wake", "real-wake-error" -> {
            val sub=call{ownedObserve(engine,0)}
            call{ownedTestPublication(Snapshot(2,"held"),engine,0)}
            val before=Fixture.wakes
            if(scenario.endsWith("stopped")) {
                call{ownedRuntimeClose(runtime,1)}
                if(scenario=="abort-stopped")call{ownedTestReleasePublication(engine,0)}
                else errorCode("Closing"){call{ownedTestReleasePublication(engine,1)}}
                check(Fixture.wakes==before && OwnedRuntime.metric(0)==0uL && OwnedRuntime.metric(19)==0uL)
                finishJoin();finalCheck(observer);check(sub in observer.closed)
            } else {
                if(scenario=="real-wake-error")Fixture.wakeError=5
                call{ownedTestReleasePublication(engine,1)}
                check(Fixture.wakes==before+1)
                check(OwnedRuntime.metric(0)==if(scenario=="real-wake-error")1uL else 0uL)
                if(scenario=="commit-wake") {
                    call{ownedRuntimeClose(runtime,1)};finishJoin();finalCheck(observer)
                }
            }
        }
        else -> error("unknown scenario")
    }
    println("PASS $scenario (actual owner sources; JVM/NAPI/executor fixtures, not target ACK)")
}
