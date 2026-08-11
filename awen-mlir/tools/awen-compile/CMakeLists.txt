add_llvm_executable(awen-compile awen-compile.cpp)
llvm_update_compile_flags(awen-compile)
target_link_libraries(awen-compile PRIVATE
  AwenTensorDialect
  AwenPhotonicDialect
  AwenQPhotonicDialect
  AwenDeviceDialect
  AwenTransforms
  MLIRBytecodeWriter
  MLIRFuncDialect
  MLIRIR
  MLIRParser
  MLIRPass
  MLIRSupport)
mlir_check_all_link_libraries(awen-compile)
