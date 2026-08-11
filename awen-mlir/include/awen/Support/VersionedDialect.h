#ifndef AWEN_SUPPORT_VERSIONEDDIALECT_H
#define AWEN_SUPPORT_VERSIONEDDIALECT_H

#include "mlir/Bytecode/BytecodeImplementation.h"
#include "mlir/IR/BuiltinOps.h"

#include <memory>

namespace mlir::awen::support {

struct AwenDialectVersion final : public DialectVersion {
  AwenDialectVersion(uint64_t major, uint64_t minor)
      : major(major), minor(minor) {}

  uint64_t major;
  uint64_t minor;
};

class AwenBytecodeDialectInterface final : public BytecodeDialectInterface {
public:
  explicit AwenBytecodeDialectInterface(Dialect *dialect)
      : BytecodeDialectInterface(dialect) {}

  void writeVersion(DialectBytecodeWriter &writer) const override {
    writer.writeVarInt(1);
    writer.writeVarInt(0);
  }

  std::unique_ptr<DialectVersion>
  readVersion(DialectBytecodeReader &reader) const override {
    uint64_t major = 0;
    uint64_t minor = 0;
    if (failed(reader.readVarInt(major)) || failed(reader.readVarInt(minor)))
      return nullptr;
    return std::make_unique<AwenDialectVersion>(major, minor);
  }

  LogicalResult
  upgradeFromVersion(Operation *topLevelOp,
                     const DialectVersion &opaqueVersion) const override {
    const auto &version =
        static_cast<const AwenDialectVersion &>(opaqueVersion);
    if (version.major != 1) {
      return topLevelOp->emitError()
             << "cannot read " << getDialect()->getNamespace()
             << " bytecode version " << version.major << "." << version.minor
             << "; this compiler supports 1.x";
    }
    return success();
  }
};

} // namespace mlir::awen::support

#endif
