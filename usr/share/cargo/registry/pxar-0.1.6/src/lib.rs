//! Proxmox backup archive format handling.
//!
//! This implements a reader and writer for the proxmox archive format (.pxar).

use std::ffi::OsStr;
use std::mem;
use std::path::{Path, PathBuf};

#[macro_use]
mod macros;

pub mod format;

pub(crate) mod util;

mod poll_fn;

pub mod accessor;
pub mod binary_tree_array;
pub mod decoder;
pub mod encoder;

/// Reexport of `format::Entry`. Since this conveys mostly information found via the `stat` syscall
/// we mostly use this name for public interfaces.
#[doc(inline)]
pub use format::Entry as Stat;

#[doc(inline)]
pub use format::mode;

/// File metadata found in pxar archives.
///
/// This includes the usual data you'd get from `stat()` as well as ACLs, extended attributes, file
/// capabilities and more.
#[derive(Clone, Debug, Default)]
pub struct Metadata {
    /// Data typically found in a `stat()` call.
    pub stat: Stat,

    /// Extended attributes.
    pub xattrs: Vec<format::XAttr>,

    /// ACLs.
    pub acl: Acl,

    /// File capabilities.
    pub fcaps: Option<format::FCaps>,

    /// Quota project id.
    pub quota_project_id: Option<format::QuotaProjectId>,
}

impl From<Stat> for Metadata {
    fn from(stat: Stat) -> Self {
        Self {
            stat,
            ..Default::default()
        }
    }
}

impl From<&std::fs::Metadata> for Metadata {
    fn from(meta: &std::fs::Metadata) -> Metadata {
        let this = Self::from(Stat::from(meta));

        // FIXME: fill the remaining metadata

        this
    }
}

impl From<std::fs::Metadata> for Metadata {
    fn from(meta: std::fs::Metadata) -> Metadata {
        Self::from(&meta)
    }
}

/// Convenience helpers.
impl Metadata {
    /// Get the file type (`mode & mode::IFMT`).
    #[inline]
    pub fn file_type(&self) -> u64 {
        self.stat.file_type()
    }

    /// Get the file mode bits (`mode & !mode::IFMT`).
    #[inline]
    pub fn file_mode(&self) -> u64 {
        self.stat.file_mode()
    }

    /// Check whether this is a directory.
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.stat.is_dir()
    }

    /// Check whether this is a symbolic link.
    #[inline]
    pub fn is_symlink(&self) -> bool {
        self.stat.is_symlink()
    }

    /// Check whether this is a device node.
    #[inline]
    pub fn is_device(&self) -> bool {
        self.stat.is_device()
    }

    /// Check whether this is a regular file.
    #[inline]
    pub fn is_regular_file(&self) -> bool {
        self.stat.is_regular_file()
    }

    /// Check whether this is a named pipe (FIFO).
    #[inline]
    pub fn is_fifo(&self) -> bool {
        self.stat.is_fifo()
    }

    /// Check whether this is a named socket.
    #[inline]
    pub fn is_socket(&self) -> bool {
        self.stat.is_socket()
    }

    /// Get the mtime as duration since the epoch.
    pub fn mtime_as_duration(&self) -> std::time::Duration {
        self.stat.mtime_as_duration()
    }
}

/// ACL entries of a pxar archive.
///
/// This contains all the various ACL entry types supported by the pxar archive format.
#[derive(Clone, Debug, Default)]
pub struct Acl {
    /// User ACL list.
    pub users: Vec<format::acl::User>,

    /// Group ACL list.
    pub groups: Vec<format::acl::Group>,

    /// Group object ACL entry.
    pub group_obj: Option<format::acl::GroupObject>,

    /// Default permissions.
    pub default: Option<format::acl::Default>,

    /// Default user permissions.
    pub default_users: Vec<format::acl::User>,

    /// Default group permissions.
    pub default_groups: Vec<format::acl::Group>,
}

impl Acl {
    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
            && self.groups.is_empty()
            && self.group_obj.is_none()
            && self.default.is_none()
            && self.default_users.is_empty()
            && self.default_groups.is_empty()
    }
}

/// Pxar archive entry kind.
///
/// Identifies whether the entry is a file, symlink, directory, etc.
#[derive(Clone, Debug)]
pub enum EntryKind {
    /// Symbolic links.
    Symlink(format::Symlink),

    /// Hard links, relative to the root of the current archive.
    Hardlink(format::Hardlink),

    /// Device node.
    Device(format::Device),

    /// Named unix socket.
    Socket,

    /// Named pipe.
    Fifo,

    /// Regular file.
    File { size: u64, offset: Option<u64> },

    /// Directory entry. When iterating through an archive, the contents follow next.
    Directory,

    /// End of a directory. This is for internal use to remember the goodbye-table of a directory
    /// entry. Will not occur during normal iteration.
    GoodbyeTable,
}

/// A pxar archive entry. This contains the current path, file metadata and entry type specific
/// information.
#[derive(Clone, Debug)]
pub struct Entry {
    path: PathBuf,
    metadata: Metadata,
    kind: EntryKind,
}

/// General accessors.
impl Entry {
    /// Clear everything except for the path.
    fn clear_data(&mut self) {
        self.metadata = Metadata::default();
        self.kind = EntryKind::GoodbyeTable;
    }

    fn internal_default() -> Self {
        Self {
            path: PathBuf::default(),
            metadata: Metadata::default(),
            kind: EntryKind::GoodbyeTable,
        }
    }

    fn take(&mut self) -> Self {
        let this = mem::replace(self, Self::internal_default());
        self.path = this.path.clone();
        this
    }

    /// Get the full path of this file within the current pxar directory structure.
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Convenience method to get just the file name portion of the current path.
    #[inline]
    pub fn file_name(&self) -> &OsStr {
        self.path.file_name().unwrap_or(OsStr::new(""))
    }

    /// Get the file metadata.
    #[inline]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Take out the metadata.
    #[inline]
    pub fn into_metadata(self) -> Metadata {
        self.metadata
    }

    /// Get the entry-type specific information.
    pub fn kind(&self) -> &EntryKind {
        &self.kind
    }

    /// Get the value of the symbolic link if it is one.
    pub fn get_symlink(&self) -> Option<&OsStr> {
        match &self.kind {
            EntryKind::Symlink(link) => Some(link.as_ref()),
            _ => None,
        }
    }

    /// Get the value of the hard link if it is one.
    pub fn get_hardlink(&self) -> Option<&OsStr> {
        match &self.kind {
            EntryKind::Hardlink(link) => Some(link.as_ref()),
            _ => None,
        }
    }

    /// Get the value of the device node if it is one.
    pub fn get_device(&self) -> Option<format::Device> {
        match &self.kind {
            EntryKind::Device(dev) => Some(dev.clone()),
            _ => None,
        }
    }
}

/// Convenience helpers.
impl Entry {
    /// Check whether this is a directory.
    pub fn is_dir(&self) -> bool {
        match self.kind {
            EntryKind::Directory { .. } => true,
            _ => false,
        }
    }

    /// Check whether this is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        match self.kind {
            EntryKind::Symlink(_) => true,
            _ => false,
        }
    }

    /// Check whether this is a hard link.
    pub fn is_hardlink(&self) -> bool {
        match self.kind {
            EntryKind::Hardlink(_) => true,
            _ => false,
        }
    }

    /// Check whether this is a device node.
    pub fn is_device(&self) -> bool {
        match self.kind {
            EntryKind::Device(_) => true,
            _ => false,
        }
    }

    /// Check whether this is a regular file.
    pub fn is_regular_file(&self) -> bool {
        match self.kind {
            EntryKind::File { .. } => true,
            _ => false,
        }
    }

    /// Get the file size if this is a regular file, or `None`.
    pub fn file_size(&self) -> Option<u64> {
        match self.kind {
            EntryKind::File { size, .. } => Some(size),
            _ => None,
        }
    }
}
