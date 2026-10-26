import MadBObservation

func rejectUnsynchronizedActor(model: MadBStateModel<Int32>) {
    model.requestStop()
}
