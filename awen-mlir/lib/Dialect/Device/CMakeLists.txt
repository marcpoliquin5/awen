add_mlir_dialect_library(AwenDeviceDialect
  AwenDeviceDialect.cpp
  DEPENDS MLIRAwenDeviceOpsIncGen
  LINK_LIBS PUBLIC MLIRIR MLIRSupport)
