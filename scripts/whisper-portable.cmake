# Do not derive the distributed binary's CPU features from the build host.
set(GGML_NATIVE OFF CACHE BOOL "Build portable Whisper CPU kernels" FORCE)
