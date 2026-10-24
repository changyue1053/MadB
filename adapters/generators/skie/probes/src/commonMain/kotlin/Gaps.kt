@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
package madb.gaps
import madb.custom.CustomToken
class Box<T>
enum class UnsafeSurface {
    ONLY;
    fun lambdaBox(): Box<() -> Unit> = Box()
    fun acceptCustom(token: CustomToken): String = token.description ?: "empty"
}
