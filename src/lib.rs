/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::pymodule;

mod annotations;
mod core;
mod member;
mod meta;
mod validators;

/// A Python module implemented in Rust.
#[pymodule]
mod _ators {
    use super::*;

    #[pymodule_export]
    use self::core::{AtorsBase, freeze, init_ators, is_frozen};
    #[pymodule_export]
    use self::meta::create_ators_subclass;

    #[pymodule_export]
    use self::member::{
        DefaultBehavior, DelattrBehavior, Member, MemberBuilder, PostGetattrBehavior,
        PostSetattrBehavior, PreGetattrBehavior, PreSetattrBehavior,
    };

    #[pymodule_export]
    use self::validators::{Coercer, TypeValidator, Validator, ValueValidator};
}
