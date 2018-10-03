/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[cfg(feature = "serde_support")] extern crate serde;
#[cfg(feature = "serde_support")] mod serde_support;

#[cfg(test)]
#[cfg(feature = "serde_support")]
extern crate serde_test;

#[cfg(feature = "rusqlite_support")] extern crate rusqlite;
#[cfg(feature = "rusqlite_support")] mod rusqlite_support;

use std::{fmt, str, ops};

/// This is a type intended to be used to represent the guids used by sync.
/// It has a few benefits over using a `String`:
///
/// 1. It's more explicit about what is being stored, and could prevent bugs where
///    a Guid is passed to a function expecting text.
///
/// 2. It's optimized for the guids commonly used by sync. In particular, guids that
///    meet `PlacesUtils.isValidGuid` (exposed from this library as `Guid::is_valid_for_places`)
///    do not incur any heap allocation, and are stored inline.
///
/// 3. Guaranteed immutability.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Guid(Repr);

/// The internal representation of a GUID. Most Sync GUIDs are 12 bytes,
/// and contain only base64url characters; we can store them on the stack
/// without a heap allocation. However, arbitrary ascii guids of up to length 64
/// are possible, in which case we fall back to a heap-allocated string.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum Repr {
    // TODO: We could store more strings inline... (store a byte for length and then
    // how ever many more bytes we can fit before it's as large as the string. we also
    // could loosen the base64url requirement to just require ascii).
    Fast([u8; 12]),

    // TODO: In practice, the server only allows ASCII strings of up to 64 characters
    /// (and they must be between `b' '` and `b'~'`, inclusive), so storing arbitrary
    // strings here is more forgiving than we should be...
    Slow(String),
}

const BASE64URL_BYTES: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1,
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

impl Guid {

    #[inline]
    pub fn from_str(s: &str) -> Self {
        if Self::can_use_fast(s) {
            let mut bytes = [0u8; 12];
            bytes.copy_from_slice(s.as_bytes());
            Guid(Repr::Fast(bytes))
        } else {
            Guid(Repr::Slow(s.into()))
        }
    }

    #[inline]
    pub fn try_from_bytes(b: &[u8]) -> Option<Guid> {
        if Guid::can_use_fast(b) {
            let mut bytes = [0u8; 12];
            bytes.copy_from_slice(b);
            Some(Guid(Repr::Fast(bytes)))
        } else {
            // TODO: The sync server rejects id with characters outside the
            // range ' '..='~', and IDs that are not 64 characters, we
            // probably should too...
            str::from_utf8(b).ok().map(|s| Guid(Repr::Slow(s.into())))
        }
    }

    #[inline]
    pub fn from_bytes(b: &[u8]) -> Guid {
        Guid::try_from_bytes(b).expect("Invalid UTF8 in Guid::from_bytes")
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match self.0 {
            Repr::Fast(ref bytes) => bytes,
            Repr::Slow(ref s) => s.as_ref(),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match self.0 {
            Repr::Fast(ref bytes) => {
                // This is guaranteed when constructing `Repr::Fast` -- arguably we should be using
                // `unsafe { str::from_utf8_unchecked(bytes) }`.
                str::from_utf8(bytes).unwrap()
            }
            Repr::Slow(ref s) => s,
        }
    }

    #[inline]
    pub fn into_string(self) -> String {
        match self.0 {
            Repr::Fast(ref bytes) => {
                str::from_utf8(bytes).unwrap().to_owned()
            }
            Repr::Slow(s) => s,
        }
    }

    // Equivalent to `PlacesUtils.isValidGuid`. Exposed publically as `Guid::is_valid_for_places`.
    #[inline]
    fn can_use_fast<T: ?Sized + AsRef<[u8]>>(bytes: &T) -> bool {
        let bytes = bytes.as_ref();
        bytes.len() == 12 && bytes.iter().all(|&b| BASE64URL_BYTES[b as usize] == 1)
    }

    /// Returns true for guids that have length 12, and are composed entirely of base64url
    /// characters. Equivalent to `PlacesUtils.isValidGuid`.
    #[inline]
    pub fn is_valid_for_places<T: ?Sized + AsRef<[u8]>>(bytes_or_str: &T) -> bool {
        Guid::can_use_fast(bytes_or_str.as_ref())
    }
}

impl<'a> From<&'a str> for Guid {
    #[inline]
    fn from(s: &'a str) -> Guid {
        Guid::from_str(s)
    }
}

impl<'a> From<&'a [u8]> for Guid {
    #[inline]
    fn from(s: &'a [u8]) -> Guid {
        Guid::try_from_bytes(s).unwrap()
    }
}

impl From<String> for Guid {
    #[inline]
    fn from(s: String) -> Guid {
        Guid::from(s.into_bytes())
    }
}

impl From<Vec<u8>> for Guid {
    #[inline]
    fn from(owned_bytes: Vec<u8>) -> Guid {
        if Guid::can_use_fast(&owned_bytes) {
            let mut bytes = [0u8; 12];
            bytes.copy_from_slice(owned_bytes.as_ref());
            Guid(Repr::Fast(bytes))
        } else {
            Guid(Repr::Slow(String::from_utf8(owned_bytes).unwrap()))
        }
    }
}

impl From<Guid> for String {
    #[inline]
    fn from(guid: Guid) -> String {
        guid.into_string()
    }
}

impl From<Guid> for Vec<u8> {
    #[inline]
    fn from(guid: Guid) -> Vec<u8> {
        guid.into_string().into_bytes()
    }
}

impl AsRef<str> for Guid {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for Guid {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl ops::Deref for Guid {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

// The default Debug impl is pretty unhelpful here.
impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Guid({:?})", self.as_str())
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

macro_rules! impl_guid_eq {
    ($($other: ty),+) => {$(
        impl<'a> PartialEq<$other> for Guid {
            #[inline]
            fn eq(&self, other: &$other) -> bool {
                PartialEq::eq(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }

            #[inline]
            fn ne(&self, other: &$other) -> bool {
                PartialEq::ne(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }
        }

        impl<'a> PartialEq<Guid> for $other {
            #[inline]
            fn eq(&self, other: &Guid) -> bool {
                PartialEq::eq(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }

            #[inline]
            fn ne(&self, other: &Guid) -> bool {
                PartialEq::ne(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }
        }
    )+}
}

// Implement direct comparison with some common types from the stdlib.
impl_guid_eq![
    str,  &'a str,  String,
    [u8], &'a [u8], Vec<u8>
];

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_base64url_bytes() {
        let mut expect = [0u8; 256];
        for b in b'0'..=b'9' {
            expect[b as usize] = 1;
        }
        for b in b'a'..=b'z' {
            expect[b as usize] = 1;
        }
        for b in b'A'..=b'Z' {
            expect[b as usize] = 1;
        }
        expect[b'_' as usize] = 1;
        expect[b'-' as usize] = 1;
        assert_eq!(&BASE64URL_BYTES[..], &expect[..]);
    }

    #[test]
    fn test_valid_for_places() {
        assert!(Guid::is_valid_for_places("aaaabbbbcccc"));
        assert!(Guid::is_valid_for_places(b"09_az-AZ_09-"));
        assert!(!Guid::is_valid_for_places("aaaabbbbccccd")); // too long
        assert!(!Guid::is_valid_for_places("aaaabbbbccc")); // too short
        assert!(!Guid::is_valid_for_places("aaaabbbbccc ")); // right length, bad character (ascii)
        assert!(!Guid::is_valid_for_places("aaaabbbbcccü")); // right length, bad character (unicode)
        assert!(!Guid::is_valid_for_places(b"aaaabbbbccc\xa0")); // invalid utf8
    }

    #[test]
    fn test_comparison() {
        assert_eq!(Guid::from("abcdabcdabcd"), "abcdabcdabcd");
        assert_ne!(Guid::from("abcdabcdabcd".to_string()), "ABCDabcdabcd");

        assert_eq!(Guid::from("abcdabcdabcd"), &b"abcdabcdabcd"[..]); // b"abcdabcdabcd" has type &[u8; 12]...
        assert_ne!(Guid::from(&b"abcdabcdabcd"[..]), &b"ABCDabcdabcd"[..]);

        assert_eq!(Guid::from("abcdabcdabcd".as_bytes().to_owned()), "abcdabcdabcd".to_string());
        assert_ne!(Guid::from("abcdabcdabcd"), "ABCDabcdabcd".to_string());

        assert_eq!(Guid::from("abcdabcdabcd1234"), Vec::from(b"abcdabcdabcd1234".as_ref()));
        assert_ne!(Guid::from("abcdabcdabcd4321"), Vec::from(b"ABCDabcdabcd4321".as_ref()));
    }
}
