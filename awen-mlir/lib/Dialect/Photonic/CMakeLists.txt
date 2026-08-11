add_mlir_dialect_library(AwenPhotonicDialect
  AwenPhotonicDialect.cpp
  DEPENDS MLIRAwenPhotonicOpsIncGen
  LINK_LIBS PUBLIC MLIRIR MLIRSupport)
