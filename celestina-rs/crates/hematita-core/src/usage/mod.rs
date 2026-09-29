//! The storage analyzer's domain: what fills a folder, and what may be done
//! about it.
//!
//! `walk` builds a [`tree::Tree`] of one device under one folder; `empty` and
//! `duplicates` find what may go; `layout` places a folder's children in a
//! treemap; `remove` is the suite's only permanent deletion; `mounts` reads
//! which filesystems are worth offering as a starting point; `view` projects
//! a scanned folder into the rows and treemap tiles a page shows, shared by
//! every consumer of the tree; `identity` tells an entry, and the mount it
//! lies on, by what the kernel reports rather than by its path. Nothing here
//! knows Qt; every error names the path it concerns.

pub mod duplicates;
pub mod empty;
mod identity;
pub mod layout;
pub mod mounts;
pub mod remove;
pub mod tree;
pub mod view;
pub mod walk;
