add_mlir_dialect_library(AwenQPhotonicDialect
  AwenQPhotonicDialect.cpp
  DEPENDS MLIRAwenQPhotonicOpsIncGen
  LINK_LIBS PUBLIC MLIRIR MLIRSupport)
