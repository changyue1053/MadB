// Appended to the actual generated Swift source for private handle-owner regression tests.
import Dispatch
private final class CancellationCounters: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = 0
    private var freed = 0
    private var afterFree = 0
    func cancel() { lock.lock(); if freed > 0 { afterFree += 1 }; cancelled += 1; lock.unlock() }
    func free() { lock.lock(); freed += 1; lock.unlock() }
    var snapshot: (Int, Int, Int) { lock.lock(); defer { lock.unlock() }; return (cancelled, freed, afterFree) }
}
private final class CancellationHolder: @unchecked Sendable {
    var state: UniffiRustFutureCancellation?
}
@main
private struct CancellationRegression {
    static func main() {
        let pre = CancellationCounters()
        let beforeInstall = UniffiRustFutureCancellation(cancelFunc: { _ in pre.cancel() }, freeFunc: { _ in pre.free() })
        beforeInstall.cancel()
        beforeInstall.install(10)
        precondition(!beforeInstall.claimCompletion())
        beforeInstall.finish()
        beforeInstall.cancel()
        precondition(pre.snapshot == (1, 1, 0))

        let completed = CancellationCounters()
        let completionFirst = UniffiRustFutureCancellation(cancelFunc: { _ in completed.cancel() }, freeFunc: { _ in completed.free() })
        completionFirst.install(11)
        precondition(completionFirst.claimCompletion())
        completionFirst.cancel()
        completionFirst.finish()
        completionFirst.cancel()
        precondition(completed.snapshot == (0, 1, 0))

        let synchronous = CancellationCounters()
        let holder = CancellationHolder()
        let reentrant = UniffiRustFutureCancellation(cancelFunc: { _ in
            synchronous.cancel()
            // A native cancellation may resume a waiter that finishes immediately.
            holder.state!.finish()
            precondition(synchronous.snapshot.1 == 0, "free must wait for native cancel to return")
        }, freeFunc: { _ in synchronous.free() })
        holder.state = reentrant
        reentrant.install(12)
        reentrant.cancel()
        reentrant.finish()
        precondition(synchronous.snapshot == (1, 1, 0))

        for number in 0..<1000 {
            let counters = CancellationCounters()
            let state = UniffiRustFutureCancellation(cancelFunc: { _ in counters.cancel() }, freeFunc: { _ in counters.free() })
            state.install(UInt64(100 + number))
            DispatchQueue.concurrentPerform(iterations: 2) { branch in
                if branch == 0 { state.cancel() } else { state.finish() }
            }
            state.cancel()
            let (cancelled, freed, afterFree) = counters.snapshot
            precondition(cancelled <= 1 && freed == 1 && afterFree == 0)
        }
        print("{\"check\":\"swift-future-cancellation-ownership\",\"actual\":\"passed\",\"races\":1000}")
    }
}
