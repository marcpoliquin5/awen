#include "awen/Dialect/Device/AwenDeviceDialect.h"
#include "awen/Dialect/Photonic/AwenPhotonicDialect.h"
#include "awen/Dialect/QPhotonic/AwenQPhotonicDialect.h"
#include "awen/Dialect/Tensor/AwenTensorDialect.h"
#include "awen/Transforms/Passes.h"

#include "mlir/Bytecode/BytecodeWriter.h"
#include "mlir/Dialect/Func/IR/FuncOps.h"
#include "mlir/IR/BuiltinOps.h"
#include "mlir/IR/Dialect.h"
#include "mlir/IR/DialectRegistry.h"
#include "mlir/IR/MLIRContext.h"
#include "mlir/Parser/Parser.h"
#include "mlir/Pass/PassManager.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/Support/CommandLine.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/raw_ostream.h"

#include <cstdint>
#include <string>

namespace {

class StablehloImportDialect final : public mlir::Dialect {
public:
  static llvm::StringRef getDialectNamespace() { return "stablehlo"; }

  explicit StablehloImportDialect(mlir::MLIRContext *context)
      : Dialect(getDialectNamespace(), context,
                mlir::TypeID::get<StablehloImportDialect>()) {
    allowUnknownOperations();
  }
};

llvm::cl::opt<std::string>
    inputFilename(llvm::cl::Positional,
                  llvm::cl::desc("<input StableHLO MLIR>"), llvm::cl::Required);
llvm::cl::opt<std::string>
    outputFilename("o", llvm::cl::desc("Output AWEN executable"),
                   llvm::cl::Required);

template <typename T>
void appendLittleEndian(llvm::SmallVectorImpl<char> &output, T value) {
  char bytes[sizeof(T)];
  llvm::support::endian::write<T, llvm::endianness::little,
                               llvm::support::unaligned>(bytes, value);
  output.append(bytes, bytes + sizeof(T));
}

void appendString(llvm::SmallVectorImpl<char> &output, llvm::StringRef value) {
  appendLittleEndian<uint16_t>(output, static_cast<uint16_t>(value.size()));
  output.append(value.begin(), value.end());
}

mlir::DialectRegistry createRegistry() {
  mlir::DialectRegistry registry;
  registry.insert<mlir::func::FuncDialect, StablehloImportDialect,
                  mlir::awen::tensor::AwenTensorDialect,
                  mlir::awen::photonic::AwenPhotonicDialect,
                  mlir::awen::qphotonic::AwenQPhotonicDialect,
                  mlir::awen::device::AwenDeviceDialect>();
  return registry;
}

} // namespace

int main(int argc, char **argv) {
  llvm::cl::ParseCommandLineOptions(argc, argv, "AWEN StableHLO compiler\n");
  mlir::DialectRegistry registry = createRegistry();
  mlir::MLIRContext context(registry);
  auto module = mlir::parseSourceFile<mlir::ModuleOp>(inputFilename, &context);
  if (!module)
    return 1;

  mlir::PassManager manager(&context);
  mlir::awen::buildAwenLoweringPipeline(manager);
  if (mlir::failed(manager.run(*module)))
    return 1;

  llvm::SmallVector<mlir::awen::device::AwenDeviceExecuteGemmOp> commands;
  module->walk([&](mlir::awen::device::AwenDeviceExecuteGemmOp op) {
    commands.push_back(op);
  });
  if (commands.empty()) {
    module->emitError("lowering produced no executable AWEN device commands");
    return 1;
  }
  llvm::StringRef backend = commands.front().getBackend();
  for (auto command : commands) {
    if (command.getBackend() != backend) {
      command.emitError("all commands in AWENEXE v1 must target one backend");
      return 1;
    }
  }

  std::string bytecode;
  llvm::raw_string_ostream bytecodeStream(bytecode);
  mlir::BytecodeWriterConfig bytecodeConfig;
  if (mlir::failed(
          mlir::writeBytecodeToFile(*module, bytecodeStream, bytecodeConfig)))
    return 1;
  bytecodeStream.flush();

  llvm::SmallVector<char> artifact;
  constexpr char magic[] = {'A', 'W', 'E', 'N', 'E', 'X', 'E', '\0'};
  artifact.append(std::begin(magic), std::end(magic));
  appendLittleEndian<uint16_t>(artifact, 1);
  appendLittleEndian<uint16_t>(artifact, 0);
  appendString(artifact, backend);
  appendLittleEndian<uint32_t>(artifact,
                               static_cast<uint32_t>(commands.size()));

  for (auto command : commands) {
    artifact.push_back(1); // ExecuteGemm command kind.
    appendLittleEndian<uint32_t>(artifact,
                                 static_cast<uint32_t>(command.getTileM()));
    appendLittleEndian<uint32_t>(artifact,
                                 static_cast<uint32_t>(command.getTileN()));
    appendLittleEndian<uint32_t>(artifact,
                                 static_cast<uint32_t>(command.getTileK()));
    appendLittleEndian<uint16_t>(
        artifact, static_cast<uint16_t>(command.getMinimumEffectiveBits()));
    appendString(artifact, command.getCalibration());
    appendString(artifact, command.getLayout());

    auto resultType =
        llvm::cast<mlir::RankedTensorType>(command.getResult().getType());
    artifact.push_back(static_cast<char>(resultType.getRank()));
    for (int64_t dimension : resultType.getShape()) {
      const int64_t abiDimension =
          mlir::ShapedType::isDynamic(dimension) ? -1 : dimension;
      appendLittleEndian<int64_t>(artifact, abiDimension);
    }
  }

  appendLittleEndian<uint32_t>(artifact,
                               static_cast<uint32_t>(bytecode.size()));
  artifact.append(bytecode.begin(), bytecode.end());

  std::error_code error;
  llvm::raw_fd_ostream output(outputFilename, error, llvm::sys::fs::OF_None);
  if (error) {
    llvm::errs() << "cannot open output '" << outputFilename
                 << "': " << error.message() << "\n";
    return 1;
  }
  output.write(artifact.data(), artifact.size());
  output.flush();
  llvm::outs() << "wrote " << artifact.size() << " bytes and "
               << commands.size() << " command(s) to " << outputFilename
               << "\n";
  return 0;
}
