"""Versioned AWEN backend capability and live-health contracts.

This module intentionally uses only the Python standard library so framework
frontends and backend plugins can validate discovery data before importing a
native AWEN runtime binding.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import math
import struct
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple


CAPABILITY_VERSION = "awen.device-capability.v1"
HEALTH_VERSION = "awen.backend-health.v1"
RUNTIME_ABI_VERSION = "awen.runtime-backend.v1"
PLUGIN_ABI_VERSION = "awen.backend-plugin.v1"
CALIBRATION_SNAPSHOT_VERSION = "awen.calibration-snapshot.v1"


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
