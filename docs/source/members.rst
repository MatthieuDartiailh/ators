Members
=======

Members are the core building blocks of Ators. They act as descriptors that manage attribute access, validation, and behaviors.

Internal Workings
-----------------

The following diagrams describe the internal logic executed when accessing or modification a member.

Member GET Flow
~~~~~~~~~~~~~~~

When you access a member (e.g., ``value = obj.member_name``), the following flow occurs:

.. graphviz::

   digraph get_flow {
       node [shape=box, fontname="Arial"];

       start [label="Access obj.member_name", shape=ellipse];
       pre_get [label="Execute Pre-Get Behaviors"];
       check_slot [label="Check internal slot", shape=diamond];
       has_value [label="Return Value from Slot"];
       no_value [label="Generate Default Value"];
       validate_default [label="Validate Default Value"];
       store_default [label="Store in Slot"];
       post_get [label="Execute Post-Get Behaviors"];
       end [label="Return Value", shape=ellipse];

       start -> pre_get;
       pre_get -> check_slot;
       check_slot -> has_value [label="Exists"];
       check_slot -> no_value [label="Empty"];
       no_value -> validate_default;
       validate_default -> store_default;
       store_default -> post_get;
       has_value -> post_get;
       post_get -> end;
   }

Member SET Flow
~~~~~~~~~~~~~~~

When you assign a value to a member (e.g., ``obj.member_name = value``), the following flow occurs:

.. graphviz::

   digraph set_flow {
       node [shape=box, fontname="Arial"];

       start [label="Assign obj.member_name = value", shape=ellipse];
       check_frozen [label="Is Object Frozen?", shape=diamond];
       is_frozen [label="Raise TypeError", color=red];
       pre_set [label="Execute Pre-Set Behaviors\n(Check ReadOnly/Constant)"];
       validate [label="Validate & Coerce Value"];
       store [label="Store Value in Slot"];
       post_set [label="Execute Post-Set Behaviors\n(Notifications)"];
       end [label="Done", shape=ellipse];

       start -> check_frozen;
       check_frozen -> is_frozen [label="Yes"];
       check_frozen -> pre_set [label="No"];
       pre_set -> validate;
       validate -> store;
       store -> post_set;
       post_set -> end;
   }

Behaviors
---------

Ators allows you to attach custom logic to various stages of the member lifecycle:

* **Default**: Generate a value if the slot is empty.
* **Pre-Get**: Logic executed before value retrieval.
* **Post-Get**: Logic executed after value retrieval.
* **Pre-Set**: Logic executed before value assignment (can block or adjust).
* **Post-Set**: Logic executed after value assignment (e.g., observers).
* **Validate**: Custom validation logic.
