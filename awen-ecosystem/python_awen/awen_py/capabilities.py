"""Versioned AWEN backend capability and live-health contracts.

This module intentionally uses only the Python standard library so framework
frontends and backend plugins can validate discovery data before importing a
native AWEN runtime binding.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import math
import struct
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple


CAPABILITY_VERSION = "awen.device-capability.v1"
HEALTH_VERSION = "awen.backend-health.v1"
RUNTIME_ABI_VERSION = "awen.runtime-backend.v1"
PLUGIN_ABI_VERSION = "awen.backend-plugin.v1"
CALIBRATION_SNAPSHOT_VERSION = "awen.calibration-snapshot.v1"
PHYSICAL_DESIGN_VERSION = "awen.physical-design.v1"


class CapabilityError(ValueError):
    """Raised when a capability or health document is unsafe or incompatible."""


@dataclass(frozen=True)
class MatrixCore:
    m: int
    n: int
    k: int


@dataclass(frozen=True)
class OperationCapability:
    operation: str
    supports_transpose_lhs: bool
    supports_transpose_rhs: bool
    supports_partial_m: bool
    supports_partial_n: bool
    supports_partial_k: bool


@dataclass(frozen=True)
class CalibrationRequirements:
    required: bool
    maximum_age_seconds: int
    temperature_tolerance_c: float
    drift_tolerance: float


@dataclass(frozen=True)
class CalibrationEnvironment:
    temperature_c: float
    laser_power_mw: float


@dataclass(frozen=True)
class CalibrationCell:
    id: str
    row: int
    column: int
    gain: float
    offset: float
    phase_error_radians: float
    insertion_loss_db: float
    uncertainty: float


@dataclass(frozen=True)
class CalibrationSpareCell:
    id: str
    gain: float
    offset: float
    phase_error_radians: float
    insertion_loss_db: float
    uncertainty: float


@dataclass(frozen=True)
class CalibrationChannel:
    id: str
    wavelength_nm: float
    gain: float
    phase_error_radians: float
    insertion_loss_db: float
    uncertainty: float


@dataclass(frozen=True)
class CalibrationProfile:
    snapshot_version: str
    id: str
    fingerprint: str
    parent_id: Optional[str]
    backend_id: str
    topology_fingerprint: str
    measured_at: str
    environment: CalibrationEnvironment
    gain: float
    offset: float
    phase_error_radians: float
    insertion_loss_db: float
    uncertainty: float
    cells: Tuple[CalibrationCell, ...]
    spare_cells: Tuple[CalibrationSpareCell, ...]
    channels: Tuple[CalibrationChannel, ...]


@dataclass(frozen=True)
class AnalogNoise:
    shot_noise_fraction: float
    thermal_noise_fraction: float
    phase_noise_radians: float
    detector_noise_fraction: float


@dataclass(frozen=True)
class PhysicalDesignBinding:
    """Validated identity view over a closed physical-design binding.

    ``canonical_json`` preserves the Rust contract's deterministic field order
    and omission rules. The Python framework layer does not expose layout or
    solver payloads; callers that need the complete public logical document can
    decode this string explicitly.
    """

    contract_version: str
    classification: str
    pdk_name: str
    pdk_version: str
    process_corner_id: str
    topology_name: str
    circuit_models: Tuple[str, ...]
    adapter_kinds: Tuple[str, ...]
    verification_kinds: Tuple[str, ...]
    fingerprint: str
    canonical_json: str

    def document(self) -> Mapping[str, Any]:
        return json.loads(self.canonical_json)


@dataclass(frozen=True)
class NegotiationDiagnostic:
    code: str
    message: str


@dataclass(frozen=True)
class CapabilityNegotiation:
    backend_id: str
    operation: str
    eligible: bool
    diagnostics: Tuple[NegotiationDiagnostic, ...]


@dataclass(frozen=True)
class DeviceCapabilities:
    capability_version: str
    runtime_abi_version: str
    plugin_abi_version: str
    backend_id: str
    matrix_core: MatrixCore
    supported_operations: Tuple[OperationCapability, ...]
    supported_dtypes: Tuple[str, ...]
    supported_wavelengths_nm: Tuple[float, ...]
    modulation_rate_gbaud: float
    coherence_mode: str
    adc_bits: int
    dac_bits: int
    effective_bits: int
    bit_slicing_modes: Tuple[str, ...]
    saturation_mode: str
    input_dynamic_range: Tuple[float, float]
    analog_noise: AnalogNoise
    sample_rate_gsps: float
    reconfiguration_latency_ns: float
    detector_bandwidth_ghz: float
    insertion_loss_budget_db: float
    supports_complex: bool
    simultaneous_channels: int
    accumulation_modes: Tuple[str, ...]
    calibration_requirements: CalibrationRequirements
    calibration_profile: Optional[CalibrationProfile]
    physical_design: PhysicalDesignBinding
    host_bandwidth_gbps: float
    link_bandwidth_gbps: float
    boundary_latency_ns: float
    laser_power_mw: float
    total_power_budget_mw: float
    dac_energy_pj_per_sample: float
    adc_energy_pj_per_sample: float

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "DeviceCapabilities":
        required = {
            "capability_version",
            "runtime_abi_version",
            "plugin_abi_version",
            "backend_id",
            "matrix_core",
            "supported_operations",
            "supported_dtypes",
            "supported_wavelengths_nm",
            "modulation_rate_gbaud",
            "coherence_mode",
            "adc_bits",
            "dac_bits",
            "effective_bits",
            "bit_slicing_modes",
            "saturation_mode",
            "input_dynamic_range",
            "analog_noise",
            "sample_rate_gsps",
            "reconfiguration_latency_ns",
            "detector_bandwidth_ghz",
            "insertion_loss_budget_db",
            "supports_complex",
            "simultaneous_channels",
            "accumulation_modes",
            "calibration_requirements",
            "physical_design",
            "host_bandwidth_gbps",
            "link_bandwidth_gbps",
            "boundary_latency_ns",
            "laser_power_mw",
            "total_power_budget_mw",
            "dac_energy_pj_per_sample",
            "adc_energy_pj_per_sample",
        }
        _strict_keys(value, required, {"calibration_profile"}, "capability")
        matrix = _mapping(value["matrix_core"], "matrix_core")
        _strict_keys(matrix, {"m", "n", "k"}, set(), "matrix_core")
        operations = tuple(
            _operation(_mapping(item, "supported operation"))
            for item in _sequence(value["supported_operations"], "supported_operations")
        )
        requirements_value = _mapping(
            value["calibration_requirements"], "calibration_requirements"
        )
        _strict_keys(
            requirements_value,
            {
                "required",
                "maximum_age_seconds",
                "temperature_tolerance_c",
                "drift_tolerance",
            },
            set(),
            "calibration_requirements",
        )
        profile_value = value.get("calibration_profile")
        profile = None
        if profile_value is not None:
            profile = _calibration_profile(
                _mapping(profile_value, "calibration_profile")
            )
        dynamic_range = _mapping(value["input_dynamic_range"], "input_dynamic_range")
        _strict_keys(
            dynamic_range,
            {"minimum", "maximum"},
            set(),
            "input_dynamic_range",
        )
        analog_noise = _mapping(value["analog_noise"], "analog_noise")
        _strict_keys(
            analog_noise,
            {
                "shot_noise_fraction",
                "thermal_noise_fraction",
                "phase_noise_radians",
                "detector_noise_fraction",
            },
            set(),
            "analog_noise",
        )
        result = cls(
            capability_version=str(value["capability_version"]),
            runtime_abi_version=str(value["runtime_abi_version"]),
            plugin_abi_version=str(value["plugin_abi_version"]),
            backend_id=str(value["backend_id"]),
            matrix_core=MatrixCore(
                m=_integer(matrix["m"], "matrix_core.m"),
                n=_integer(matrix["n"], "matrix_core.n"),
                k=_integer(matrix["k"], "matrix_core.k"),
            ),
            supported_operations=operations,
            supported_dtypes=tuple(
                str(item)
                for item in _sequence(value["supported_dtypes"], "supported_dtypes")
            ),
            supported_wavelengths_nm=tuple(
                _number(item, "wavelength")
                for item in _sequence(
                    value["supported_wavelengths_nm"], "supported_wavelengths_nm"
                )
            ),
            modulation_rate_gbaud=_number(
                value["modulation_rate_gbaud"], "modulation_rate_gbaud"
            ),
            coherence_mode=str(value["coherence_mode"]),
            adc_bits=_integer(value["adc_bits"], "adc_bits"),
            dac_bits=_integer(value["dac_bits"], "dac_bits"),
            effective_bits=_integer(value["effective_bits"], "effective_bits"),
            bit_slicing_modes=tuple(
                str(item)
                for item in _sequence(
                    value["bit_slicing_modes"], "bit_slicing_modes"
                )
            ),
            saturation_mode=str(value["saturation_mode"]),
            input_dynamic_range=(
                _number(dynamic_range["minimum"], "input_dynamic_range.minimum"),
                _number(dynamic_range["maximum"], "input_dynamic_range.maximum"),
            ),
            analog_noise=AnalogNoise(
                shot_noise_fraction=_number(
                    analog_noise["shot_noise_fraction"],
                    "analog_noise.shot_noise_fraction",
                ),
                thermal_noise_fraction=_number(
                    analog_noise["thermal_noise_fraction"],
                    "analog_noise.thermal_noise_fraction",
                ),
                phase_noise_radians=_number(
                    analog_noise["phase_noise_radians"],
                    "analog_noise.phase_noise_radians",
                ),
                detector_noise_fraction=_number(
                    analog_noise["detector_noise_fraction"],
                    "analog_noise.detector_noise_fraction",
                ),
            ),
            sample_rate_gsps=_number(value["sample_rate_gsps"], "sample_rate_gsps"),
            reconfiguration_latency_ns=_number(
                value["reconfiguration_latency_ns"], "reconfiguration_latency_ns"
            ),
            detector_bandwidth_ghz=_number(
                value["detector_bandwidth_ghz"], "detector_bandwidth_ghz"
            ),
            insertion_loss_budget_db=_number(
                value["insertion_loss_budget_db"], "insertion_loss_budget_db"
            ),
            supports_complex=bool(value["supports_complex"]),
            simultaneous_channels=_integer(
                value["simultaneous_channels"], "simultaneous_channels"
            ),
            accumulation_modes=tuple(
                str(item)
                for item in _sequence(
                    value["accumulation_modes"], "accumulation_modes"
                )
            ),
            calibration_requirements=CalibrationRequirements(
                required=bool(requirements_value["required"]),
                maximum_age_seconds=_integer(
                    requirements_value["maximum_age_seconds"],
                    "calibration maximum age",
                ),
                temperature_tolerance_c=_number(
                    requirements_value["temperature_tolerance_c"],
                    "calibration temperature tolerance",
                ),
                drift_tolerance=_number(
                    requirements_value["drift_tolerance"],
                    "calibration drift tolerance",
                ),
            ),
            calibration_profile=profile,
            physical_design=_physical_design(
                _mapping(value["physical_design"], "physical_design")
            ),
            host_bandwidth_gbps=_number(
                value["host_bandwidth_gbps"], "host_bandwidth_gbps"
            ),
            link_bandwidth_gbps=_number(
                value["link_bandwidth_gbps"], "link_bandwidth_gbps"
            ),
            boundary_latency_ns=_number(
                value["boundary_latency_ns"], "boundary_latency_ns"
            ),
            laser_power_mw=_number(value["laser_power_mw"], "laser_power_mw"),
            total_power_budget_mw=_number(
                value["total_power_budget_mw"], "total_power_budget_mw"
            ),
            dac_energy_pj_per_sample=_number(
                value["dac_energy_pj_per_sample"], "dac_energy_pj_per_sample"
            ),
            adc_energy_pj_per_sample=_number(
                value["adc_energy_pj_per_sample"], "adc_energy_pj_per_sample"
            ),
        )
        result.validate()
        return result

    def validate(self) -> None:
        _expect_version(self.capability_version, CAPABILITY_VERSION, "capability")
        _expect_version(self.runtime_abi_version, RUNTIME_ABI_VERSION, "runtime ABI")
        _expect_version(self.plugin_abi_version, PLUGIN_ABI_VERSION, "plugin ABI")
        if not self.backend_id.strip():
            raise CapabilityError("backend_id must not be empty")
        if min(self.matrix_core.m, self.matrix_core.n, self.matrix_core.k) <= 0:
            raise CapabilityError("matrix core dimensions must be positive")
        if not self.supported_operations:
            raise CapabilityError("at least one supported operation is required")
        _unique([operation.operation for operation in self.supported_operations], "operations")
        if any(operation.operation != "gemm" for operation in self.supported_operations):
            raise CapabilityError("unsupported operation identifier")
        _non_empty_unique(self.supported_dtypes, "supported dtypes")
        complex_dtype = any(dtype.startswith("complex") for dtype in self.supported_dtypes)
        if complex_dtype != self.supports_complex:
            raise CapabilityError(
                "supports_complex must agree with the supported dtype list"
            )
        _positive_unique(self.supported_wavelengths_nm, "wavelengths")
        if not 0 < self.simultaneous_channels <= len(self.supported_wavelengths_nm):
            raise CapabilityError("simultaneous channel count is contradictory")
        for name, number in (
            ("modulation rate", self.modulation_rate_gbaud),
            ("sample rate", self.sample_rate_gsps),
            ("detector bandwidth", self.detector_bandwidth_ghz),
            ("host bandwidth", self.host_bandwidth_gbps),
            ("link bandwidth", self.link_bandwidth_gbps),
            ("total power budget", self.total_power_budget_mw),
        ):
            if number <= 0:
                raise CapabilityError("{} must be positive".format(name))
        for name, number in (
            ("reconfiguration latency", self.reconfiguration_latency_ns),
            ("boundary latency", self.boundary_latency_ns),
            ("insertion-loss budget", self.insertion_loss_budget_db),
            ("laser power", self.laser_power_mw),
            ("DAC energy", self.dac_energy_pj_per_sample),
            ("ADC energy", self.adc_energy_pj_per_sample),
        ):
            if number < 0:
                raise CapabilityError("{} must be non-negative".format(name))
        if self.laser_power_mw > self.total_power_budget_mw:
            raise CapabilityError("laser power exceeds total power budget")
        if min(self.adc_bits, self.dac_bits, self.effective_bits) <= 0:
            raise CapabilityError("precision fields must be positive")
        _non_empty_unique(self.bit_slicing_modes, "bit-slicing modes")
        if (
            self.effective_bits > self.adc_bits
            or self.effective_bits > self.dac_bits
        ) and set(self.bit_slicing_modes) == {"none"}:
            raise CapabilityError("effective precision requires bit slicing")
        if self.saturation_mode not in {"clamp", "error"}:
            raise CapabilityError("unsupported saturation mode")
        if self.input_dynamic_range[0] >= self.input_dynamic_range[1]:
            raise CapabilityError("invalid input dynamic range")
        for name, number in (
            ("shot-noise fraction", self.analog_noise.shot_noise_fraction),
            ("thermal-noise fraction", self.analog_noise.thermal_noise_fraction),
            ("phase-noise radians", self.analog_noise.phase_noise_radians),
            ("detector-noise fraction", self.analog_noise.detector_noise_fraction),
        ):
            if number < 0:
                raise CapabilityError("{} must be non-negative".format(name))
        _non_empty_unique(self.accumulation_modes, "accumulation modes")
        if self.calibration_profile is not None:
            profile = self.calibration_profile
            if profile.backend_id != self.backend_id:
                raise CapabilityError("calibration backend does not match capability")
            if profile.snapshot_version != CALIBRATION_SNAPSHOT_VERSION:
                raise CapabilityError("unsupported calibration snapshot version")
            if not profile.id or not profile.fingerprint or profile.gain == 0:
                raise CapabilityError("invalid calibration identity or gain")
            if not _valid_calibration_fingerprint(profile.fingerprint):
                raise CapabilityError("invalid calibration fingerprint")
            if profile.parent_id is not None and (
                not profile.parent_id or profile.parent_id == profile.id
            ):
                raise CapabilityError("invalid calibration parent id")
            if profile.topology_fingerprint != _topology_fingerprint(self):
                raise CapabilityError("calibration topology fingerprint mismatch")
            _timestamp(profile.measured_at, "calibration measured_at")
            if profile.environment.laser_power_mw < 0:
                raise CapabilityError("calibration laser power must be non-negative")
            if profile.environment.laser_power_mw > self.total_power_budget_mw:
                raise CapabilityError("calibration laser power exceeds device budget")
            _validate_transfer(
                profile.gain,
                profile.offset,
                profile.phase_error_radians,
                profile.insertion_loss_db,
                profile.uncertainty,
                "calibration",
            )
            component_ids = []
            coordinates = []
            for cell in profile.cells:
                component_ids.append(cell.id)
                coordinates.append((cell.row, cell.column))
                if not 0 <= cell.row < self.matrix_core.m or not 0 <= cell.column < self.matrix_core.n:
                    raise CapabilityError("calibration cell lies outside matrix topology")
                _validate_transfer(
                    cell.gain,
                    cell.offset,
                    cell.phase_error_radians,
                    cell.insertion_loss_db,
                    cell.uncertainty,
                    "cell calibration",
                )
            for spare in profile.spare_cells:
                component_ids.append(spare.id)
                _validate_transfer(
                    spare.gain,
                    spare.offset,
                    spare.phase_error_radians,
                    spare.insertion_loss_db,
                    spare.uncertainty,
                    "spare-cell calibration",
                )
            _non_empty_unique(component_ids, "calibration cells", allow_empty=True)
            if len(coordinates) != len(set(coordinates)):
                raise CapabilityError("calibration cell coordinates must be unique")
            _non_empty_unique(
                [channel.id for channel in profile.channels],
                "calibration channels",
                allow_empty=True,
            )
            calibrated_wavelengths = [channel.wavelength_nm for channel in profile.channels]
            if len(calibrated_wavelengths) != len(set(calibrated_wavelengths)):
                raise CapabilityError("calibration channel wavelengths must be unique")
            for channel in profile.channels:
                if channel.wavelength_nm not in self.supported_wavelengths_nm:
                    raise CapabilityError("calibration channel is outside device topology")
                _validate_transfer(
                    channel.gain,
                    0.0,
                    channel.phase_error_radians,
                    channel.insertion_loss_db,
                    channel.uncertainty,
                    "channel calibration",
                )

    def operation(self, name: str) -> Optional[OperationCapability]:
        return next(
            (operation for operation in self.supported_operations if operation.operation == name),
            None,
        )


@dataclass(frozen=True)
class BackendHealth:
    health_version: str
    backend_id: str
    observed_at: str
    status: str
    temperature_c: float
    drift: float
    available_channels: int
    disabled_components: Tuple[str, ...]
    unavailable_resources: Tuple[str, ...]
    calibration_profile_id: Optional[str]
    calibration_fingerprint: Optional[str]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "BackendHealth":
        required = {
            "health_version",
            "backend_id",
            "observed_at",
            "status",
            "temperature_c",
            "drift",
            "available_channels",
            "disabled_components",
            "unavailable_resources",
        }
        _strict_keys(
            value,
            required,
            {"calibration_profile_id", "calibration_fingerprint"},
            "health",
        )
        result = cls(
            health_version=str(value["health_version"]),
            backend_id=str(value["backend_id"]),
            observed_at=str(value["observed_at"]),
            status=str(value["status"]),
            temperature_c=_number(value["temperature_c"], "temperature_c"),
            drift=_number(value["drift"], "drift"),
            available_channels=_integer(
                value["available_channels"], "available_channels"
            ),
            disabled_components=tuple(
                str(item)
                for item in _sequence(
                    value["disabled_components"], "disabled_components"
                )
            ),
            unavailable_resources=tuple(
                str(item)
                for item in _sequence(
                    value["unavailable_resources"], "unavailable_resources"
                )
            ),
            calibration_profile_id=(
                str(value["calibration_profile_id"])
                if value.get("calibration_profile_id") is not None
                else None
            ),
            calibration_fingerprint=(
                str(value["calibration_fingerprint"])
                if value.get("calibration_fingerprint") is not None
                else None
            ),
        )
        result.validate()
        return result

    def validate(self) -> None:
        _expect_version(self.health_version, HEALTH_VERSION, "health")
        if not self.backend_id:
            raise CapabilityError("health backend_id must not be empty")
        _timestamp(self.observed_at, "health observed_at")
        if self.status not in {"healthy", "degraded", "unavailable"}:
            raise CapabilityError("invalid health status")
        if self.drift < 0 or self.available_channels < 0:
            raise CapabilityError("health drift and channel count must be non-negative")
        _non_empty_unique(self.disabled_components, "disabled components", allow_empty=True)
        _non_empty_unique(
            self.unavailable_resources, "unavailable resources", allow_empty=True
        )
        if self.calibration_profile_id == "" or self.calibration_fingerprint == "":
            raise CapabilityError("calibration health identifiers must not be empty")
        if self.calibration_fingerprint is not None and not _valid_calibration_fingerprint(
            self.calibration_fingerprint
        ):
            raise CapabilityError("invalid health calibration fingerprint")
        if (self.calibration_profile_id is None) != (
            self.calibration_fingerprint is None
        ):
            raise CapabilityError(
                "calibration profile id and fingerprint must be provided together"
            )


@dataclass(frozen=True)
class BackendSnapshot:
    capabilities: DeviceCapabilities
    health: BackendHealth

    def validate(self) -> None:
        self.capabilities.validate()
        self.health.validate()
        if self.health.backend_id != self.capabilities.backend_id:
            raise CapabilityError("health backend does not match capability")
        if self.health.available_channels > self.capabilities.simultaneous_channels:
            raise CapabilityError("health channel count exceeds capability")
        profile = self.capabilities.calibration_profile
        if profile is not None:
            disabled_channels = sum(
                channel.id in self.health.disabled_components
                for channel in profile.channels
            )
            if self.health.available_channels > (
                self.capabilities.simultaneous_channels - disabled_channels
            ):
                raise CapabilityError(
                    "health channel count contradicts disabled calibrated channels"
                )

    def negotiate_gemm(
        self,
        shape: Tuple[int, int, int],
        dtype: str,
        minimum_effective_bits: Optional[int] = None,
        transpose_lhs: bool = False,
        transpose_rhs: bool = False,
    ) -> CapabilityNegotiation:
        self.validate()
        diagnostics: List[NegotiationDiagnostic] = []

        def reject(code: str, message: str) -> None:
            diagnostics.append(NegotiationDiagnostic(code, message))

        if self.health.status == "unavailable":
            reject("backend_unavailable", "backend health status is unavailable")
        if self.health.available_channels == 0:
            reject("no_channels", "backend has no available wavelength channels")
        if "matrix_core" in self.health.unavailable_resources:
            reject("matrix_core_unavailable", "the matrix core is unavailable")
        profile = self.capabilities.calibration_profile
        if profile is not None:
            disabled_cells = sum(
                cell.id in self.health.disabled_components for cell in profile.cells
            )
            healthy_spares = sum(
                spare.id not in self.health.disabled_components
                for spare in profile.spare_cells
            )
            if disabled_cells > healthy_spares:
                reject(
                    "calibration_remap_capacity_exhausted",
                    "disabled calibrated cells exceed healthy spare capacity",
                )
        operation = self.capabilities.operation("gemm")
        if operation is None:
            reject("operation_unsupported", "backend does not advertise GEMM")
        else:
            if transpose_lhs and not operation.supports_transpose_lhs:
                reject("transpose_lhs_unsupported", "left transpose is unsupported")
            if transpose_rhs and not operation.supports_transpose_rhs:
                reject("transpose_rhs_unsupported", "right transpose is unsupported")
            for size, tile, supported, code in (
                (shape[0], self.capabilities.matrix_core.m, operation.supports_partial_m, "partial_m_unsupported"),
                (shape[1], self.capabilities.matrix_core.n, operation.supports_partial_n, "partial_n_unsupported"),
                (shape[2], self.capabilities.matrix_core.k, operation.supports_partial_k, "partial_k_unsupported"),
            ):
                if size % tile and not supported:
                    reject(code, "required partial tile is unsupported")
        if dtype not in self.capabilities.supported_dtypes:
            reject("dtype_unsupported", "dtype {} is unsupported".format(dtype))
        if minimum_effective_bits is not None and minimum_effective_bits > self.capabilities.effective_bits:
            reject("precision_insufficient", "effective precision is insufficient")
        self._check_calibration(reject)
        return CapabilityNegotiation(
            backend_id=self.capabilities.backend_id,
            operation="gemm",
            eligible=not diagnostics,
            diagnostics=tuple(diagnostics),
        )

    def _check_calibration(self, reject: Any) -> None:
        requirements = self.capabilities.calibration_requirements
        if not requirements.required:
            return
        profile = self.capabilities.calibration_profile
        if profile is None:
            reject("calibration_missing", "required calibration profile is missing")
            return
        if self.health.calibration_profile_id != profile.id:
            reject("calibration_mismatch", "health does not confirm calibration profile")
        if self.health.calibration_fingerprint != profile.fingerprint:
            reject(
                "calibration_fingerprint_mismatch",
                "health does not confirm exact calibration fingerprint",
            )
        measured = _timestamp(profile.measured_at, "calibration measured_at")
        observed = _timestamp(self.health.observed_at, "health observed_at")
        age = int((observed - measured).total_seconds())
        if age < 0:
            reject("calibration_from_future", "calibration is later than health snapshot")
        elif age > requirements.maximum_age_seconds:
            reject("calibration_expired", "calibration profile has expired")
        if abs(self.health.temperature_c - profile.environment.temperature_c) > requirements.temperature_tolerance_c:
            reject("temperature_out_of_range", "temperature is outside calibration tolerance")
        if self.health.drift > requirements.drift_tolerance:
            reject("drift_out_of_range", "drift exceeds calibration tolerance")


def _operation(value: Mapping[str, Any]) -> OperationCapability:
    fields = {
        "operation",
        "supports_transpose_lhs",
        "supports_transpose_rhs",
        "supports_partial_m",
        "supports_partial_n",
        "supports_partial_k",
    }
    _strict_keys(value, fields, set(), "supported operation")
    return OperationCapability(
        operation=str(value["operation"]),
        supports_transpose_lhs=bool(value["supports_transpose_lhs"]),
        supports_transpose_rhs=bool(value["supports_transpose_rhs"]),
        supports_partial_m=bool(value["supports_partial_m"]),
        supports_partial_n=bool(value["supports_partial_n"]),
        supports_partial_k=bool(value["supports_partial_k"]),
    )


def _calibration_profile(value: Mapping[str, Any]) -> CalibrationProfile:
    fields = {
        "snapshot_version",
        "id",
        "fingerprint",
        "backend_id",
        "topology_fingerprint",
        "measured_at",
        "environment",
        "gain",
        "offset",
        "phase_error_radians",
        "insertion_loss_db",
        "uncertainty",
        "cells",
        "spare_cells",
        "channels",
    }
    _strict_keys(value, fields, {"parent_id"}, "calibration_profile")
    environment = _mapping(value["environment"], "calibration environment")
    _strict_keys(
        environment,
        {"temperature_c", "laser_power_mw"},
        set(),
        "calibration environment",
    )
    return CalibrationProfile(
        snapshot_version=str(value["snapshot_version"]),
        id=str(value["id"]),
        fingerprint=str(value["fingerprint"]),
        parent_id=(
            str(value["parent_id"]) if value.get("parent_id") is not None else None
        ),
        backend_id=str(value["backend_id"]),
        topology_fingerprint=str(value["topology_fingerprint"]),
        measured_at=str(value["measured_at"]),
        environment=CalibrationEnvironment(
            temperature_c=_number(
                environment["temperature_c"],
                "calibration environment temperature_c",
            ),
            laser_power_mw=_number(
                environment["laser_power_mw"],
                "calibration environment laser_power_mw",
            ),
        ),
        gain=_number(value["gain"], "gain"),
        offset=_number(value["offset"], "offset"),
        phase_error_radians=_number(
            value["phase_error_radians"], "phase_error_radians"
        ),
        insertion_loss_db=_number(
            value["insertion_loss_db"], "insertion_loss_db"
        ),
        uncertainty=_number(value["uncertainty"], "uncertainty"),
        cells=tuple(
            _calibration_cell(_mapping(item, "calibration cell"))
            for item in _sequence(value["cells"], "calibration cells")
        ),
        spare_cells=tuple(
            _calibration_spare_cell(_mapping(item, "calibration spare cell"))
            for item in _sequence(value["spare_cells"], "calibration spare cells")
        ),
        channels=tuple(
            _calibration_channel(_mapping(item, "calibration channel"))
            for item in _sequence(value["channels"], "calibration channels")
        ),
    )


def _calibration_cell(value: Mapping[str, Any]) -> CalibrationCell:
    fields = {
        "id",
        "row",
        "column",
        "gain",
        "offset",
        "phase_error_radians",
        "insertion_loss_db",
        "uncertainty",
    }
    _strict_keys(value, fields, set(), "calibration cell")
    return CalibrationCell(
        id=str(value["id"]),
        row=_integer(value["row"], "calibration cell row"),
        column=_integer(value["column"], "calibration cell column"),
        gain=_number(value["gain"], "calibration cell gain"),
        offset=_number(value["offset"], "calibration cell offset"),
        phase_error_radians=_number(
            value["phase_error_radians"], "calibration cell phase error"
        ),
        insertion_loss_db=_number(
            value["insertion_loss_db"], "calibration cell insertion loss"
        ),
        uncertainty=_number(value["uncertainty"], "calibration cell uncertainty"),
    )


def _calibration_spare_cell(value: Mapping[str, Any]) -> CalibrationSpareCell:
    fields = {
        "id",
        "gain",
        "offset",
        "phase_error_radians",
        "insertion_loss_db",
        "uncertainty",
    }
    _strict_keys(value, fields, set(), "calibration spare cell")
    return CalibrationSpareCell(
        id=str(value["id"]),
        gain=_number(value["gain"], "calibration spare gain"),
        offset=_number(value["offset"], "calibration spare offset"),
        phase_error_radians=_number(
            value["phase_error_radians"], "calibration spare phase error"
        ),
        insertion_loss_db=_number(
            value["insertion_loss_db"], "calibration spare insertion loss"
        ),
        uncertainty=_number(value["uncertainty"], "calibration spare uncertainty"),
    )


def _calibration_channel(value: Mapping[str, Any]) -> CalibrationChannel:
    fields = {
        "id",
        "wavelength_nm",
        "gain",
        "phase_error_radians",
        "insertion_loss_db",
        "uncertainty",
    }
    _strict_keys(value, fields, set(), "calibration channel")
    return CalibrationChannel(
        id=str(value["id"]),
        wavelength_nm=_number(
            value["wavelength_nm"], "calibration channel wavelength"
        ),
        gain=_number(value["gain"], "calibration channel gain"),
        phase_error_radians=_number(
            value["phase_error_radians"], "calibration channel phase error"
        ),
        insertion_loss_db=_number(
            value["insertion_loss_db"], "calibration channel insertion loss"
        ),
        uncertainty=_number(value["uncertainty"], "calibration channel uncertainty"),
    )


def _physical_design(value: Mapping[str, Any]) -> PhysicalDesignBinding:
    fields = {
        "contract_version",
        "classification",
        "pdk",
        "process_corner",
        "component_library",
        "topology_artifact",
        "topology",
        "layout_constraints",
        "circuit_models",
        "adapters",
        "verification",
    }
    _strict_keys(value, fields, set(), "physical_design")
    contract_version = _text(value["contract_version"], "physical_design version")
    _expect_version(contract_version, PHYSICAL_DESIGN_VERSION, "physical design")
    classification = _text(value["classification"], "physical_design classification")
    if classification not in {"open_reference", "proprietary_reference"}:
        raise CapabilityError("unsupported physical_design classification")
    proprietary = classification == "proprietary_reference"

    pdk = _mapping(value["pdk"], "physical_design pdk")
    _strict_keys(pdk, {"name", "version", "manifest"}, set(), "physical_design pdk")
    normalized_pdk = {
        "name": _text(pdk["name"], "physical_design pdk name"),
        "version": _text(pdk["version"], "physical_design pdk version"),
        "manifest": _physical_artifact(
            _mapping(pdk["manifest"], "physical_design pdk manifest"),
            "physical_design pdk manifest",
            proprietary,
        ),
    }

    corner = _mapping(value["process_corner"], "physical_design process corner")
    _strict_keys(
        corner,
        {"corner_id", "fingerprint", "temperature_c", "parameters"},
        set(),
        "physical_design process corner",
    )
    parameters = _physical_parameters(
        _mapping(corner["parameters"], "physical_design process parameters"),
        "physical_design process parameters",
    )
    normalized_corner = {
        "corner_id": _text(corner["corner_id"], "physical_design process corner id"),
        "fingerprint": _sha256_text(
            corner["fingerprint"], "physical_design process corner fingerprint"
        ),
        "temperature_c": _number(
            corner["temperature_c"], "physical_design process temperature"
        ),
        "parameters": parameters,
    }
    component_library = _physical_artifact(
        _mapping(value["component_library"], "physical_design component library"),
        "physical_design component library",
        proprietary,
    )
    topology_artifact = _physical_artifact(
        _mapping(value["topology_artifact"], "physical_design topology artifact"),
        "physical_design topology artifact",
        proprietary,
    )
    topology = _physical_topology(
        _mapping(value["topology"], "physical_design topology")
    )
    topology_json = _canonical_json(topology)
    topology_digest = "sha256:" + hashlib.sha256(topology_json.encode("utf-8")).hexdigest()
    if topology_artifact["digest"] != topology_digest:
        raise CapabilityError("physical_design topology artifact digest mismatch")
    constraints = _physical_constraints(
        _mapping(value["layout_constraints"], "physical_design constraints")
    )

    models = tuple(
        _physical_model(
            _mapping(item, "physical_design circuit model"), proprietary
        )
        for item in _sequence(value["circuit_models"], "physical_design circuit models")
    )
    if not models:
        raise CapabilityError("physical_design requires a circuit model")
    _non_empty_unique(
        tuple(model["name"] for model in models), "physical_design circuit models"
    )
    adapters = tuple(
        _physical_adapter(_mapping(item, "physical_design adapter"))
        for item in _sequence(value["adapters"], "physical_design adapters")
    )
    if not adapters:
        raise CapabilityError("physical_design requires an adapter")
    adapter_kinds = tuple(adapter["kind"] for adapter in adapters)
    _non_empty_unique(adapter_kinds, "physical_design adapter kinds")
    if "gdsfactory" not in adapter_kinds:
        raise CapabilityError("physical_design requires a gdsfactory adapter")
    if any(model["framework"] == "circulax" for model in models) and (
        "circuit_simulator" not in adapter_kinds
    ):
        raise CapabilityError("Circulax model requires a circuit-simulator adapter")

    evidence = tuple(
        _physical_evidence(
            _mapping(item, "physical_design verification"), proprietary
        )
        for item in _sequence(value["verification"], "physical_design verification")
    )
    if not evidence or not any(item["kind"] == "connectivity" for item in evidence):
        raise CapabilityError("physical_design requires passed connectivity evidence")
    supported_evidence = {
        kind for adapter in adapters for kind in adapter["supported_evidence"]
    }
    if any(item["kind"] not in supported_evidence for item in evidence):
        raise CapabilityError("physical_design verification is not supported by an adapter")

    if proprietary:
        if parameters or any(node["settings"] for node in topology["nodes"]):
            raise CapabilityError("proprietary physical_design parameters must not be embedded")
        if topology["nodes"] or topology["connections"]:
            raise CapabilityError("proprietary topology internals must not be embedded")
        if any(model["parameters"] for model in models):
            raise CapabilityError("proprietary circuit parameters must not be embedded")

    normalized = {
        "contract_version": contract_version,
        "classification": classification,
        "pdk": normalized_pdk,
        "process_corner": normalized_corner,
        "component_library": component_library,
        "topology_artifact": topology_artifact,
        "topology": topology,
        "layout_constraints": constraints,
        "circuit_models": list(models),
        "adapters": list(adapters),
        "verification": list(evidence),
    }
    canonical_json = _canonical_json(normalized)
    return PhysicalDesignBinding(
        contract_version=contract_version,
        classification=classification,
        pdk_name=normalized_pdk["name"],
        pdk_version=normalized_pdk["version"],
        process_corner_id=normalized_corner["corner_id"],
        topology_name=topology["name"],
        circuit_models=tuple(model["name"] for model in models),
        adapter_kinds=adapter_kinds,
        verification_kinds=tuple(item["kind"] for item in evidence),
        fingerprint="sha256:" + hashlib.sha256(canonical_json.encode("utf-8")).hexdigest(),
        canonical_json=canonical_json,
    )


def _physical_artifact(
    value: Mapping[str, Any], name: str, proprietary: bool
) -> Dict[str, Any]:
    _strict_keys(
        value,
        {"artifact_id", "digest", "media_type"},
        {"uri"},
        name,
    )
    artifact_id = _text(value["artifact_id"], name + " artifact_id")
    if artifact_id.startswith("sha256:"):
        _sha256_text(artifact_id, name + " artifact_id")
    elif not artifact_id.startswith("urn:") or artifact_id == "urn:":
        raise CapabilityError(name + " artifact_id must be an immutable urn or sha256 identity")
    normalized = {
        "artifact_id": artifact_id,
        "digest": _sha256_text(value["digest"], name + " digest"),
        "media_type": _text(value["media_type"], name + " media_type"),
    }
    if value.get("uri") is not None:
        uri = _text(value["uri"], name + " uri")
        if not (
            (uri.startswith("https://") and len(uri) > 8)
            or (uri.startswith("urn:") and len(uri) > 4)
        ):
            raise CapabilityError(name + " uri must use https or urn")
        if proprietary:
            raise CapabilityError("proprietary physical_design references must not expose URIs")
        normalized["uri"] = uri
    return normalized


def _physical_parameters(value: Mapping[str, Any], name: str) -> Dict[str, float]:
    normalized: Dict[str, float] = {}
    for key in sorted(value):
        normalized[_text(key, name + " key")] = _number(value[key], name + " value")
    return normalized


def _physical_wavelength(value: Mapping[str, Any], name: str) -> Dict[str, float]:
    _strict_keys(value, {"minimum_nm", "maximum_nm"}, set(), name)
    minimum = _number(value["minimum_nm"], name + " minimum")
    maximum = _number(value["maximum_nm"], name + " maximum")
    if minimum <= 0 or maximum <= 0 or minimum > maximum:
        raise CapabilityError(name + " is invalid")
    return {"minimum_nm": minimum, "maximum_nm": maximum}


def _physical_port(value: Mapping[str, Any], name: str) -> Dict[str, Any]:
    required = {"name", "kind", "center", "orientation_degrees", "width", "unit", "layer"}
    _strict_keys(value, required, {"wavelength", "mode"}, name)
    kind = _text(value["kind"], name + " kind")
    if kind not in {"optical", "electrical", "placement"}:
        raise CapabilityError(name + " has an unsupported kind")
    center = _sequence(value["center"], name + " center")
    if len(center) != 2:
        raise CapabilityError(name + " center must have two coordinates")
    orientation = _number(value["orientation_degrees"], name + " orientation")
    width = _number(value["width"], name + " width")
    unit = _text(value["unit"], name + " unit")
    if not 0 <= orientation < 360 or width <= 0:
        raise CapabilityError(name + " has invalid orientation or width")
    if unit not in {"nanometer", "micrometer", "meter"}:
        raise CapabilityError(name + " has an unsupported unit")
    normalized: Dict[str, Any] = {
        "name": _text(value["name"], name + " name"),
        "kind": kind,
        "center": [_number(center[0], name + " center x"), _number(center[1], name + " center y")],
        "orientation_degrees": orientation,
        "width": width,
        "unit": unit,
        "layer": _text(value["layer"], name + " layer"),
    }
    if value.get("wavelength") is not None:
        normalized["wavelength"] = _physical_wavelength(
            _mapping(value["wavelength"], name + " wavelength"), name + " wavelength"
        )
    elif kind == "optical":
        raise CapabilityError(name + " optical port requires a wavelength")
    if value.get("mode") is not None:
        normalized["mode"] = _text(value["mode"], name + " mode")
    return normalized


def _physical_topology(value: Mapping[str, Any]) -> Dict[str, Any]:
    _strict_keys(value, {"name", "external_ports", "nodes", "connections"}, set(), "physical_design topology")
    external_ports = [
        _physical_port(_mapping(item, "physical_design external port"), "physical_design external port")
        for item in _sequence(value["external_ports"], "physical_design external ports")
    ]
    _non_empty_unique(tuple(port["name"] for port in external_ports), "physical_design external ports")
    nodes = []
    for item in _sequence(value["nodes"], "physical_design topology nodes"):
        node = _mapping(item, "physical_design topology node")
        _strict_keys(node, {"instance_id", "component", "ports", "settings"}, set(), "physical_design topology node")
        ports = [
            _physical_port(_mapping(port, "physical_design node port"), "physical_design node port")
            for port in _sequence(node["ports"], "physical_design node ports")
        ]
        _non_empty_unique(tuple(port["name"] for port in ports), "physical_design node ports")
        nodes.append(
            {
                "instance_id": _text(node["instance_id"], "physical_design node id"),
                "component": _text(node["component"], "physical_design component"),
                "ports": ports,
                "settings": _physical_parameters(
                    _mapping(node["settings"], "physical_design node settings"),
                    "physical_design node settings",
                ),
            }
        )
    _non_empty_unique(
        tuple(node["instance_id"] for node in nodes),
        "physical_design node ids",
        allow_empty=True,
    )
    connections = []
    for item in _sequence(value["connections"], "physical_design connections"):
        connection = _mapping(item, "physical_design connection")
        _strict_keys(connection, {"source", "destination"}, set(), "physical_design connection")
        connections.append(
            {
                "source": _physical_endpoint(_mapping(connection["source"], "physical_design source")),
                "destination": _physical_endpoint(_mapping(connection["destination"], "physical_design destination")),
            }
        )
    normalized = {
        "name": _text(value["name"], "physical_design topology name"),
        "external_ports": external_ports,
        "nodes": nodes,
        "connections": connections,
    }
    _validate_physical_connections(normalized)
    return normalized


def _physical_endpoint(value: Mapping[str, Any]) -> Dict[str, str]:
    _strict_keys(value, {"port_name"}, {"instance_id"}, "physical_design endpoint")
    normalized = {}
    if value.get("instance_id") is not None:
        normalized["instance_id"] = _text(value["instance_id"], "physical_design endpoint node")
    normalized["port_name"] = _text(value["port_name"], "physical_design endpoint port")
    return normalized


def _validate_physical_connections(topology: Mapping[str, Any]) -> None:
    external = {port["name"] for port in topology["external_ports"]}
    nodes = {node["instance_id"]: {port["name"] for port in node["ports"]} for node in topology["nodes"]}
    seen = set()
    for connection in topology["connections"]:
        pair = []
        for endpoint in (connection["source"], connection["destination"]):
            instance = endpoint.get("instance_id")
            ports = external if instance is None else nodes.get(instance)
            if ports is None or endpoint["port_name"] not in ports:
                raise CapabilityError("physical_design connection references an unknown port")
            pair.append((instance, endpoint["port_name"]))
        if pair[0] == pair[1] or tuple(pair) in seen:
            raise CapabilityError("physical_design connections must be unique and non-reflexive")
        seen.add(tuple(pair))


def _physical_constraints(value: Mapping[str, Any]) -> Dict[str, Any]:
    required = {"unit", "maximum_crossings", "allowed_layers"}
    optional = {"maximum_width", "maximum_height", "minimum_bend_radius", "maximum_path_length_imbalance"}
    _strict_keys(value, required, optional, "physical_design constraints")
    unit = _text(value["unit"], "physical_design constraint unit")
    if unit not in {"nanometer", "micrometer", "meter"}:
        raise CapabilityError("physical_design constraint unit is unsupported")
    normalized: Dict[str, Any] = {"unit": unit}
    for field in ("maximum_width", "maximum_height", "minimum_bend_radius", "maximum_path_length_imbalance"):
        if value.get(field) is not None:
            number = _number(value[field], "physical_design " + field)
            if number <= 0:
                raise CapabilityError("physical_design constraints must be positive")
            normalized[field] = number
    crossings = _integer(value["maximum_crossings"], "physical_design maximum crossings")
    if crossings < 0:
        raise CapabilityError("physical_design maximum crossings must be non-negative")
    layers = tuple(
        _text(item, "physical_design allowed layer")
        for item in _sequence(value["allowed_layers"], "physical_design allowed layers")
    )
    _non_empty_unique(layers, "physical_design allowed layers")
    normalized["maximum_crossings"] = crossings
    normalized["allowed_layers"] = list(layers)
    return normalized


def _physical_adapter(value: Mapping[str, Any]) -> Dict[str, Any]:
    _strict_keys(value, {"kind", "tool", "request_version", "response_version", "supported_evidence"}, set(), "physical_design adapter")
    kind = _text(value["kind"], "physical_design adapter kind")
    if kind not in {"gdsfactory", "circuit_simulator", "electromagnetic_simulator"}:
        raise CapabilityError("unsupported physical_design adapter kind")
    tool = _mapping(value["tool"], "physical_design adapter tool")
    _strict_keys(tool, {"name", "version"}, set(), "physical_design adapter tool")
    request_version = _text(value["request_version"], "physical_design adapter request version")
    response_version = _text(value["response_version"], "physical_design adapter response version")
    _expect_version(request_version, PHYSICAL_DESIGN_VERSION, "physical design adapter request")
    _expect_version(response_version, PHYSICAL_DESIGN_VERSION, "physical design adapter response")
    evidence = tuple(
        _text(item, "physical_design adapter evidence")
        for item in _sequence(value["supported_evidence"], "physical_design adapter evidence")
    )
    allowed = {"connectivity", "drc", "lvs", "circuit_simulation", "electromagnetic_simulation"}
    if any(item not in allowed for item in evidence):
        raise CapabilityError("unsupported physical_design evidence kind")
    _non_empty_unique(evidence, "physical_design adapter evidence", allow_empty=True)
    return {
        "kind": kind,
        "tool": {"name": _text(tool["name"], "physical_design tool name"), "version": _text(tool["version"], "physical_design tool version")},
        "request_version": request_version,
        "response_version": response_version,
        "supported_evidence": list(evidence),
    }


def _physical_model(value: Mapping[str, Any], proprietary: bool) -> Dict[str, Any]:
    _strict_keys(value, {"name", "framework", "artifact", "ports", "wavelength", "parameters"}, set(), "physical_design circuit model")
    framework = _text(value["framework"], "physical_design model framework")
    if framework not in {"circulax", "sax", "touchstone", "analytic"}:
        raise CapabilityError("unsupported physical_design circuit framework")
    ports = tuple(
        _text(item, "physical_design model port")
        for item in _sequence(value["ports"], "physical_design model ports")
    )
    _non_empty_unique(ports, "physical_design model ports")
    return {
        "name": _text(value["name"], "physical_design model name"),
        "framework": framework,
        "artifact": _physical_artifact(_mapping(value["artifact"], "physical_design model artifact"), "physical_design model artifact", proprietary),
        "ports": list(ports),
        "wavelength": _physical_wavelength(_mapping(value["wavelength"], "physical_design model wavelength"), "physical_design model wavelength"),
        "parameters": _physical_parameters(_mapping(value["parameters"], "physical_design model parameters"), "physical_design model parameters"),
    }


def _physical_evidence(value: Mapping[str, Any], proprietary: bool) -> Dict[str, Any]:
    _strict_keys(value, {"kind", "status", "tool", "settings_fingerprint", "report"}, set(), "physical_design verification")
    kind = _text(value["kind"], "physical_design verification kind")
    allowed = {"connectivity", "drc", "lvs", "circuit_simulation", "electromagnetic_simulation"}
    if kind not in allowed:
        raise CapabilityError("unsupported physical_design verification kind")
    if value["status"] != "passed":
        raise CapabilityError("physical_design binding cannot contain failed evidence")
    tool = _mapping(value["tool"], "physical_design verification tool")
    _strict_keys(tool, {"name", "version"}, set(), "physical_design verification tool")
    return {
        "kind": kind,
        "status": "passed",
        "tool": {"name": _text(tool["name"], "physical_design verification tool name"), "version": _text(tool["version"], "physical_design verification tool version")},
        "settings_fingerprint": _sha256_text(value["settings_fingerprint"], "physical_design verification settings"),
        "report": _physical_artifact(_mapping(value["report"], "physical_design verification report"), "physical_design verification report", proprietary),
    }


def _canonical_json(value: Mapping[str, Any]) -> str:
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False, allow_nan=False)


def _sha256_text(value: Any, name: str) -> str:
    text = _text(value, name)
    digest = text.removeprefix("sha256:")
    if not text.startswith("sha256:") or len(digest) != 64 or any(
        character not in "0123456789abcdef" for character in digest
    ):
        raise CapabilityError(name + " must be a lowercase sha256 digest")
    return text


def _text(value: Any, name: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise CapabilityError(name + " must be a non-empty string")
    return value


def _topology_fingerprint(capabilities: DeviceCapabilities) -> str:
    payload = bytearray(capabilities.backend_id.encode())
    for dimension in (
        capabilities.matrix_core.m,
        capabilities.matrix_core.n,
        capabilities.matrix_core.k,
        capabilities.simultaneous_channels,
    ):
        payload.extend(struct.pack("<Q", dimension))
    for wavelength in capabilities.supported_wavelengths_nm:
        payload.extend(struct.pack("<d", wavelength))
    payload.extend(capabilities.physical_design.fingerprint.encode())
    fingerprint = 0xCBF29CE484222325
    for byte in payload:
        fingerprint ^= byte
        fingerprint = (fingerprint * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return "fnv1a64:{:016x}".format(fingerprint)


def _validate_transfer(
    gain: float,
    offset: float,
    phase_error_radians: float,
    insertion_loss_db: float,
    uncertainty: float,
    name: str,
) -> None:
    if not all(
        math.isfinite(value)
        for value in (
            gain,
            offset,
            phase_error_radians,
            insertion_loss_db,
            uncertainty,
        )
    ):
        raise CapabilityError("{} transfer fields must be finite".format(name))
    if gain == 0:
        raise CapabilityError("{} gain must be non-zero".format(name))
    if insertion_loss_db < 0 or uncertainty < 0:
        raise CapabilityError(
            "{} insertion loss and uncertainty must be non-negative".format(name)
        )


def _valid_calibration_fingerprint(value: str) -> bool:
    prefix, separator, digest = value.partition(":")
    expected_length = {"sha256": 64, "fnv1a64": 16}.get(prefix)
    return (
        separator == ":"
        and expected_length is not None
        and len(digest) == expected_length
        and all(character in "0123456789abcdef" for character in digest)
    )


def _strict_keys(
    value: Mapping[str, Any],
    required: set,
    optional: set,
    name: str,
) -> None:
    keys = set(value)
    missing = sorted(required - keys)
    unknown = sorted(keys - required - optional)
    if missing:
        raise CapabilityError("{} missing fields: {}".format(name, ", ".join(missing)))
    if unknown:
        raise CapabilityError("{} has unknown fields: {}".format(name, ", ".join(unknown)))


def _mapping(value: Any, name: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise CapabilityError("{} must be an object".format(name))
    return value


def _sequence(value: Any, name: str) -> Sequence[Any]:
    if not isinstance(value, (list, tuple)):
        raise CapabilityError("{} must be an array".format(name))
    return value


def _integer(value: Any, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise CapabilityError("{} must be an integer".format(name))
    return value


def _number(value: Any, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise CapabilityError("{} must be numeric".format(name))
    result = float(value)
    if not math.isfinite(result):
        raise CapabilityError("{} must be finite".format(name))
    return result


def _expect_version(actual: str, expected: str, name: str) -> None:
    if actual != expected:
        raise CapabilityError(
            "unsupported {} version '{}'; expected '{}'".format(name, actual, expected)
        )


def _timestamp(value: str, name: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise CapabilityError("{} must be RFC 3339".format(name)) from error
    if parsed.tzinfo is None:
        raise CapabilityError("{} must include a timezone".format(name))
    return parsed.astimezone(timezone.utc)


def _unique(values: Sequence[Any], name: str) -> None:
    if len(set(values)) != len(values):
        raise CapabilityError("{} must not contain duplicates".format(name))


def _non_empty_unique(
    values: Sequence[str], name: str, allow_empty: bool = False
) -> None:
    if not values and not allow_empty:
        raise CapabilityError("{} must not be empty".format(name))
    if any(not value for value in values):
        raise CapabilityError("{} contains an empty identifier".format(name))
    _unique(values, name)


def _positive_unique(values: Sequence[float], name: str) -> None:
    if not values or any(value <= 0 for value in values):
        raise CapabilityError("{} must contain positive values".format(name))
    _unique(values, name)
