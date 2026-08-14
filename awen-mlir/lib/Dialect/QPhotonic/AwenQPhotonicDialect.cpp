#include "awen/Dialect/QPhotonic/AwenQPhotonicDialect.h"
#include "awen/Support/VersionedDialect.h"

#include "mlir/IR/Builders.h"
#include "mlir/IR/DialectImplementation.h"
#include "llvm/ADT/TypeSwitch.h"

#include <cmath>

using namespace mlir;
using namespace mlir::awen::qphotonic;

#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsDialect.cpp.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsTypes.cpp.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/QPhotonic/AwenQPhotonicOps.cpp.inc"

void AwenQPhotonicDialect::initialize() {
  addInterfaces<awen::support::AwenBytecodeDialectInterface>();
  addTypes<
#define GET_TYPEDEF_LIST
#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsTypes.cpp.inc"
      >();
  addOperations<
#define GET_OP_LIST
#include "awen/Dialect/QPhotonic/AwenQPhotonicOps.cpp.inc"
      >();
}

namespace {

LogicalResult verifyCoherenceCost(Operation *operation, int64_t cost) {
  if (cost < 0)
    return operation->emitOpError("coherence_cost_ns must be non-negative");
  return success();
}

LogicalResult verifyFinite(Operation *operation, double value,
                           StringRef attribute) {
  if (!std::isfinite(value))
    return operation->emitOpError() << attribute << " must be finite";
  return success();
}

LogicalResult verifyMeasurement(Operation *operation, int64_t shots,
                                int64_t seed, double confidenceLevel,
                                double maximumError,
                                StringRef maximumErrorName) {
  if (shots <= 0)
    return operation->emitOpError("shots must be positive");
  if (seed < 0)
    return operation->emitOpError("seed must be non-negative");
  if (!std::isfinite(confidenceLevel) || confidenceLevel <= 0.0 ||
      confidenceLevel > 1.0)
    return operation->emitOpError(
        "confidence_level must be finite and in (0, 1]");
  if (!std::isfinite(maximumError) || maximumError < 0.0)
    return operation->emitOpError()
           << maximumErrorName << " must be finite and non-negative";
  return success();
}

LogicalResult verifyFeedForward(Operation *operation, double scale,
                                double offset, int64_t maximumLatencyNs) {
  if (failed(verifyFinite(operation, scale, "scale")) ||
      failed(verifyFinite(operation, offset, "offset")))
    return failure();
  if (maximumLatencyNs <= 0)
    return operation->emitOpError("maximum_latency_ns must be positive");
  return success();
}

} // namespace

LogicalResult AwenQPhotonicPrepareFockOp::verify() {
  if (getModes() <= 0)
    return emitOpError("modes must be positive");
  if (getCutoff() < 2)
    return emitOpError("cutoff must be at least two");
  if (getCoherenceBudgetNs() <= 0)
    return emitOpError("coherence_budget_ns must be positive");
  if (getSeed() < 0)
    return emitOpError("seed must be non-negative");
  return success();
}

LogicalResult AwenQPhotonicPrepareGaussianOp::verify() {
  if (getModes() <= 0)
    return emitOpError("modes must be positive");
  if (getCoherenceBudgetNs() <= 0)
    return emitOpError("coherence_budget_ns must be positive");
  if (getSeed() < 0)
    return emitOpError("seed must be non-negative");
  return success();
}

LogicalResult AwenQPhotonicBeamSplitterFockOp::verify() {
  if (failed(verifyFinite(this->getOperation(),
                          getThetaRadians().convertToDouble(),
                          "theta_radians")) ||
      failed(verifyFinite(this->getOperation(),
                          getPhiRadians().convertToDouble(), "phi_radians")))
    return failure();
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicBeamSplitterGaussianOp::verify() {
  if (failed(verifyFinite(this->getOperation(),
                          getThetaRadians().convertToDouble(),
                          "theta_radians")) ||
      failed(verifyFinite(this->getOperation(),
                          getPhiRadians().convertToDouble(), "phi_radians")))
    return failure();
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicPhaseShiftFockOp::verify() {
  if (failed(verifyFinite(this->getOperation(), getRadians().convertToDouble(),
                          "radians")))
    return failure();
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicPhaseShiftGaussianOp::verify() {
  if (failed(verifyFinite(this->getOperation(), getRadians().convertToDouble(),
                          "radians")))
    return failure();
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicSqueezeOp::verify() {
  double magnitude = getMagnitude().convertToDouble();
  if (!std::isfinite(magnitude) || magnitude < 0.0)
    return emitOpError("magnitude must be finite and non-negative");
  if (failed(verifyFinite(this->getOperation(),
                          getAngleRadians().convertToDouble(),
                          "angle_radians")))
    return failure();
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicDisplaceOp::verify() {
  if (failed(
          verifyFinite(this->getOperation(), getQ().convertToDouble(), "q")) ||
      failed(verifyFinite(this->getOperation(), getP().convertToDouble(), "p")))
    return failure();
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicControlledXOp::verify() {
  if (getDimension() < 2)
    return emitOpError("dimension must be at least two");
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicFourierOp::verify() {
  return verifyCoherenceCost(this->getOperation(), getCoherenceCostNs());
}

LogicalResult AwenQPhotonicPhotonCountOp::verify() {
  double maximumDistance = getMaximumTotalVariationDistance().convertToDouble();
  if (maximumDistance > 1.0)
    return emitOpError("maximum_total_variation_distance must not exceed one");
  return verifyMeasurement(this->getOperation(), getShots(), getSeed(),
                           getConfidenceLevel().convertToDouble(),
                           maximumDistance, "maximum_total_variation_distance");
}

LogicalResult AwenQPhotonicHomodyneQOp::verify() {
  return verifyMeasurement(this->getOperation(), getShots(), getSeed(),
                           getConfidenceLevel().convertToDouble(),
                           getMaximumMeanError().convertToDouble(),
                           "maximum_mean_error");
}

LogicalResult AwenQPhotonicHomodynePOp::verify() {
  return verifyMeasurement(this->getOperation(), getShots(), getSeed(),
                           getConfidenceLevel().convertToDouble(),
                           getMaximumMeanError().convertToDouble(),
                           "maximum_mean_error");
}

LogicalResult AwenQPhotonicHeterodyneOp::verify() {
  return verifyMeasurement(this->getOperation(), getShots(), getSeed(),
                           getConfidenceLevel().convertToDouble(),
                           getMaximumMeanError().convertToDouble(),
                           "maximum_mean_error");
}

LogicalResult AwenQPhotonicFeedForwardPhaseOp::verify() {
  return verifyFeedForward(this->getOperation(), getScale().convertToDouble(),
                           getOffset().convertToDouble(),
                           getMaximumLatencyNs());
}

LogicalResult AwenQPhotonicFeedForwardDisplacementQOp::verify() {
  return verifyFeedForward(this->getOperation(), getScale().convertToDouble(),
                           getOffset().convertToDouble(),
                           getMaximumLatencyNs());
}

LogicalResult AwenQPhotonicFeedForwardDisplacementPOp::verify() {
  return verifyFeedForward(this->getOperation(), getScale().convertToDouble(),
                           getOffset().convertToDouble(),
                           getMaximumLatencyNs());
}

LogicalResult AwenQPhotonicFeedForwardSqueezingOp::verify() {
  return verifyFeedForward(this->getOperation(), getScale().convertToDouble(),
                           getOffset().convertToDouble(),
                           getMaximumLatencyNs());
}
