#ifndef AWEN_TRANSFORMS_PASSES_H
#define AWEN_TRANSFORMS_PASSES_H

namespace mlir {
class OpPassManager;
}

namespace mlir::awen {

void registerAwenPasses();
void buildAwenLoweringPipeline(OpPassManager &manager);

} // namespace mlir::awen

#endif
