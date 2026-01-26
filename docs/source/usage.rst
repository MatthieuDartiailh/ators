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
