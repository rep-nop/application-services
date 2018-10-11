/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// TODO: it would be nice if this was an optional dep.
extern crate serde_json;
extern crate serde;

#[macro_use]
extern crate log;

use std::{panic, thread};

#[macro_use]
mod macros;
mod string;
mod error;
mod into_ffi;

pub use macros::*;
pub use string::*;
pub use error::*;
pub use into_ffi::*;

/// Call a callback that returns a `Result<T, E>` while
///
/// - Catching panics, and reporting them to C via ExternError
/// - Converting T to a c-compatible type using `IntoFfi`,
/// - Converting E to a c-compatible error via `Into<ExternError>`.
///
/// This call should be the majority of the FFI functions.
///
/// TODO: more docs for this...
pub fn call_with_result<R, E, F>(out_error: &mut ExternError, callback: F) -> R::Value
where
    F: panic::UnwindSafe + FnOnce() -> Result<R, E>,
    E: Into<ExternError>,
    R: IntoFfi,
{
    call_with_result_impl(out_error, callback, false)
}

fn call_with_result_impl<R, E, F>(out_error: &mut ExternError, callback: F, abort_on_panic: bool) -> R::Value
where
    F: FnOnce() -> Result<R, E>,
    E: Into<ExternError>,
    R: IntoFfi,
{
    *out_error = ExternError::success();
    // Our callers ensure that F is either unwind safe, or we `abort_on_panic` is true.
    let res: thread::Result<(ExternError, R::Value)> = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        match callback() {
            Ok(v) => (ExternError::default(), v.into_ffi_value()),
            Err(e) => (e.into(), R::ffi_default()),
        }
    }));
    match res {
        Ok((err, o)) => {
            *out_error = err;
            o
        }
        Err(e) => {
            error!("Caught a panic calling rust code: {:?}", e);
            if abort_on_panic {
                std::process::abort();
            }
            *out_error = e.into();
            R::ffi_default()
        }
    }
}

/// This module exists just to expose a variant of `call_with_result` that aborts on panic.
pub mod abort_on_panic {
    use super::*;

    /// Same `ffi_support::call_with_result`, but aborts on panic, and (as a result) doesn't require
    /// the UnwindSafe bound on the callback.
    pub fn call_with_result<R, E, F>(out_error: &mut ExternError, callback: F) -> R::Value
    where
        F: FnOnce() -> Result<R, E>,
        E: Into<ExternError>,
        R: IntoFfi,
    {
        super::call_with_result_impl(out_error, callback, true)
    }
}

