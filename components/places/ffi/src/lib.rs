/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate serde_json;
extern crate rusqlite;
extern crate places;
extern crate url;

#[macro_use]
extern crate log;

#[cfg(target_os = "android")]
extern crate android_logger;

use std::os::raw::c_char;
use std::ffi::{CString, CStr};
use std::ptr;
use places::PlacesDb;

use places::api::matcher::{
    search_frecent,
    SearchParams,
};

fn logging_init() {
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Filter::default().with_min_level(log::Level::Trace),
            Some("libplaces_ffi"));
        debug!("Android logging should be hooked up!")
    }
}

// XXX I'm completely punting on error handling until we have time to refactor. I'd rather not
// add more ffi error copypasta in the meantime.

/// Instantiate a places connection. Returned connection must be freed with
/// `places_connection_destroy`. Returns null and logs on errors (for now).
#[no_mangle]
pub unsafe extern "C" fn places_connection_new(
    db_path: *const c_char,
    encryption_key: *const c_char,
) -> *mut PlacesDb {
    logging_init();
    let path = c_str_to_str(db_path);
    let key = if encryption_key.is_null() {
        None
    } else {
        let s = c_str_to_str(encryption_key);
        if s == "" { None } else { Some(s) }
    };
    match PlacesDb::open(path, key) {
        Ok(state) => Box::into_raw(Box::new(state)),
        Err(e) => {
            error!("places_connection_new error: {:?}", e);
            ptr::null_mut()
        }
    }
}

fn do_note_observation(db: &mut PlacesDb, json: &str) -> places::Result<()> {
    // let obs: SerializedObservation = serde_json::from_str(json)?;
    // let visit: obs.into_visit()?;
    let visit: places::VisitObservation = serde_json::from_str(json)?;
    places::storage::apply_observation(db, visit)?;
    Ok(())
}


/// Add an observation to the database. The observation is a VisitObservation represented as JSON.
/// Errors are logged.
#[no_mangle]
pub unsafe extern "C" fn places_note_observation(
    conn: *mut PlacesDb,
    json_observation: *const c_char,
) {
    let db = &mut *conn;
    let json = c_str_to_str(json_observation);
    if let Err(e) = do_note_observation(db, json) {
        error!("places_note_observation error: {:?}", e);
    }
}

/// Execute a query, returning a `Vec<SearchResult>` as a JSON string. Returned string must be freed
/// using `places_destroy_string`. Returns null and logs on errors (for now).
#[no_mangle]
pub unsafe extern "C" fn places_query_autocomplete(
    conn: *mut PlacesDb,
    search: *const c_char,
    limit: u32,
) -> *mut c_char {
    let db = &mut *conn;
    let query = c_str_to_str(search);

    let result = search_frecent(db, SearchParams {
        search_string: query.to_owned(),
        limit,
    }).and_then(|search_results| {
        Ok(serde_json::to_string(&search_results)?)
    });

    match result {
        Ok(rust_string) => CString::new(rust_string).unwrap().into_raw(),
        Err(e) => {
            error!("places_query_autocomplete error: {:?}", e);
            ptr::null_mut()
        }
    }
}

#[inline]
unsafe fn c_str_to_str<'a>(cstr: *const c_char) -> &'a str {
    CStr::from_ptr(cstr).to_str().unwrap_or_default()
}

/// Destroy a string allocated returned in this library.
#[no_mangle]
pub unsafe extern "C" fn places_destroy_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Destroy a connection allocated by places_connection_new
#[no_mangle]
pub unsafe extern "C" fn places_connection_destroy(obj: *mut PlacesDb) {
    if !obj.is_null() {
        drop(Box::from_raw(obj));
    }
}
