Usage
=====

Ators is a high-performance attribute system for Python, implemented in Rust. It provides a way to define classes with typed, validated, and observable attributes (members).

Defining an Ators Class
-----------------------

To create an Ators class, inherit from ``Ators``:

.. code-block:: python

   from ators import Ators, member

   class MyClass(Ators):
       # Define a member with type validation
       name = member(str)

       # Define a member with a default value
       age = member(int).default(0)

   obj = MyClass(name="Alice", age=30)
   print(obj.name)  # Alice

Key Features
------------

* **Type Safety**: Members are validated against their type at assignment time.
* **Performance**: Slot-based storage and Rust-accelerated validation.
* **Behaviors**: Custom logic for getting, setting, and deleting attributes.
* **Freezing**: Objects can be made immutable (frozen).

Supported Typing Constructs
----------------------------

Ators supports many standard Python typing annotations for member validation.

Constrained ``TypeVar``
~~~~~~~~~~~~~~~~~~~~~~~~

Constrained ``TypeVar``\s (e.g., ``TypeVar("T", int, str)``) are supported and
treated as equivalent to a union of their constraints for validation purposes.

.. code-block:: python

   from typing import TypeVar
   from ators import Ators, member

   T = TypeVar("T", int, str)

   class Box(Ators):
       item: T = member(T)

   box = Box()
   box.item = 1      # OK
   box.item = "x"    # OK
   box.item = 1.5    # raises TypeError

This is semantically equivalent to annotating ``item`` as ``int | str``.

.. note::
   Unconstrained ``TypeVar``\s and ``TypeVar``\s with a ``bound`` continue to
   work as before.  Having both ``__constraints__`` and ``__bound__`` set on
   the same ``TypeVar`` is a typing-library error and is not supported.
