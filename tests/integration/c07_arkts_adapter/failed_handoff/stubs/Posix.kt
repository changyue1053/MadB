package platform.posix
fun usleep(value:UInt) { Thread.sleep((value/1000u).toLong()) }
