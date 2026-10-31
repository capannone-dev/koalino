# Included after ggml's project(): once its directory is processed, give its static libraries
# the libNAME.a names the MinGW-targeting rustc expects.
cmake_language(DEFER DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  CALL set_target_properties ggml ggml-base ggml-cpu PROPERTIES PREFIX "lib")
# Only the GPU build has this library, and a deferred call can't test for it unless it runs code.
cmake_language(DEFER DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  CALL cmake_language EVAL CODE
  "if(TARGET ggml-vulkan)\n set_target_properties(ggml-vulkan PROPERTIES PREFIX \"lib\")\nendif()")
