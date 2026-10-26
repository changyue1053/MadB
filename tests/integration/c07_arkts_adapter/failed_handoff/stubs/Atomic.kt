package kotlinx.atomicfu
class AtomicBoolean(initial:Boolean) { @Volatile var value=initial }
class AtomicInt(initial:Int) { private val v=java.util.concurrent.atomic.AtomicInteger(initial); var value:Int get()=v.get(); set(x){v.set(x)}; fun incrementAndGet()=v.incrementAndGet() }
fun atomic(value:Boolean)=AtomicBoolean(value)
fun atomic(value:Int)=AtomicInt(value)
