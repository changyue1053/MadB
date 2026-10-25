package madb.boundary

/** Validate the linked ABI before lowering constructor arguments or allocating an engine. */
fun openEngine(expectedProfile: String): Engine {
    uniffiEnsureInitialized()
    return Engine(expectedProfile)
}
