//! The storage analyzer's domain: what fills a folder, and what may be done
//! about it.
//!
//! `walk` builds a [`tree::Tree`] of one device under one folder; `empty` and
//! `duplicates` find what may go; `layout` places a folder's children in a
//! treemap; `remove` is the suite's only permanent deletion; `mounts` reads
//! which filesystems are worth offering as a starting point. Nothing here
//! knows Qt; every error names the path it concerns.

pub mod duplicates;
pub mod empty;
pub mod layout;
pub mod mounts;
pub mod remove;
pub mod tree;
pub mod walk;
