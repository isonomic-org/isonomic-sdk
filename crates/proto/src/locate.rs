// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Where a Capsule lives and where its engine listens.
//!
//! Pure functions, used by the engine to bind and by the window to connect,
//! so the two always compute the same path. Nothing here touches the disk.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Overrides the Capsule root. Must be an absolute path.
pub const ENV_CAPSULE_DIR: &str = "ISONOMIC_CAPSULE_DIR";

/// Longest socket path the engine binds or hands to a service. macOS limits
/// `sun_path` to 104 bytes including the terminator.
pub const SOCKET_PATH_MAX: usize = 103;

/// Longest name any socket, or its lock file, takes under the run directory.
pub const LONGEST_RUN_ENTRY: &str = "postgres/.s.PGSQL.5432.lock";

/// File name of the engine socket inside the run directory.
pub const ENGINE_SOCKET: &str = "engine.sock";

/// The per-user default root.
pub fn default_root(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Isonomic/Capsule")
    } else {
        home.join(".local/share/isonomic/capsule")
    }
}

/// Pick the root: an absolute override wins, else the default under `home`.
/// `None` when the override is relative or neither input is usable.
pub fn capsule_root(override_dir: Option<&OsStr>, home: Option<&Path>) -> Option<PathBuf> {
    match override_dir.map(Path::new) {
        Some(dir) if dir.is_absolute() => Some(dir.to_path_buf()),
        Some(_) => None,
        None => home.filter(|h| h.is_absolute()).map(default_root),
    }
}

/// Where sockets live: `<root>/run`, or a short per-user directory under
/// `/tmp` when the root is too deep for a socket path to fit.
pub fn run_dir(root: &Path, user_id: u32) -> PathBuf {
    let preferred = root.join("run");
    if preferred.join(LONGEST_RUN_ENTRY).as_os_str().len() <= SOCKET_PATH_MAX {
        preferred
    } else {
        PathBuf::from(format!("/tmp/isonomic-{user_id}-{:08x}", fnv1a(root)))
    }
}

/// The socket the engine's broker listens on.
pub fn engine_socket(root: &Path, user_id: u32) -> PathBuf {
    run_dir(root, user_id).join(ENGINE_SOCKET)
}

/// Stable hash of the root, so two Capsules never share a short run directory.
fn fnv1a(path: &Path) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_socket_path_is_frozen() {
        // The engine and the window must agree on these forever.
        assert_eq!(
            engine_socket(Path::new("/c"), 501),
            Path::new("/c/run/engine.sock")
        );
        let deep = format!("/{}", "d".repeat(150));
        assert_eq!(
            engine_socket(Path::new(&deep), 501),
            Path::new("/tmp/isonomic-501-44e5f59e/engine.sock")
        );
    }

    #[test]
    fn a_deep_root_gets_a_run_dir_that_fits_and_is_its_own() {
        let deep = format!("/{}", "d".repeat(150));
        let run = run_dir(Path::new(&deep), 7);
        assert!(run.join(LONGEST_RUN_ENTRY).as_os_str().len() <= SOCKET_PATH_MAX);
        assert_ne!(run, run_dir(Path::new(&format!("{deep}x")), 7));
        assert_ne!(run, run_dir(Path::new(&deep), 8));
    }

    #[test]
    fn the_root_is_the_override_or_the_default() {
        let home = Path::new("/home/u");
        assert_eq!(
            capsule_root(Some(OsStr::new("/data/x")), Some(home)),
            Some(PathBuf::from("/data/x"))
        );
        assert_eq!(capsule_root(None, Some(home)), Some(default_root(home)));
        assert_eq!(capsule_root(Some(OsStr::new("relative")), Some(home)), None);
        assert_eq!(capsule_root(None, None), None);
    }
}
