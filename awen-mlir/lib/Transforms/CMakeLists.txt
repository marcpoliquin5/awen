add_mlir_library(AwenTransforms
  Passes.cpp
  LINK_LIBS PUBLIC
  AwenTensorDialect
  AwenPhotonicDialect
  AwenDeviceDialect
  MLIRFuncDialect
  MLIRIR
  MLIRPass
  MLIRSupport
  MLIRTransforms)
