//! Qt string glue for the thin shell.
//!
//! All logic lives in `ui`; this module only translates QStrings
//! at the boundary.

use cxx_qt_lib::QString;

/// Read a `QString` property as an owned string.
pub(crate) fn s(value: &QString) -> String {
    value.to_string()
}
