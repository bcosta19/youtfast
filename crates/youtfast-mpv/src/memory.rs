//! Process allocator tuning and heap trimming for libmpv's playback buffers.
//!
//! On Linux with glibc, libmpv and FFmpeg allocate and free tens to hundreds
//! of megabytes of demuxer packets and decoded video frames across worker
//! threads created per file load. Without bounding per-thread arenas and
//! `M_MMAP_THRESHOLD`, glibc ratchets `M_MMAP_THRESHOLD` up to 32 MiB and
//! keeps freed pages resident across opens and resolution reloads.

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod imp {
    use std::ffi::c_int;
    use std::sync::Once;

    // From `<malloc.h>` on glibc Linux.
    const M_MMAP_THRESHOLD: c_int = -3;
    const M_ARENA_MAX: c_int = -8;
    // Keep large demuxer/frame allocations eligible for mmap/munmap instead of
    // letting glibc's dynamic threshold ratchet up to 32 MiB.
    const MMAP_THRESHOLD_BYTES: c_int = 128 * 1024;
    // Limit arena proliferation when libmpv/FFmpeg spawns fresh worker threads
    // on each video open or resolution switch.
    const ARENA_MAX: c_int = 2;

    unsafe extern "C" {
        fn mallopt(param: c_int, value: c_int) -> c_int;
        fn malloc_trim(pad: usize) -> c_int;
    }

    static CONFIGURED: Once = Once::new();

    pub fn configure_allocator() {
        CONFIGURED.call_once(|| {
            // Respect explicit environment overrides when the user sets them.
            if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
                // SAFETY: `M_ARENA_MAX` takes a positive arena limit.
                unsafe {
                    let _ = mallopt(M_ARENA_MAX, ARENA_MAX);
                }
            }
            if std::env::var_os("MALLOC_MMAP_THRESHOLD_").is_none() {
                // SAFETY: `M_MMAP_THRESHOLD` takes a byte threshold and disables
                // dynamic threshold growth in glibc.
                unsafe {
                    let _ = mallopt(M_MMAP_THRESHOLD, MMAP_THRESHOLD_BYTES);
                }
            }
        });
    }

    pub(crate) fn trim_heap() {
        // SAFETY: `malloc_trim(0)` is thread-safe in glibc and releases unused
        // top-of-heap and madvise-eligible pages back to the OS.
        unsafe {
            let _ = malloc_trim(0);
        }
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod imp {
    pub fn configure_allocator() {}
    pub(crate) fn trim_heap() {}
}

/// Tunes the platform C allocator (glibc on Linux) before worker threads start
/// so libmpv's playback buffers return to the OS instead of staying retained in
/// per-thread arenas. Safe to call multiple times.
pub use imp::configure_allocator;

/// Hints to the platform C allocator to release freed heap pages back to the OS.
pub(crate) use imp::trim_heap;
