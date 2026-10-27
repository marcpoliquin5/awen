"""H4 Haar layer defect check: the torch Haar transform against an independent NumPy matrix reference.

The reference builds each level as an explicit orthonormal analysis matrix (pair sums then pair differences,
scaled by 1/sqrt(2)) acting on the coarse corner, and applies it separably in 2-D as L @ X @ L.T. It shares no
code with ``h4_multiscale.haar``.
"""

import math
import sys
from pathlib import Path

import pytest

torch = pytest.importorskip("torch")
np = pytest.importorskip("numpy")

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
sys.path.insert(0, str(FRACTAL))
import h4_multiscale as h4  # noqa: E402

R = 1 / math.sqrt(2)


def _level_matrix(n: int, lv: int) -> np.ndarray:
    """n x n matrix of Haar analysis level ``lv``: transforms the first n >> lv entries, identity elsewhere."""
    m, a = np.eye(n), n >> lv
    blk = np.zeros((a, a))
    for i in range(a // 2):
        blk[i, 2 * i] = blk[i, 2 * i + 1] = R
        blk[a // 2 + i, 2 * i], blk[a // 2 + i, 2 * i + 1] = R, -R
    m[:a, :a] = blk
    return m


def _analysis(n: int, levels: int) -> np.ndarray:
    m = np.eye(n)
    for lv in range(levels):
        m = _level_matrix(n, lv) @ m
    return m


def _ref_2d(x: np.ndarray, levels: int) -> np.ndarray:
    """Level-by-level separable reference: at each level only the coarse (LL) corner is transformed, rows and
    columns alike, so the detail bands of earlier levels are left untouched (square Mallat layout)."""
    out = x.copy()
    for lv in range(levels):
        r, c = x.shape[-2] >> lv, x.shape[-1] >> lv
        lr, lc = _level_matrix(r, 0), _level_matrix(c, 0)  # full-size level-0 matrix of the corner
        out[..., :r, :c] = lr @ out[..., :r, :c] @ lc.T
    return out


@pytest.mark.parametrize("n,levels", [(8, 1), (8, 3), (256, 4), (512, 5)])
def test_1d_forward_and_inverse_match_matrix_reference(n, levels):
    x = np.random.default_rng(0).standard_normal((3, 2, n))
    a = _analysis(n, levels)
    np.testing.assert_allclose(a @ a.T, np.eye(n), atol=1e-12)  # the reference itself is orthonormal
    got = h4.haar(torch.tensor(x), levels).numpy()
    np.testing.assert_allclose(got, x @ a.T, atol=1e-12)
    back = h4.haar(torch.tensor(got), levels, inverse=True).numpy()
    np.testing.assert_allclose(back, x, atol=1e-12)


@pytest.mark.parametrize("shape,levels", [((8, 8), 2), ((28, 28), 2), ((16, 8), 3)])
def test_2d_forward_and_inverse_match_matrix_reference(shape, levels):
    x = np.random.default_rng(1).standard_normal((2, 3, *shape))
    got = h4.haar(torch.tensor(x), levels).numpy()
    np.testing.assert_allclose(got, _ref_2d(x, levels), atol=1e-12)
    np.testing.assert_allclose(h4.haar(torch.tensor(got), levels, inverse=True).numpy(), x, atol=1e-12)
    np.testing.assert_allclose((got ** 2).sum((-2, -1)), (x ** 2).sum((-2, -1)), rtol=1e-12)  # energy kept


def test_haar_gradient_matches_transpose_of_reference():
    """In-place corner writes must not corrupt autograd: d<g, H x>/dx = H.T g."""
    n, levels = 16, 3
    x = torch.randn(1, 1, n, dtype=torch.float64, requires_grad=True)
    g = torch.randn(1, 1, n, dtype=torch.float64)
    (h4.haar(x, levels) * g).sum().backward()
    np.testing.assert_allclose(x.grad.numpy(), g.numpy() @ _analysis(n, levels), atol=1e-12)
    x2 = torch.randn(1, 1, 8, 8, dtype=torch.float64, requires_grad=True)
    assert torch.autograd.gradcheck(lambda t: h4.haar(t, 2), (x2,))
    assert torch.autograd.gradcheck(lambda t: h4.haar(t, 2, inverse=True), (x2,))


@pytest.mark.parametrize("shape,levels", [((256,), 4), ((28, 28), 2)])
def test_bands_label_the_level_each_coefficient_came_from(shape, levels):
    band = h4.haar_bands(shape, levels).view(shape)
    # a constant input has all its energy in the coarse band 0
    c = h4.haar(torch.ones(1, 1, *shape, dtype=torch.float64), levels)[0, 0]
    assert torch.allclose(c[band != 0], torch.zeros(()).double(), atol=1e-12)
    # an input made of one detail basis function at level lv lands only in band lv + 1
    for lv in range(levels):
        coef = torch.zeros(1, 1, *shape, dtype=torch.float64)
        idx = (band == lv + 1).nonzero()[0]
        coef[(0, 0, *idx.tolist())] = 1.0
        x = h4.haar(coef, levels, inverse=True)
        c = h4.haar(x, levels)[0, 0]
        assert torch.allclose(c[band != lv + 1], torch.zeros(()).double(), atol=1e-12)
    counts = torch.bincount(band.flatten(), minlength=levels + 1).tolist()
    n, d = math.prod(shape), len(shape)
    assert counts == [n // 2 ** (d * levels)] + [n // 2 ** (d * lv) - n // 2 ** (d * (lv + 1)) for lv in range(levels)]


def test_haar_block_with_identity_band_mixing_is_identity_before_skip():
    """With every band's mixing matrix set to I, analysis -> mix -> synthesis returns the input exactly."""
    torch.manual_seed(0)
    blk = h4.Block("haar_wavelet", 4, (28, 28), modes=8, levels=2).double()
    with torch.no_grad():
        blk.mix.copy_(torch.eye(4, dtype=torch.float64).expand_as(blk.mix))
    h = torch.randn(2, 4, 28, 28, dtype=torch.float64)
    c = h4.haar(h, 2).flatten(2)
    c = torch.einsum("bik,kio->bok", c, blk.mix[blk.band])
    y = h4.haar(c.view(2, 4, 28, 28), 2, inverse=True)
    np.testing.assert_allclose(y.detach().numpy(), h.numpy(), atol=1e-12)
