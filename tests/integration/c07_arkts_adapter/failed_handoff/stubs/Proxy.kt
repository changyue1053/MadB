package org.cpf.kotlin.akinterop.types
class Ref { var closes=0; fun close(){ check(closes==0); closes++ } }
interface ArkProxy { val ref:Ref }
object tsfnRegister {
 val pending=java.util.concurrent.ConcurrentLinkedQueue<()->Unit>()
 fun enqueueOwned(tid:Int,dispose:()->Unit,call:()->Unit) { pending.add { try {call()} finally {dispose()} } }
 fun flush(){while(true){val call=pending.poll()?:return;call()}}
}
