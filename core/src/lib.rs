#![recursion_limit = "1000"]

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate error_chain;
extern crate mime as extern_mime;
extern crate num;
extern crate serde;
extern crate relative_path;
pub extern crate semver;

mod as_loc;
mod error_pos;
mod for_each_loc;
mod loc;
mod merge;
mod mime;
mod models;
mod object;
mod option_entry;
mod options;
mod pos;
mod rp_modifier;
mod rp_number;
mod rp_package;
mod rp_required_package;
mod rp_versioned_package;
mod with_pos;
mod manifest;
pub mod errors;

pub use self::error_pos::ErrorPos;
pub use self::for_each_loc::ForEachLoc;
pub use self::loc::Loc;
pub use self::manifest::{FileManifest, Manifest, load_manifest};
pub use self::merge::Merge;
pub use self::mime::Mime;
pub use self::models::*;
pub use self::object::{BytesObject, Object, PathObject};
pub use self::option_entry::OptionEntry;
pub use self::options::Options;
pub use self::pos::Pos;
pub use self::rp_modifier::RpModifier;
pub use self::rp_number::RpNumber;
pub use self::rp_package::RpPackage;
pub use self::rp_required_package::RpRequiredPackage;
pub use self::rp_versioned_package::RpVersionedPackage;
pub use self::with_pos::WithPos;
pub use semver::{Version, VersionReq};
