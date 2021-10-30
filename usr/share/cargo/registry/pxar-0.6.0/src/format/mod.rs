//! *pxar* binary format definition
//!
//! Please note the all values are stored in little endian ordering.
//!
//! The Archive contains a list of items. Each item starts with a `Header`, followed by the
//! item data.
//!
//! An archive contains items in the following order:
//!  * `ENTRY`              -- containing general stat() data and related bits
//!   * `XATTR`             -- one extended attribute
//!   * ...                 -- more of these when there are multiple defined
//!   * `ACL_USER`          -- one `USER ACL` entry
//!   * ...                 -- more of these when there are multiple defined
//!   * `ACL_GROUP`         -- one `GROUP ACL` entry
//!   * ...                 -- more of these when there are multiple defined
//!   * `ACL_GROUP_OBJ`     -- The `ACL_GROUP_OBJ`
//!   * `ACL_DEFAULT`       -- The various default ACL fields if there's one defined
//!   * `ACL_DEFAULT_USER`  -- one USER ACL entry
//!   * ...                 -- more of these when multiple are defined
//!   * `ACL_DEFAULT_GROUP` -- one GROUP ACL entry
//!   * ...                 -- more of these when multiple are defined
//!   * `FCAPS`             -- file capability in Linux disk format
//!   * `QUOTA_PROJECT_ID`  -- the ext4/xfs quota project ID
//!   * `PAYLOAD`           -- file contents, if it is one
//!   * `SYMLINK`           -- symlink target, if it is one
//!   * `DEVICE`            -- device major/minor, if it is a block/char device
//!
//!   If we are serializing a directory, then this is followed by:
//!
//!   * `FILENAME`          -- name of the first directory entry (strictly ordered!)
//!   * `<archive>`         -- serialization of the first directory entry's metadata and contents,
//!                            following the exact same archive format
//!   * `FILENAME`          -- name of the second directory entry (strictly ordered!)
//!   * `<archive>`         -- serialization of the second directory entry
//!   * ...
//!   * `GOODBYE`           -- lookup table at the end of a list of directory entries

use std::cmp::Ordering;
use std::ffi::{CStr, OsStr};
use std::fmt;
use std::fmt::Display;
use std::io;
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::{Duration, SystemTime};

use endian_trait::Endian;
use siphasher::sip::SipHasher24;

pub mod acl;

// generated with:
// $ echo -n 'PROXMOX ARCHIVE FORMAT' | sha1sum | sed -re 's/^(.{16})(.{16}).*$/0x\1, 0x\2/'
pub const PXAR_HASH_KEY_1: u64 = 0x83ac3f1cfbb450db;
pub const PXAR_HASH_KEY_2: u64 = 0xaa4f1b6879369fbd;

/// While these constants correspond to `libc::S_` constants, we need these to be fixed for the
/// format itself, so we redefine them here.
///
/// Additionally this gets rid of a bunch of casts between u32 and u64.
///
/// You can usually find the values for these in `/usr/include/linux/stat.h`.
#[rustfmt::skip]
pub mod mode {
    pub const IFMT   : u64 = 0o0170000;

    pub const IFSOCK : u64 = 0o0140000;
    pub const IFLNK  : u64 = 0o0120000;
    pub const IFREG  : u64 = 0o0100000;
    pub const IFBLK  : u64 = 0o0060000;
    pub const IFDIR  : u64 = 0o0040000;
    pub const IFCHR  : u64 = 0o0020000;
    pub const IFIFO  : u64 = 0o0010000;

    pub const ISUID  : u64 = 0o0004000;
    pub const ISGID  : u64 = 0o0002000;
    pub const ISVTX  : u64 = 0o0001000;
}

/// Beginning of an entry (current version).
pub const PXAR_ENTRY: u64 = 0xd5956474e588acef;
/// Previous version of the entry struct
pub const PXAR_ENTRY_V1: u64 = 0x11da850a1c1cceff;
pub const PXAR_FILENAME: u64 = 0x16701121063917b3;
pub const PXAR_SYMLINK: u64 = 0x27f971e7dbf5dc5f;
pub const PXAR_DEVICE: u64 = 0x9fc9e906586d5ce9;
pub const PXAR_XATTR: u64 = 0x0dab0229b57dcd03;
pub const PXAR_ACL_USER: u64 = 0x2ce8540a457d55b8;
pub const PXAR_ACL_GROUP: u64 = 0x136e3eceb04c03ab;
pub const PXAR_ACL_GROUP_OBJ: u64 = 0x10868031e9582876;
pub const PXAR_ACL_DEFAULT: u64 = 0xbbbb13415a6896f5;
pub const PXAR_ACL_DEFAULT_USER: u64 = 0xc89357b40532cd1f;
pub const PXAR_ACL_DEFAULT_GROUP: u64 = 0xf90a8a5816038ffe;
pub const PXAR_FCAPS: u64 = 0x2da9dd9db5f7fb67;
pub const PXAR_QUOTA_PROJID: u64 = 0xe07540e82f7d1cbb;
/// Marks item as hardlink
pub const PXAR_HARDLINK: u64 = 0x51269c8422bd7275;
/// Marks the beginnig of the payload (actual content) of regular files
pub const PXAR_PAYLOAD: u64 = 0x28147a1b0b7c1a25;
/// Marks item as entry of goodbye table
pub const PXAR_GOODBYE: u64 = 0x2fec4fa642d5731d;
/// The end marker used in the GOODBYE object
pub const PXAR_GOODBYE_TAIL_MARKER: u64 = 0xef5eed5b753e1555;

#[derive(Debug, Endian)]
#[repr(C)]
pub struct Header {
    /// The item type (see `PXAR_` constants).
    pub htype: u64,
    /// The size of the item, including the size of `Header`.
    full_size: u64,
}

impl Header {
    #[inline]
    pub fn with_full_size(htype: u64, full_size: u64) -> Self {
        Self { htype, full_size }
    }

    #[inline]
    pub fn with_content_size(htype: u64, content_size: u64) -> Self {
        Self::with_full_size(htype, content_size + size_of::<Header>() as u64)
    }

    #[inline]
    pub fn full_size(&self) -> u64 {
        self.full_size
    }

    #[inline]
    pub fn content_size(&self) -> u64 {
        self.full_size() - (size_of::<Self>() as u64)
    }

    #[inline]
    pub fn max_content_size(&self) -> u64 {
        match self.htype {
            // + null-termination
            PXAR_FILENAME => crate::util::MAX_FILENAME_LEN + 1,
            // + null-termination
            PXAR_SYMLINK => crate::util::MAX_PATH_LEN + 1,
            // + null-termination + offset
            PXAR_HARDLINK => crate::util::MAX_PATH_LEN + 1 + (size_of::<u64>() as u64),
            PXAR_DEVICE => size_of::<Device>() as u64,
            PXAR_XATTR | PXAR_FCAPS => crate::util::MAX_XATTR_LEN,
            PXAR_ACL_USER | PXAR_ACL_DEFAULT_USER => size_of::<acl::User>() as u64,
            PXAR_ACL_GROUP | PXAR_ACL_DEFAULT_GROUP => size_of::<acl::Group>() as u64,
            PXAR_ACL_DEFAULT => size_of::<acl::Default>() as u64,
            PXAR_ACL_GROUP_OBJ => size_of::<acl::GroupObject>() as u64,
            PXAR_QUOTA_PROJID => size_of::<QuotaProjectId>() as u64,
            PXAR_ENTRY => size_of::<Stat>() as u64,
            PXAR_PAYLOAD | PXAR_GOODBYE => std::u64::MAX - (size_of::<Self>() as u64),
            _ => std::u64::MAX - (size_of::<Self>() as u64),
        }
    }

    #[inline]
    pub fn check_header_size(&self) -> io::Result<()> {
        if self.full_size() < size_of::<Header>() as u64 {
            io_bail!("invalid header {} - too small ({})", self, self.full_size());
        }

        if self.content_size() > self.max_content_size() {
            io_bail!(
                "invalid content size ({} > {}) of entry with {}",
                self.content_size(),
                self.max_content_size(),
                self
            );
        }
        Ok(())
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let readable = match self.htype {
            PXAR_FILENAME => "FILENAME",
            PXAR_SYMLINK => "SYMLINK",
            PXAR_HARDLINK => "HARDLINK",
            PXAR_DEVICE => "DEVICE",
            PXAR_XATTR => "XATTR",
            PXAR_FCAPS => "FCAPS",
            PXAR_ACL_USER => "ACL_USER",
            PXAR_ACL_DEFAULT_USER => "ACL_DEFAULT_USER",
            PXAR_ACL_GROUP => "ACL_GROUP",
            PXAR_ACL_DEFAULT_GROUP => "ACL_DEFAULT_GROUP",
            PXAR_ACL_DEFAULT => "ACL_DEFAULT",
            PXAR_ACL_GROUP_OBJ => "ACL_GROUP_OBJ",
            PXAR_QUOTA_PROJID => "QUOTA_PROJID",
            PXAR_ENTRY => "ENTRY",
            PXAR_PAYLOAD => "PAYLOAD",
            PXAR_GOODBYE => "GOODBYE",
            _ => "UNKNOWN",
        };
        write!(f, "{} header ({:x})", readable, self.htype)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignedDuration {
    Positive(Duration),
    Negative(Duration),
}

#[derive(Clone, Debug, Default, Endian, Eq, PartialEq)]
#[repr(C)]
pub struct StatxTimestamp {
    /// Seconds since the epoch (unix time).
    pub secs: i64,

    /// Nanoseconds since this struct's `secs`.
    pub nanos: u32,
}

impl From<SystemTime> for StatxTimestamp {
    fn from(time: SystemTime) -> Self {
        match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(positive) => Self::from_duration_since_epoch(positive),
            Err(negative) => Self::from_duration_before_epoch(negative.duration()),
        }
    }
}

impl StatxTimestamp {
    /// `const` version of `Default`
    pub const fn zero() -> Self {
        Self { secs: 0, nanos: 0 }
    }

    #[cfg(all(test, target_os = "linux"))]
    /// From data found in `struct stat` (`libc::stat`).
    pub fn from_stat(sec: i64, nsec: u32) -> Self {
        if sec < 0 {
            Self::from_duration_before_epoch(Duration::new((-sec) as u64, nsec))
        } else {
            Self::from_duration_since_epoch(Duration::new(sec as u64, nsec))
        }
    }

    /// Turn a positive duration relative to the unix epoch into a time stamp.
    pub fn from_duration_since_epoch(duration: Duration) -> Self {
        Self {
            secs: duration.as_secs() as i64,
            nanos: duration.subsec_nanos(),
        }
    }

    /// Turn a *negative* duration from relative to the unix epoch into a time stamp.
    pub fn from_duration_before_epoch(duration: Duration) -> Self {
        match duration.subsec_nanos() {
            0 => Self {
                secs: -(duration.as_secs() as i64),
                nanos: 0,
            },
            nanos => Self {
                secs: -(duration.as_secs() as i64) - 1,
                nanos: 1_000_000_000 - nanos,
            },
        }
    }

    /// Get the duration since the epoch. an `Ok` value is a positive duration, an `Err` value is a
    /// negative duration.
    pub fn to_duration(&self) -> SignedDuration {
        if self.secs >= 0 {
            SignedDuration::Positive(Duration::new(self.secs as u64, self.nanos))
        } else {
            // this handles the nanos=0 case as `Duration::new()` performs the carry-over.
            SignedDuration::Negative(Duration::new(
                -(self.secs + 1) as u64,
                1_000_000_000 - self.nanos,
            ))
        }
    }

    /// Get a `std::time::SystemTime` from this time stamp.
    pub fn system_time(&self) -> SystemTime {
        match self.to_duration() {
            SignedDuration::Positive(positive) => SystemTime::UNIX_EPOCH + positive,
            SignedDuration::Negative(negative) => SystemTime::UNIX_EPOCH - negative,
        }
    }
}

#[test]
fn test_statx_timestamp() {
    const MAY_1_2015_1530: i64 = 1430487000;
    let system_time = SystemTime::UNIX_EPOCH + Duration::new(MAY_1_2015_1530 as u64, 1_000_000);
    let tx = StatxTimestamp::from(system_time);
    assert_eq!(
        tx,
        StatxTimestamp {
            secs: MAY_1_2015_1530,
            nanos: 1_000_000,
        }
    );
    assert_eq!(tx.system_time(), system_time);

    const MAY_1_1960_1530: i64 = -305112600;
    let system_time = SystemTime::UNIX_EPOCH - Duration::new(-MAY_1_1960_1530 as u64, 1_000_000);
    let tx = StatxTimestamp::from(system_time);
    assert_eq!(
        tx,
        StatxTimestamp {
            secs: MAY_1_1960_1530 - 1,
            nanos: 999_000_000,
        }
    );
    assert_eq!(tx.system_time(), system_time);

    let system_time = SystemTime::UNIX_EPOCH - Duration::new(-MAY_1_1960_1530 as u64, 0);
    let tx = StatxTimestamp::from(system_time);
    assert_eq!(
        tx,
        StatxTimestamp {
            secs: MAY_1_1960_1530,
            nanos: 0,
        }
    );
    assert_eq!(tx.system_time(), system_time);
}
#[derive(Clone, Debug, Default, Endian)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[repr(C)]
pub struct Stat_V1 {
    pub mode: u64,
    pub flags: u64,
    pub uid: u32,
    pub gid: u32,
    pub mtime: u64,
}

impl Into<Stat> for Stat_V1 {
    fn into(self) -> Stat {
        Stat {
            mode: self.mode,
            flags: self.flags,
            uid: self.uid,
            gid: self.gid,
            mtime: StatxTimestamp::from_duration_since_epoch(Duration::from_nanos(self.mtime)),
        }
    }
}

#[derive(Clone, Debug, Default, Endian)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[repr(C)]
pub struct Stat {
    pub mode: u64,
    pub flags: u64,
    pub uid: u32,
    pub gid: u32,
    pub mtime: StatxTimestamp,
}

/// Builder pattern methods.
impl Stat {
    pub const fn mode(self, mode: u64) -> Self {
        Self { mode, ..self }
    }

    pub const fn flags(self, flags: u64) -> Self {
        Self { flags, ..self }
    }

    pub const fn uid(self, uid: u32) -> Self {
        Self { uid, ..self }
    }

    pub const fn gid(self, gid: u32) -> Self {
        Self { gid, ..self }
    }

    pub const fn mtime(self, mtime: StatxTimestamp) -> Self {
        Self { mtime, ..self }
    }

    pub const fn set_dir(self) -> Self {
        let mode = self.mode;
        self.mode((mode & !mode::IFMT) | mode::IFDIR)
    }

    pub const fn set_regular_file(self) -> Self {
        let mode = self.mode;
        self.mode((mode & !mode::IFMT) | mode::IFREG)
    }

    pub const fn set_symlink(self) -> Self {
        let mode = self.mode;
        self.mode((mode & !mode::IFMT) | mode::IFLNK)
    }

    pub const fn set_blockdev(self) -> Self {
        let mode = self.mode;
        self.mode((mode & !mode::IFMT) | mode::IFBLK)
    }

    pub const fn set_chardev(self) -> Self {
        let mode = self.mode;
        self.mode((mode & !mode::IFMT) | mode::IFCHR)
    }

    pub const fn set_fifo(self) -> Self {
        let mode = self.mode;
        self.mode((mode & !mode::IFMT) | mode::IFIFO)
    }
}

/// Convenience accessor methods.
impl Stat {
    /// Get the mtime as duration since the epoch.
    pub fn mtime_as_duration(&self) -> SignedDuration {
        self.mtime.to_duration()
    }

    /// Get the file type portion of the mode bitfield.
    pub fn get_file_bits(&self) -> u64 {
        self.mode & mode::IFMT
    }

    /// Get the permission portion of the mode bitfield.
    pub fn get_permission_bits(&self) -> u64 {
        self.mode & !mode::IFMT
    }
}

/// Convenience methods.
impl Stat {
    /// Get the file type (`mode & mode::IFMT`).
    pub fn file_type(&self) -> u64 {
        self.mode & mode::IFMT
    }

    /// Get the file mode bits (`mode & !mode::IFMT`).
    pub fn file_mode(&self) -> u64 {
        self.mode & !mode::IFMT
    }

    /// Check whether this is a directory.
    pub fn is_dir(&self) -> bool {
        (self.mode & mode::IFMT) == mode::IFDIR
    }

    /// Check whether this is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        (self.mode & mode::IFMT) == mode::IFLNK
    }

    /// Check whether this is a device node.
    pub fn is_device(&self) -> bool {
        let fmt = self.mode & mode::IFMT;
        fmt == mode::IFCHR || fmt == mode::IFBLK
    }

    /// Check whether this is a block device node.
    pub fn is_blockdev(&self) -> bool {
        let fmt = self.mode & mode::IFMT;
        fmt == mode::IFBLK
    }

    /// Check whether this is a character device node.
    pub fn is_chardev(&self) -> bool {
        let fmt = self.mode & mode::IFMT;
        fmt == mode::IFCHR
    }

    /// Check whether this is a regular file.
    pub fn is_regular_file(&self) -> bool {
        (self.mode & mode::IFMT) == mode::IFREG
    }

    /// Check whether this is a named pipe (FIFO).
    pub fn is_fifo(&self) -> bool {
        (self.mode & mode::IFMT) == mode::IFIFO
    }

    /// Check whether this is a named socket.
    pub fn is_socket(&self) -> bool {
        (self.mode & mode::IFMT) == mode::IFSOCK
    }
}

impl From<&std::fs::Metadata> for Stat {
    fn from(meta: &std::fs::Metadata) -> Stat {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let this = Stat::default();

        #[cfg(unix)]
        let this = this
            .uid(meta.uid())
            .gid(meta.gid())
            .mode(meta.mode() as u64);

        let this = match meta.modified() {
            Ok(mtime) => this.mtime(mtime.into()),
            Err(_) => this,
        };

        let file_type = meta.file_type();
        let mode = this.mode;
        if file_type.is_dir() {
            this.mode(mode | mode::IFDIR)
        } else if file_type.is_symlink() {
            this.mode(mode | mode::IFLNK)
        } else {
            this.mode(mode | mode::IFREG)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Filename {
    pub name: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Symlink {
    pub data: Vec<u8>,
}

impl Symlink {
    pub fn as_os_str(&self) -> &OsStr {
        self.as_ref()
    }
}

impl AsRef<[u8]> for Symlink {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl AsRef<OsStr> for Symlink {
    fn as_ref(&self) -> &OsStr {
        OsStr::from_bytes(&self.data[..self.data.len().max(1) - 1])
    }
}

#[derive(Clone, Debug)]
pub struct Hardlink {
    pub offset: u64,
    pub data: Vec<u8>,
}

impl Hardlink {
    pub fn as_os_str(&self) -> &OsStr {
        self.as_ref()
    }
}

impl AsRef<[u8]> for Hardlink {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl AsRef<OsStr> for Hardlink {
    fn as_ref(&self) -> &OsStr {
        OsStr::from_bytes(&self.data[..self.data.len().max(1) - 1])
    }
}

#[derive(Clone, Debug, Eq)]
#[repr(C)]
pub struct XAttr {
    pub(crate) data: Vec<u8>,
    pub(crate) name_len: usize,
}

impl XAttr {
    pub fn new<N: AsRef<[u8]>, V: AsRef<[u8]>>(name: N, value: V) -> Self {
        let name = name.as_ref();
        let value = value.as_ref();
        let mut data = Vec::with_capacity(name.len() + value.len() + 1);
        data.extend(name);
        data.push(0);
        data.extend(value);
        Self {
            data,
            name_len: name.len(),
        }
    }

    pub fn name(&self) -> &CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(&self.data[..self.name_len + 1]) }
    }

    pub fn value(&self) -> &[u8] {
        &self.data[(self.name_len + 1)..]
    }
}

impl Ord for XAttr {
    fn cmp(&self, other: &XAttr) -> Ordering {
        self.name().cmp(&other.name())
    }
}

impl PartialOrd for XAttr {
    fn partial_cmp(&self, other: &XAttr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for XAttr {
    fn eq(&self, other: &XAttr) -> bool {
        self.name() == other.name()
    }
}

#[derive(Clone, Debug, Endian, Eq, PartialEq)]
#[repr(C)]
pub struct Device {
    pub major: u64,
    pub minor: u64,
}

#[cfg(target_os = "linux")]
impl Device {
    /// Get a `dev_t` value for this device.
    #[rustfmt::skip]
    pub fn to_dev_t(&self) -> u64 {
        // see bits/sysmacros.h
        ((self.major & 0x0000_0fff) << 8) |
        ((self.major & 0xffff_f000) << 32) |
         (self.minor & 0x0000_00ff) |
        ((self.minor & 0xffff_ff00) << 12)
    }

    /// Get a `Device` from a `dev_t` value.
    #[rustfmt::skip]
    pub fn from_dev_t(dev: u64) -> Self {
        // see to_dev_t
        Self {
            major: (dev >>  8) & 0x0000_0fff |
                   (dev >> 32) & 0xffff_f000,
            minor:  dev        & 0x0000_00ff |
                   (dev >> 12) & 0xffff_ff00,
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[test]
fn test_linux_devices() {
    let c_dev = unsafe { ::libc::makedev(0xabcd_1234, 0xdcba_5678) };
    let dev = Device::from_dev_t(c_dev);
    assert_eq!(dev.to_dev_t(), c_dev);
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[repr(C)]
pub struct FCaps {
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Endian, Eq, PartialEq)]
#[repr(C)]
pub struct QuotaProjectId {
    pub projid: u64,
}

#[derive(Clone, Debug, Endian)]
#[repr(C)]
pub struct GoodbyeItem {
    /// SipHash24 of the directory item name. The last GOODBYE item uses the special hash value
    /// `PXAR_GOODBYE_TAIL_MARKER`.
    pub hash: u64,

    /// The offset from the start of the GOODBYE object to the start of the matching directory item
    /// (point to a FILENAME). The last GOODBYE item points to the start of the matching ENTRY
    /// object.
    pub offset: u64,

    /// The overall size of the directory item. This includes the FILENAME header. In other words,
    /// `goodbye_start - offset + size` points to the end of the directory.
    ///
    /// The last GOODBYE item repeats the size of the GOODBYE item.
    pub size: u64,
}

impl GoodbyeItem {
    pub fn new(name: &[u8], offset: u64, size: u64) -> Self {
        let hash = hash_filename(name);
        Self { hash, offset, size }
    }
}

pub fn hash_filename(name: &[u8]) -> u64 {
    use std::hash::Hasher;

    let mut hasher = SipHasher24::new_with_keys(PXAR_HASH_KEY_1, PXAR_HASH_KEY_2);
    hasher.write(name);
    hasher.finish()
}

pub fn path_is_legal_component(path: &Path) -> bool {
    let mut components = path.components();
    match components.next() {
        Some(std::path::Component::Normal(_)) => (),
        _ => return false,
    }
    components.next().is_none()
}

pub fn check_file_name(path: &Path) -> io::Result<()> {
    if !path_is_legal_component(path) {
        io_bail!("invalid file name in archive: {:?}", path);
    } else {
        Ok(())
    }
}