Iron
====

Features
--------

- use only type annotations (using Annotated)
- automatically slotted
- support for weakref like mechanism (simply create an extra slot on demand and use weakref)
    - standard is better here
    - more memory hungry
- compatible with dataclass transform
- compatible with ABC
- no implicit default values, if no default is provided an exception is raised
    - support for custom args, kwargs or callable to create a default value
- chained validation steps (typing first, followed by custom steps validating the values)
- custom pickling support
- coercion on assignment or only init
- ability to create immutable object
- support for metadata on descriptors
- support for immutable containers
- support for forwarded types, str like pydantic or callable for circular import
- support for narrowing generics validation
- support for custom access behavior:
    - standard
    - read-only
    - constant
    - write-only (event like)
- provide helpers to add standard behaviors
    - ``__repr__``
    - ``__eq__``
- allow to derive pydantic model and to convert to and from such objects
- minimal object clutter
  use free functions above methods
  provide fast shards filtering based on metadata
- optionally support observation (lower priority)

Questions
---------

- support for post_getattr like behavior ?
- support for post_setattr like behavior ?

Implementation
--------------

- rely on Annotated
- implement in rust
- Iron (base object)
- shard (descriptors)
- ore (configuration, better discoverability than metaclass args)
- use a bit field to store config
  (atom has guard (used for atomref), atomref, notifications, frozen,
   and number of slots which we won't need when using an array ref)
    - frozen


Strategy
--------

- minimal project to benchmarck access to object stored in array ref
- measure object size
