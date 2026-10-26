package kotlinx.cinterop
annotation class ExperimentalForeignApi
class ByteVar
class ULongVar
class CPointer<T>(val owner: Any) {
 operator fun set(index:Int,value:Byte) { (owner as Memory).data[index]=value }
 operator fun set(index:Int,value:ULong) { (owner as Memory).longs[index]=value }
 operator fun get(index:Int):ULong = (owner as Memory).longs[index]
 fun readBytes(size:Int):ByteArray=(owner as Memory).data.copyOf(size)
}
typealias COpaquePointer = CPointer<Any>
class Memory(size:Int) { val data=ByteArray(size); val longs=ULongArray(size); var freed=false }
object nativeHeap {
 fun <T> allocArray(size:Int):CPointer<T> = CPointer(Memory(size))
 fun free(pointer:CPointer<*>) { val m=pointer.owner as Memory; check(!m.freed); m.freed=true }
}
fun <T> allocArray(size:Int):CPointer<T> = CPointer(Memory(size))
fun <T> memScoped(block:()->T):T=block()
