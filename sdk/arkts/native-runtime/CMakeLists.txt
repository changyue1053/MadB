cmake_minimum_required(VERSION 3.16)
project(madb_arkts_native_protocol LANGUAGES CXX)
find_package(Threads REQUIRED)
add_library(madb_arkts_native_protocol STATIC src/protocol.cpp src/worker.cpp)
target_include_directories(madb_arkts_native_protocol PUBLIC include)
target_compile_features(madb_arkts_native_protocol PUBLIC cxx_std_17)
target_link_libraries(madb_arkts_native_protocol PUBLIC Threads::Threads)
set_target_properties(madb_arkts_native_protocol PROPERTIES
    CXX_EXTENSIONS OFF POSITION_INDEPENDENT_CODE ON)
target_compile_options(madb_arkts_native_protocol PRIVATE -Wall -Wextra -Werror)
if(CMAKE_SYSTEM_NAME STREQUAL "OHOS")
    # Official OHOS driver emits an unused --gcc-toolchain argument.
    # Keep the diagnostic visible without disabling source warning errors.
    target_compile_options(madb_arkts_native_protocol PRIVATE
        -Wno-error=unused-command-line-argument)
endif()
