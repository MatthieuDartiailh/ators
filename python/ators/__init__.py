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
    init_ators as _init_ators,
    freeze,
    is_frozen,
    member,
    Member,
)
from ._meta import AtorsMeta as _Meta


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


__all__ = ["Ators", "freeze", "is_frozen", "member", "Member"]
