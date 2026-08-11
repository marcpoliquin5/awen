add_mlir_dialect_library(AwenTensorDialect
  AwenTensorDialect.cpp
  DEPENDS MLIRAwenTensorOpsIncGen
  LINK_LIBS PUBLIC MLIRIR MLIRSupport)
