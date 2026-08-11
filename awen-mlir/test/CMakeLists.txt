find_package(Python3 REQUIRED COMPONENTS Interpreter)
find_program(AWEN_FILECHECK NAMES FileCheck FileCheck-20
  HINTS "${LLVM_TOOLS_BINARY_DIR}" REQUIRED)

add_test(NAME awen-mlir-conformance
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/run_tests.py"
          --awen-opt "$<TARGET_FILE:awen-opt>"
          --awen-compile "$<TARGET_FILE:awen-compile>"
          --filecheck "${AWEN_FILECHECK}"
          --source-dir "${CMAKE_CURRENT_SOURCE_DIR}"
          --output-dir "${CMAKE_CURRENT_BINARY_DIR}")
