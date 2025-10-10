use pyo3::pymodule;

mod core;
mod member;
mod validators;

/// A Python module implemented in Rust.
#[pymodule]
mod _ators {
    use super::*;

    #[pymodule_export]
    use self::core::{BaseAtors, freeze, is_frozen};

    #[pymodule_export]
    use self::member::{
        DefaultBehavior, DelattrBehavior, Member, PostGetattrBehavior, PostSetattrBehavior,
        PreGetattrBehavior, PreSetattrBehavior,
    };

    #[pymodule_export]
    use self::validators::{Coercer, TypeValidator, Validator, ValueValidator};
}
