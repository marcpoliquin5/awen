#include "awen/Dialect/Photonic/AwenPhotonicDialect.h"
#include "awen/Support/VersionedDialect.h"

#include "mlir/IR/Builders.h"
#include "mlir/IR/DialectImplementation.h"
#include "llvm/ADT/TypeSwitch.h"

#include <cmath>

using namespace mlir;
using namespace mlir::awen::photonic;

#include "awen/Dialect/Photonic/AwenPhotonicOpsDialect.cpp.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/Photonic/AwenPhotonicOpsTypes.cpp.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/Photonic/AwenPhotonicOps.cpp.inc"

void AwenPhotonicDialect::initialize() {
  addInterfaces<awen::support::AwenBytecodeDialectInterface>();
  addTypes<
#define GET_TYPEDEF_LIST
#include "awen/Dialect/Photonic/AwenPhotonicOpsTypes.cpp.inc"
      >();
  addOperations<
#define GET_OP_LIST
#include "awen/Dialect/Photonic/AwenPhotonicOps.cpp.inc"
      >();
}

namespace {

bool isSha256Fingerprint(StringRef fingerprint) {
  if (!fingerprint.consume_front("sha256:") || fingerprint.size() != 64)
    return false;
  for (char character : fingerprint)
    if (!((character >= '0' && character <= '9') ||
          (character >= 'a' && character <= 'f')))
      return false;
  return true;
}

LogicalResult verifyTensorContract(Operation *operation, Type input,
                                   Type result) {
  auto inputType = dyn_cast<RankedTensorType>(input);
  auto resultType = dyn_cast<RankedTensorType>(result);
  if (!inputType || !resultType)
    return operation->emitOpError("requires ranked tensor input and result");
  if (inputType != resultType)
    return operation->emitOpError(
        "requires identical input and result tensor types");
  return success();
}

} // namespace

LogicalResult AwenPhotonicGemmTileOp::verify() {
  auto lhsType = dyn_cast<RankedTensorType>(getLhs().getType());
  auto rhsType = dyn_cast<RankedTensorType>(getRhs().getType());
  auto resultType = dyn_cast<RankedTensorType>(getResult().getType());
  if (!lhsType || !rhsType || !resultType || lhsType.getRank() != 2 ||
      rhsType.getRank() != 2 || resultType.getRank() != 2)
    return emitOpError("requires rank-two lhs, rhs, and result tensors");
  if (lhsType.getElementType() != rhsType.getElementType() ||
      lhsType.getElementType() != resultType.getElementType())
    return emitOpError("requires identical lhs, rhs, and result element types");
  if (getTileM() <= 0 || getTileN() <= 0 || getTileK() <= 0)
    return emitOpError("tile dimensions must be positive");
  if (getMinimumEffectiveBits() <= 0)
    return emitOpError("minimum_effective_bits must be positive");
  if (getCalibration().empty())
    return emitOpError("calibration must identify a calibration snapshot");
  if (getLayout() != "row_major" && getLayout() != "column_major")
    return emitOpError("layout must be 'row_major' or 'column_major'");
  return success();
}

LogicalResult AwenPhotonicCalibratedTransformOp::verify() {
  if (failed(verifyTensorContract(this->getOperation(), getInput().getType(),
                                  getResult().getType())))
    return failure();
  if (getTransferModel() != "affine" && getTransferModel() != "matrix" &&
      getTransferModel() != "detector_response")
    return emitOpError(
        "transfer_model is not a closed classical transfer model");
  if (getCalibrationSnapshot().empty())
    return emitOpError("calibration_snapshot must not be empty");
  if (!isSha256Fingerprint(getCalibrationFingerprint()))
    return emitOpError(
        "calibration_fingerprint must be a lowercase sha256 fingerprint");
  if (getOpticalEffectiveBits() <= 0)
    return emitOpError("optical_effective_bits must be positive");
  double maximumResidualError = getMaximumResidualError().convertToDouble();
  if (!std::isfinite(maximumResidualError) || maximumResidualError < 0.0)
    return emitOpError(
        "maximum_residual_error must be finite and non-negative");
  return success();
}

LogicalResult AwenPhotonicModulateOp::verify() {
  if (failed(verifyTensorContract(this->getOperation(), getInput().getType(),
                                  getResult().getType())))
    return failure();
  if (getModulation() != "amplitude" && getModulation() != "phase" &&
      getModulation() != "in_phase_quadrature")
    return emitOpError("modulation is not a closed classical modulation kind");
  double carrierWavelengthNm = getCarrierWavelengthNm().convertToDouble();
  if (!std::isfinite(carrierWavelengthNm) || carrierWavelengthNm <= 0.0)
    return emitOpError("carrier_wavelength_nm must be finite and positive");
  if (getDacBits() <= 0)
    return emitOpError("dac_bits must be positive");
  if (!isSha256Fingerprint(getCalibrationFingerprint()))
    return emitOpError(
        "calibration_fingerprint must be a lowercase sha256 fingerprint");
  return success();
}

LogicalResult AwenPhotonicDetectOp::verify() {
  if (failed(verifyTensorContract(this->getOperation(), getInput().getType(),
                                  getResult().getType())))
    return failure();
  if (getDetection() != "direct" && getDetection() != "homodyne" &&
      getDetection() != "heterodyne")
    return emitOpError("detection is not a closed classical detection kind");
  if (getIntegrationTimeNs() <= 0)
    return emitOpError("integration_time_ns must be positive");
  if (getAdcBits() <= 0)
    return emitOpError("adc_bits must be positive");
  if (!isSha256Fingerprint(getCalibrationFingerprint()))
    return emitOpError(
        "calibration_fingerprint must be a lowercase sha256 fingerprint");
  return success();
}
