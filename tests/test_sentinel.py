import copy
import pickle
import sys

from ators import UNSET


def test_unset_is_public_and_has_a_concise_repr():
    from ators import __all__

    assert "UNSET" in __all__
    assert repr(UNSET) == "UNSET"


def test_unset_preserves_identity_across_copy_and_pickle():
    assert copy.copy(UNSET) is UNSET
    assert copy.deepcopy(UNSET) is UNSET
    assert pickle.loads(pickle.dumps(UNSET)) is UNSET


def test_unset_uses_builtin_sentinel_when_available():
    if sys.version_info >= (3, 15):
        import builtins

        assert type(UNSET) is builtins.sentinel
