#![recursion_limit = "1000"]

extern crate bytes;
extern crate hex;
#[macro_use]
extern crate log;
extern crate reproto_core as core;
extern crate ring;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;
extern crate url;

mod checksum;
mod git;
mod hex_slice;
mod index;
mod metadata;
mod objects;
mod repository;
mod resolver;
mod sha256;
mod update;

pub use self::checksum::Checksum;
pub use self::git::GitRepo;
pub use self::hex_slice::HexSlice;
pub use self::index::{index_from_path, index_from_url, init_file_index, Index, IndexConfig,
                      NoIndex};
pub use self::objects::{objects_from_path, objects_from_url, CachedObjects, FileObjects,
                        NoObjects, Objects, ObjectsConfig};
pub use self::repository::Repository;
pub use self::resolver::{Paths, Resolvers};
pub use self::sha256::{Sha256 as Digest, to_sha256 as to_checksum};
pub use self::update::Update;
