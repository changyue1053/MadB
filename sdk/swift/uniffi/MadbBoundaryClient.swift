// Compiled into the generated MadbBoundary module. No second lifecycle or ABI checker.
// Use this entry before constructing resources; the raw UniFFI initializer retains
// its upstream fatal-error behavior on incompatible binary/checksum combinations.
public func openEngine(expectedProfile: String) throws -> Engine {
    try madbCheckAbi()
    return try Engine(expectedProfile: expectedProfile)
}
