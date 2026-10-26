import MadBObservation

final class MutableReading {
    var value: Int32 = 0
}

@MainActor
func rejectMutableSnapshot() {
    _ = MadBStateModel<MutableReading>()
}
