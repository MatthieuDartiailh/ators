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

Defaults
--------

There are three ways to provide a default for a member.

**Static default** — pass a plain value directly to the constructor::

   class Config(Ators):
       retries: int = member(default=3)
       label: str = member(default="unnamed")

This is equivalent to the chained form ``member().default(value)``.

**Factory default** — provide a zero-argument callable; it is called once
on first access and the result is cached for that instance::

   class Container(Ators):
       items: list = member(default_factory=list)

``default_factory`` must be callable and accept **zero** arguments.
A ``ValueError`` is raised at class definition time if the callable
has the wrong signature.

``default`` and ``default_factory`` are **mutually exclusive**; specifying
both raises ``TypeError``::

   member(default=0, default_factory=int)  # TypeError

**Advanced defaults** — for behaviors that depend on the owner instance
(e.g. calling a method on the object) use the ``.default()`` chaining API
with an explicit ``DefaultBehavior`` variant, or the ``@default`` decorator
from ``ators.behaviors``::

   from ators.behaviors import Default, default

   class Computed(Ators):
       a: int = member().default(Default.Call(lambda: expensive()))

       b: int = member()

       @default(b)
       def _default_b(self, member):
           return self.a * 2

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
