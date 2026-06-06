/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/

pub mod base;
pub mod generic;
pub mod info;
pub mod meta;

pub use self::base::{
    AtorsBase, disable_notifications, enable_notifications, freeze, get_event,
    get_event_customization_tool, get_events, get_events_by_tag, get_events_by_tag_and_value,
    get_member, get_member_customization_tool, get_members, get_members_by_tag,
    get_members_by_tag_and_value, is_frozen, is_notifications_enabled,
    maybe_freeze_instance_after_call, observe, unobserve,
};
pub use self::generic::create_ators_specialized_subclass;
pub use self::info::{
    MembersByNameMapping, PicklePolicy, create_ators_specialized_alias, drop_class_info,
    get_ators_abstract_methods, get_ators_args, get_ators_frozen_flag, get_ators_init_member_names,
    get_ators_members_by_name, get_ators_origin, get_ators_specific_member_names,
    get_ators_type_params, get_tracked_class_info_size,
};
pub use self::meta::create_ators_subclass;
