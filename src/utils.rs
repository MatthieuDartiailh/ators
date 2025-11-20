/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///

///
macro_rules! create_behavior_callable_checker {
    ($mod: ident, $behavior:ident, $variant:ident, $n:literal) => {
        mod $mod {
            use pyo3::{
                Bound, FromPyObject, IntoPyObject, Py, PyAny, PyResult, Python, intern,
                types::PyAnyMethods,
            };
            use std::convert::Infallible;

            #[derive(Debug)]
            pub struct Callable(pub Py<PyAny>);

            impl FromPyObject<'_> for Callable {
                fn extract_bound<'py>(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
                    let py = ob.py();
                    let sig = py
                        .import(intern!(py, "inspect"))?
                        .getattr(intern!(py, "signature"))?;
                    let ob_sig_len = sig
                        .call1((ob,))?
                        .getattr(intern!(py, "parameters"))?
                        .len()?;
                    if !ob.is_callable() || ob_sig_len != $n {
                        Err(pyo3::exceptions::PyValueError::new_err(format!(
                            "{}.{} expect a callable taking {} got {} which takes {}.",
                            stringify!($behavior),
                            stringify!($variant),
                            $n,
                            ob,
                            ob_sig_len
                        )))
                    } else {
                        Ok(Callable(ob.clone().unbind()))
                    }
                }
            }

            impl<'py> IntoPyObject<'py> for &Callable {
                type Target = PyAny;
                type Output = Bound<'py, PyAny>;
                type Error = Infallible;
                fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
                    Ok(self.0.clone_ref(py).into_bound(py))
                }
            }
        }
    };
}

pub(crate) use create_behavior_callable_checker;
// This approach allows to implement an equivalent of custom constructor
// for enums
