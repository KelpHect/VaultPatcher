//! Game-agnostic plumbing: ini editing, detection, backups, binary patching,
//! downloads and install records.

pub mod backup;
pub mod binpatch;
pub mod detect;
pub mod display;
pub mod gpu;
pub mod ini;
pub mod manifest;
pub mod net;
pub mod winshot;
