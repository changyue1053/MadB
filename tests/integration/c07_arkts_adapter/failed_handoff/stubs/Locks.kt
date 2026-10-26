package kotlinx.atomicfu.locks
open class SynchronizedObject
inline fun <T> synchronized(lock:SynchronizedObject, block:()->T):T=kotlin.synchronized(lock,block)
