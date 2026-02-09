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
use raw_window_handle::{HandleError, HasWindowHandle, WindowHandle};

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
    match window.as_raw() {
        // Default case: platform this version of the code doesn't anticipate.
        _ => Err(HandleError::NotSupported.into()),
    }
}

/// Decrement reference count of the underlying handle.
///
/// # Safety
///
/// `window` must have been created via [`inc_refcount`].
unsafe fn dec_refcount(window: WindowHandle<'static>) -> Result<(), Error> {
    match window.as_raw() {
        // Default case: platform this version of the code doesn't anticipate.
        _ => Err(HandleError::NotSupported.into()),
    }
}

/// Possible error codes.
#[derive(Debug)]
enum Repr {
    /// Underlying [`raw-window-handle`] error.
    Raw(HandleError),
}
