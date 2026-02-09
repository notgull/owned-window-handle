// Copyright (c) 2025 The Winit Publishers
//
// This software is release under one of the following licenses, at your option:
//
// - The MIT License
// - The Zlib License
// - The Apache License, Version 2.0

//! Take ownership of window handles passed in via [`raw-window-handle`].
//!
//! [`raw-window-handle`]: https://crates.io/crates/raw-window-handle

use core::fmt;
use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};

pub use raw_window_handle;

/// An owned equivalent of the window handle.
///
/// See [crate level documentation](crate) for more information.
pub struct OwnedWindowHandle {
    /// Underlying owned handle.
    handle: WindowHandle<'static>,
}

impl fmt::Debug for OwnedWindowHandle {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedWindowHandle").finish_non_exhaustive()
    }
}

impl OwnedWindowHandle {
    /// Create a new [`OwnedWindowHandle`] from something that implements [`HasWindowHandle`].
    #[inline]
    pub fn new(handle: impl HasWindowHandle) -> Result<Self, Error> {
        Self::_new(handle.window_handle()?)
    }

    fn _new(handle: WindowHandle<'_>) -> Result<Self, Error> {
        Ok(Self {
            handle: inc_refcount(handle)?,
        })
    }

    /// Clone this window handle.
    #[inline]
    pub fn try_clone(&self) -> Result<Self, Error> {
        Self::_new(self.handle)
    }
}

impl Drop for OwnedWindowHandle {
    fn drop(&mut self) {
        // SAFETY: Our handle was created via inc_refcount.
        let _ = unsafe { dec_refcount(self.handle) };
    }
}

impl HasWindowHandle for OwnedWindowHandle {
    #[inline]
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(self.handle)
    }
}

/// Error type for window handles.
#[derive(Debug)]
pub struct Error(Repr);

impl From<HandleError> for Error {
    #[inline]
    fn from(err: HandleError) -> Self {
        Self(Repr::Raw(err))
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Repr::Raw(HandleError::NotSupported) => write!(f, "unsupported platform"),
            Repr::Raw(HandleError::Unavailable) => write!(f, "window handle is unavailable"),
            Repr::Raw(_) => write!(f, "unknown raw window handle error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Increment reference count of the underlying handle.
fn inc_refcount(window: WindowHandle<'_>) -> Result<WindowHandle<'static>, Error> {
    let raw = match window.as_raw() {
        RawWindowHandle::Xlib(xlib) => {
            // Xlib windows are just numeric ID's and are safe to use after destruction.
            RawWindowHandle::Xlib(xlib)
        }

        RawWindowHandle::Xcb(xcb) => {
            // XCB windows are just numeric ID's and are safe to use after destruction.
            RawWindowHandle::Xcb(xcb)
        }

        RawWindowHandle::Win32(win32) => {
            // Win32 windows are ID's into a thread local table.
            // https://github.com/rust-windowing/raw-window-handle/issues/171#issuecomment-2282313064
            RawWindowHandle::Win32(win32)
        }

        RawWindowHandle::Wayland(wayland) => {
            // Wayland windows can be destroyed in safe code and are safe to
            // use even after destruction.
            //
            // TODO: I'm skeptical of this, check it later!
            RawWindowHandle::Wayland(wayland)
        }

        // Default case: platform this version of the code doesn't anticipate.
        _ => return Err(HandleError::NotSupported.into()),
    };

    // SAFETY: See above comments, this is always a valid handle.
    Ok(unsafe { WindowHandle::borrow_raw(raw) })
}

/// Decrement reference count of the underlying handle.
///
/// # Safety
///
/// `window` must have been created via [`inc_refcount`].
unsafe fn dec_refcount(window: WindowHandle<'static>) -> Result<(), Error> {
    match window.as_raw() {
        RawWindowHandle::Xlib(_) => {
            // We did nothing with the window above, so no need to do anything
            // here either.
        }

        RawWindowHandle::Xcb(_) => {
            // We did nothing with the window above, so no need to do anything
            // here either.
        }

        RawWindowHandle::Win32(_) => {
            // We did nothing with the window above, so no need to do anything
            // here either.
        }

        RawWindowHandle::Wayland(_) => {
            // We did nothing with the window above, so no need to do anything
            // here either.
        }

        // Default case: platform this version of the code doesn't anticipate.
        _ => return Err(HandleError::NotSupported.into()),
    }

    Ok(())
}

/// Possible error codes.
#[derive(Debug)]
enum Repr {
    /// Underlying [`raw-window-handle`] error.
    Raw(HandleError),
}
