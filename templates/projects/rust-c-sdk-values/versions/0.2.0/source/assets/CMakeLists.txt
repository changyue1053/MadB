cmake_minimum_required(VERSION 4.4.4)
project(@@MADB:name@@ LANGUAGES NONE)
# Every invocation enters the same frozen project coordinator as the CLI.
add_custom_target(madb_build ALL
  COMMAND "${CMAKE_CURRENT_SOURCE_DIR}/madbw" build --frozen
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  VERBATIM USES_TERMINAL)
