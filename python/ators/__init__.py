# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
""""""

from ._ators import (
    AtorsBase as _Base,
    add_generic_type_attributes,
    init_ators as _init_ators,
    freeze,
    is_frozen,
    member,
    Member,
    get_member,
    get_members,
    get_members_by_tag,
    get_members_by_tag_and_value,
)
from ._meta import AtorsMeta as _Meta


# Register generic type attributes for numpy ndarray if numpy is available
try:
    import numpy as np

    add_generic_type_attributes(np.ndarray, ("shape", "dtype"))
except ImportError:
    pass

# Register generic type attributes for pint Quantity if pint is available
try:
    from pint import Quantity

    add_generic_type_attributes(Quantity, ("_magnitude",))
except ImportError:
    pass


def __newobj__(cls, *args):
    """A compatibility pickler function.

    This function is not part of the public Atom api.

    """
    return cls.__new__(cls, *args)


class Ators(_Base, metaclass=_Meta):
    """"""

    # NOTE implement init using a rust accelerator, once pyo3 support init it
    # will be moved
    def __init__(self, **kwargs):
        _init_ators(self, kwargs)

    def __reduce_ex__(self, proto):
        """An implementation of the reduce protocol.

        This method creates a reduction tuple for Atom instances. This
        method should not be overridden by subclasses unless the author
        fully understands the rammifications.

        """
        args = (type(self), *self.__getnewargs__())
        return (__newobj__, args, self.__getstate__())

    def __getnewargs__(self) -> tuple:
        """Get the argument tuple to pass to __new__ on unpickling.

        See the Python.org docs for more information.

        """
        return ()


__all__ = [
    "Ators",
    "add_generic_type_attributes",
    "freeze",
    "is_frozen",
    "member",
    "Member",
    "get_member",
    "get_members",
    "get_members_by_tag",
    "get_members_by_tag_and_value",
]
