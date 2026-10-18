"""Operator library: the Rust f64 reference against independent NumPy oracles,
and the photonic engine (simulated) against the reference."""

import numpy as np
import pytest

import awen_native

RNG = np.random.default_rng(7)


def u(*shape):
    return RNG.uniform(-1, 1, shape)


def ref(op, inputs, **params):
    out, report = awen_native.run_op(op, inputs, params)
    assert report["fidelity"].startswith("exact")
    return out


def test_table_levels_are_ordered_and_none_measured():
    table = awen_native.op_table()
    names = {o["name"] for o in table}
    assert {"gemm", "complex_matmul", "dft", "conv2d", "correlate1d", "low_rank_linear",
            "attention", "ffn", "spectral_conv1d", "rnn_step", "reservoir_step",
            "intensity_layer"} == names
    assert all(o["level"] != "hardware_measured" for o in table)
    assert [o["level"] for o in table if o["name"] == "gemm"] == ["hardware_lowerable"]


def test_complex_matmul_matches_numpy_complex():
    xr, xi, wr, wi = u(4, 9), u(4, 9), u(6, 9), u(6, 9)
    out = ref("complex_matmul", dict(xr=xr, xi=xi, wr=wr, wi=wi))
    want = (xr + 1j * xi) @ (wr + 1j * wi).T
    np.testing.assert_allclose(out["yr"] + 1j * out["yi"], want, atol=1e-12)


@pytest.mark.parametrize("inverse", [False, True])
def test_dft_matches_numpy_fft(inverse):
    xr, xi = u(3, 16), u(3, 16)
    out = ref("dft", dict(xr=xr, xi=xi), inverse=inverse)
    x = xr + 1j * xi
    want = np.fft.ifft(x, axis=-1) if inverse else np.fft.fft(x, axis=-1)
    np.testing.assert_allclose(out["yr"] + 1j * out["yi"], want, atol=1e-11)


def test_conv2d_matches_numpy_sliding_window():
    x, w, b = u(3, 7, 6), u(4, 3, 3, 2), u(4)
    stride, pad = 2, 1
    out = ref("conv2d", dict(x=x, w=w, bias=b), stride=stride, padding=pad)["y"]
    xp = np.pad(x, ((0, 0), (pad, pad), (pad, pad)))
    win = np.lib.stride_tricks.sliding_window_view(xp, (3, 2), axis=(1, 2))[:, ::stride, ::stride]
    want = np.einsum("chwij,ocij->ohw", win, w) + b[:, None, None]
    np.testing.assert_allclose(out, want, atol=1e-12)


def test_correlate1d_matches_numpy_correlate():
    x, k = u(3, 20), u(5)
    out = ref("correlate1d", dict(x=x, kernel=k))["y"]
    want = np.stack([np.correlate(row, k, mode="valid") for row in x])
    np.testing.assert_allclose(out, want, atol=1e-12)


def test_low_rank_linear():
    x, uu, v = u(5, 12), u(3, 12), u(7, 3)
    out = ref("low_rank_linear", dict(x=x, u=uu, v=v))["y"]
    np.testing.assert_allclose(out, x @ (v @ uu).T, atol=1e-12)


@pytest.mark.parametrize("causal", [False, True])
def test_attention_matches_numpy(causal):
    t, d, dh = 6, 8, 4
    x, wq, wk, wv, wo = u(t, d), u(dh, d), u(dh, d), u(dh, d), u(d, dh)
    out = ref("attention", dict(x=x, wq=wq, wk=wk, wv=wv, wo=wo), causal=causal)["y"]
    q, k, v = x @ wq.T, x @ wk.T, x @ wv.T
    s = q @ k.T / np.sqrt(dh)
    if causal:
        s = np.where(np.tril(np.ones((t, t), bool)), s, -np.inf)
    p = np.exp(s - s.max(1, keepdims=True))
    p /= p.sum(1, keepdims=True)
    np.testing.assert_allclose(out, (p @ v) @ wo.T, atol=1e-12)


def test_ffn_matches_numpy_tanh_gelu():
    x, w1, b1, w2, b2 = u(4, 10), u(16, 10), u(16), u(10, 16), u(10)
    out = ref("ffn", dict(x=x, w1=w1, b1=b1, w2=w2, b2=b2))["y"]
    h = x @ w1.T + b1
    g = 0.5 * h * (1 + np.tanh(np.sqrt(2 / np.pi) * (h + 0.044715 * h**3)))
    np.testing.assert_allclose(out, g @ w2.T + b2, atol=1e-12)


def test_spectral_conv1d_matches_numpy_fft_filter():
    x, hr, hi = u(2, 12), u(12), u(12)
    out = ref("spectral_conv1d", dict(x=x, hr=hr, hi=hi))["y"]
    want = np.fft.ifft(np.fft.fft(x, axis=-1) * (hr + 1j * hi), axis=-1).real
    np.testing.assert_allclose(out, want, atol=1e-11)


def test_rnn_and_reservoir_steps():
    h, x, w, uu, b = u(3, 6), u(3, 4), u(6, 6), u(6, 4), u(6)
    out = ref("rnn_step", dict(h=h, x=x, w=w, u=uu, b=b))["h_next"]
    np.testing.assert_allclose(out, np.tanh(h @ w.T + x @ uu.T + b), atol=1e-12)
    out = ref("reservoir_step", dict(s=h, x=x, w=w, w_in=uu), leak=0.25)["s_next"]
    want = 0.75 * h + 0.25 * np.tanh(h @ w.T + x @ uu.T)
    np.testing.assert_allclose(out, want, atol=1e-12)


def test_intensity_layer_is_square_law_detection():
    x, wr, wi = u(4, 8), u(5, 8), u(5, 8)
    out = ref("intensity_layer", dict(x=x, wr=wr, wi=wi))["y"]
    np.testing.assert_allclose(out, np.abs(x @ (wr + 1j * wi).T) ** 2, atol=1e-12)


def test_photonic_engine_ideal_equals_reference_and_noise_shrinks_with_photons():
    x, w = u(8, 64), u(16, 64)
    exact = x @ w.T
    y, rep = awen_native.run_op("gemm", dict(x=x, w=w), contract=awen_native.ideal_contract())
    np.testing.assert_allclose(y["y"], exact, atol=1e-10)
    assert rep["fidelity"].startswith("simulated")
    errs = []
    for photons in (1e2, 1e4):
        c = awen_native.ideal_contract()
        c.update(photons_per_mac=photons, quantum_efficiency=1.0, insertion_loss_db=0.0)
        y, rep = awen_native.run_op("conv2d", dict(x=u(2, 6, 6), w=u(3, 2, 3, 3)), contract=c, seed=1)
        errs.append(rep["photonic_macs"])
        y2, _ = awen_native.run_op("gemm", dict(x=x, w=w), contract=c, seed=1)
        errs.append(float(np.sqrt(np.mean((y2["y"] - exact) ** 2))))
    assert errs[0] == errs[2] == 16 * 3 * 18
    assert errs[3] < errs[1] / 3  # shot noise falls ~10x for 100x photons


def test_unknown_op_and_bad_shape_raise():
    with pytest.raises(ValueError):
        awen_native.run_op("teleport", {})
    with pytest.raises(ValueError):
        awen_native.run_op("gemm", dict(x=u(2, 3), w=u(2, 4)))
