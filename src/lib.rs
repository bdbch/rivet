#![deny(clippy::all)]

use napi_derive::napi;

pub mod bump;
pub mod changelog;
pub mod cli;
pub mod config;
pub mod error;
pub mod package_json;
pub mod release_file;
pub mod version_bump;
pub mod workspace;

#[napi]
pub fn plus_100(input: u32) -> u32 {
  input + 100
}
