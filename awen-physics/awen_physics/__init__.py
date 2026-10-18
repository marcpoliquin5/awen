"""Reference physics models used to validate AWEN's compact models.

Every result carries a :class:`~awen_physics.fidelity.Fidelity` label. Nothing
here is a hardware measurement.
"""

from .fidelity import Fidelity

__all__ = ["Fidelity"]
__version__ = "0.1.0"
