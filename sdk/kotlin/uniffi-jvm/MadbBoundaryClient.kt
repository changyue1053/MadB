package madb.boundary

/**
 * Validate the version-matched UniFFI ABI before lowering arguments or allocating
 * an engine. Raw generated constructors do not invoke this upstream check in
 * UniFFI 0.32.2; ordinary SDK consumers enter through this factory.
 */
public fun openEngine(expectedProfile: String): Engine {
    uniffiEnsureInitialized()
    return Engine(expectedProfile)
}
