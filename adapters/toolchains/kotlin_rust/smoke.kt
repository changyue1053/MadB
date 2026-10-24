package madb.producer
import kotlinx.coroutines.runBlocking
import madb.boundary.*
fun main(args: Array<String>) = runBlocking {
    val engine = openEngine(args.single())
    val value = Value("producer", null, "你好", emptyList(), Phase.READY)
    check(engine.roundtrip(value) == value)
    engine.requestClose()
    engine.awaitClosed()
    check(engine.statistics().workerExits == 1uL)
    engine.destroy()
    println("MADB_C06_SMOKE_OK")
}
