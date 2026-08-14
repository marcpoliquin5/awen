import copy
import json
from pathlib import Path
import unittest

from awen_py.capabilities import (
    BackendHealth,
    BackendSnapshot,
    CapabilityError,
    DeviceCapabilities,
)


ROOT = Path(__file__).resolve().parents[3]


def load(name):
    with (ROOT / "awen-compiler" / "capabilities" / name).open(
        encoding="utf-8"
    ) as handle:
        return json.load(handle)


class CapabilityContractTests(unittest.TestCase):
    def snapshot(self):
        return BackendSnapshot(
            DeviceCapabilities.from_dict(load("pace_like_128.json")),
            BackendHealth.from_dict(load("pace_like_128.health.json")),
        )

    def test_reference_profile_is_eligible(self):
        negotiation = self.snapshot().negotiate_gemm((256, 256, 256), "f16", 8)
        self.assertTrue(negotiation.eligible)
        self.assertEqual(negotiation.diagnostics, ())

    def test_version_skew_is_rejected(self):
        value = load("pace_like_128.json")
        value["runtime_abi_version"] = "awen.runtime-backend.v2"
        with self.assertRaisesRegex(CapabilityError, "runtime ABI"):
            DeviceCapabilities.from_dict(value)

    def test_analog_noise_is_typed_and_strictly_validated(self):
        capability = DeviceCapabilities.from_dict(load("pace_like_128.json"))
        self.assertGreater(capability.analog_noise.shot_noise_fraction, 0)

        missing = load("pace_like_128.json")
        del missing["analog_noise"]
        with self.assertRaisesRegex(CapabilityError, "analog_noise"):
            DeviceCapabilities.from_dict(missing)

        negative = load("pace_like_128.json")
        negative["analog_noise"]["thermal_noise_fraction"] = -0.1
        with self.assertRaisesRegex(CapabilityError, "thermal-noise"):
            DeviceCapabilities.from_dict(negative)

    def test_calibration_snapshot_fingerprint_and_topology_are_enforced(self):
        capability = DeviceCapabilities.from_dict(load("pace_like_128.json"))
        profile = capability.calibration_profile
        self.assertEqual(profile.snapshot_version, "awen.calibration-snapshot.v1")
        self.assertTrue(profile.fingerprint.startswith("sha256:"))
        self.assertEqual(profile.cells[0].id, "cell-0-0")

        wrong_topology = load("pace_like_128.json")
        wrong_topology["calibration_profile"]["topology_fingerprint"] = (
            "fnv1a64:0000000000000000"
        )
        with self.assertRaisesRegex(CapabilityError, "topology fingerprint"):
            DeviceCapabilities.from_dict(wrong_topology)

        health = load("pace_like_128.health.json")
        health["calibration_fingerprint"] = "sha256:" + "0" * 64
        negotiation = BackendSnapshot(
            capability,
            BackendHealth.from_dict(health),
        ).negotiate_gemm((128, 128, 128), "f16", 8)
        self.assertIn(
            "calibration_fingerprint_mismatch",
            [diagnostic.code for diagnostic in negotiation.diagnostics],
        )

    def test_physical_design_identity_and_proprietary_boundary_are_enforced(self):
        capability = DeviceCapabilities.from_dict(load("pace_like_128.json"))
        physical = capability.physical_design
        self.assertEqual(physical.pdk_name, "awen-example-silicon")
        self.assertEqual(physical.process_corner_id, "nominal-22c")
        self.assertEqual(physical.circuit_models, ("mzi",))
        self.assertEqual(
            physical.fingerprint,
            "sha256:b1f098f300a791775420c138f4cc51f8a5201e7e73576d078c016f9b3bdf0c62",
        )

        tampered = load("pace_like_128.json")
        tampered["physical_design"]["topology"]["nodes"][0]["settings"][
            "coupling"
        ] = 0.4
        with self.assertRaisesRegex(CapabilityError, "topology artifact digest"):
            DeviceCapabilities.from_dict(tampered)

        mutable_identity = load("pace_like_128.json")
        mutable_identity["physical_design"]["pdk"]["manifest"][
            "artifact_id"
        ] = "pdk-latest"
        with self.assertRaisesRegex(CapabilityError, "immutable urn or sha256"):
            DeviceCapabilities.from_dict(mutable_identity)

        leaked = load("pace_like_128.json")
        leaked["physical_design"]["classification"] = "proprietary_reference"
        with self.assertRaisesRegex(CapabilityError, "must not expose URIs"):
            DeviceCapabilities.from_dict(leaked)

    def test_physical_design_is_required(self):
        missing = load("pace_like_128.json")
        del missing["physical_design"]
        with self.assertRaisesRegex(CapabilityError, "physical_design"):
            DeviceCapabilities.from_dict(missing)

    def test_expired_calibration_causes_fallback(self):
        health = load("pace_like_128.health.json")
        health["observed_at"] = "2026-08-12T00:00:01Z"
        snapshot = BackendSnapshot(
            DeviceCapabilities.from_dict(load("pace_like_128.json")),
            BackendHealth.from_dict(health),
        )
        negotiation = snapshot.negotiate_gemm((128, 128, 128), "f16", 8)
        self.assertFalse(negotiation.eligible)
        self.assertIn(
            "calibration_expired",
            [diagnostic.code for diagnostic in negotiation.diagnostics],
        )

    def test_partial_tile_and_unavailable_resource_are_reported(self):
        capability = load("pace_like_128.json")
        capability["supported_operations"][0]["supports_partial_m"] = False
        health = copy.deepcopy(load("pace_like_128.health.json"))
        health["unavailable_resources"] = ["matrix_core"]
        snapshot = BackendSnapshot(
            DeviceCapabilities.from_dict(capability), BackendHealth.from_dict(health)
        )
        negotiation = snapshot.negotiate_gemm((129, 128, 128), "f16", 8)
        codes = [diagnostic.code for diagnostic in negotiation.diagnostics]
        self.assertIn("partial_m_unsupported", codes)
        self.assertIn("matrix_core_unavailable", codes)


if __name__ == "__main__":
    unittest.main()
