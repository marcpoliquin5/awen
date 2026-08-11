add_llvm_executable(awen-opt awen-opt.cpp)
llvm_update_compile_flags(awen-opt)
target_link_libraries(awen-opt PRIVATE
  AwenTensorDialect
  AwenPhotonicDialect
  AwenQPhotonicDialect
  AwenDeviceDialect
  AwenTransforms
  MLIROptLib
  MLIRParser
  MLIRFuncDialect
  MLIRPass
  MLIRTransforms)
mlir_check_all_link_libraries(awen-opt)
