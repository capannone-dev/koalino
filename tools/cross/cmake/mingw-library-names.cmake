# Included after ggml's project(): once its directory is processed, give its static libraries
# the libNAME.a names the MinGW-targeting rustc expects.
cmake_language(DEFER DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  CALL set_target_properties ggml ggml-base ggml-cpu PROPERTIES PREFIX "lib")
