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
    /// Underlying implementation.
    imp: Impl,
}

/// Underlying implementation.
enum Impl {
    /// Static window that can be refcounted.
    ///
    /// Every backend except for Wayland uses this.
    Direct(WindowHandle<'static>),

    /// Direct Wayland object ID.
    Wayland(wayland::WaylandHandle),
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
            imp: inc_refcount(handle)?,
        })
    }

    /// Clone this window handle.
    #[inline]
    pub fn try_clone(&self) -> Result<Self, Error> {
        match &self.imp {
            Impl::Direct(handle) => {
                // Just increment refcount on the handle.
                Self::_new(*handle)
            }

            Impl::Wayland(wayland) => {
                // wayland-backend's objects can be cheaply cloned.
                Ok(Self {
                    imp: Impl::Wayland(wayland.clone()),
                })
            }
        }
    }
}

impl Drop for OwnedWindowHandle {
    fn drop(&mut self) {
        if let Impl::Direct(handle) = self.imp {
            // SAFETY: Our handle was created via inc_refcount.
            let _result = unsafe { dec_refcount(handle) };

            #[cfg(debug_assertions)]
            _result.unwrap();
        }
    }
}

impl HasWindowHandle for OwnedWindowHandle {
    #[inline]
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        match &self.imp {
            Impl::Direct(handle) => Ok(*handle),
            Impl::Wayland(wayland) => wayland::as_ptr(wayland),
        }
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
            Repr::CanvasNotFound(id) => write!(f, "canvas not found with id: {}", id),
            Repr::MissingWebElements => write!(f, "missing web elements"),
            Repr::PlatformMismatch { expected } => {
                write!(f, "platform mismatch, expected: {}", expected)
            }
            Repr::RetainFailed => write!(f, "failed to retain window handle"),
            Repr::WaylandNotEnabled => write!(f, "Wayland is not enabled"),
            Repr::WaylandNotRust => write!(
                f,
                "the resulting Wayland handle was not created by Rust's `wayland-backend`"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Increment reference count of the underlying handle.
fn inc_refcount(window: WindowHandle<'_>) -> Result<Impl, Error> {
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
            // Wayland windows need to be tracked by wayland-backend.
            return Ok(Impl::Wayland(unsafe { wayland::clone_handle(wayland) }?));
        }

        RawWindowHandle::Drm(drm) => {
            // DRM planes are just numeric ID's and are safe to use after destruction.
            RawWindowHandle::Drm(drm)
        }

        #[cfg(not(target_os = "android"))]
        RawWindowHandle::AndroidNdk(_) => {
            return Err(Error(Repr::PlatformMismatch {
                expected: "android",
            }))
        }

        #[cfg(target_os = "android")]
        RawWindowHandle::AndroidNdk(android) => {
            // Use ANativeWindow_acquire to bump the reference count.
            // SAFETY: `android` is a valid pointer to an `ANativeWindow`.
            unsafe { ndk_sys::ANativeWindow_acquire(android.a_native_window.as_ptr().cast()) };

            RawWindowHandle::AndroidNdk(android)
        }

        #[cfg(not(target_vendor = "apple"))]
        RawWindowHandle::AppKit(_) | RawWindowHandle::UiKit(_) => {
            return Err(Error(Repr::PlatformMismatch { expected: "apple" }))
        }

        #[cfg(target_vendor = "apple")]
        RawWindowHandle::AppKit(appkit) => {
            use core::ptr::NonNull;
            use objc2::runtime::NSObject;

            // Use the "retain" message to retain the object.
            // SAFETY: We know this is a valid `NSView`.
            let view: *mut NSObject = appkit.ns_view.as_ptr().cast();
            let view: *mut NSObject = unsafe { objc2::msg_send![view, retain] };

            RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new({
                NonNull::new(view).ok_or(Error(Repr::RetainFailed))?.cast()
            }))
        }

        #[cfg(target_vendor = "apple")]
        RawWindowHandle::UiKit(uikit) => {
            use core::ptr::NonNull;
            use objc2::runtime::NSObject;

            // Use the "retain" message to retain the object.
            // SAFETY: We know this is a valid `UiView`.
            let view: *mut NSObject = uikit.ui_view.as_ptr().cast();
            let view: *mut NSObject = unsafe { objc2::msg_send![view, retain] };

            RawWindowHandle::UiKit(raw_window_handle::UiKitWindowHandle::new({
                NonNull::new(view).ok_or(Error(Repr::RetainFailed))?.cast()
            }))
        }

        #[cfg(not(target_family = "wasm"))]
        RawWindowHandle::Web(_)
        | RawWindowHandle::WebCanvas(_)
        | RawWindowHandle::WebOffscreenCanvas(_) => {
            return Err(Error(Repr::PlatformMismatch { expected: "wasm" }))
        }

        #[cfg(target_family = "wasm")]
        RawWindowHandle::Web(web) => {
            use wasm_bindgen::convert::IntoWasmAbi;

            // Grab the current document.
            let document = web_sys::window()
                .ok_or(Error(Repr::MissingWebElements))?
                .document()
                .ok_or(Error(Repr::MissingWebElements))?;

            // Grab the element from its data segment.
            let canvas: web_sys::Element = document
                .query_selector(&format!("canvas[data-raw-handle=\"{}\"", web.id))
                // `querySelector` only throws an error if the selector is invalid.
                .unwrap()
                .ok_or(Error(Repr::CanvasNotFound(web.id)))?;

            // The refcount is already bumped by query_selector, convert it down.
            RawWindowHandle::WebCanvas(raw_window_handle::WebCanvasWindowHandle::new(
                canvas.into_abi() as usize,
            ))
        }

        #[cfg(target_family = "wasm")]
        RawWindowHandle::WebCanvas(web) => {
            use wasm_bindgen::convert::{IntoWasmAbi, RefFromWasmAbi};

            // Get the underlying canvas.
            // SAFETY: Guaranteed to be a valid `HtmlCanvasElement`.
            let canvas = unsafe { web_sys::HtmlCanvasElement::ref_from_abi(web.obj as _) };

            // Clone the underlying JS object so we own it.
            let canvas = (&*canvas).clone();

            // The refcount is already bumped by query_selector, convert it down.
            RawWindowHandle::WebCanvas(raw_window_handle::WebCanvasWindowHandle::new(
                canvas.into_abi() as usize,
            ))
        }

        #[cfg(target_family = "wasm")]
        RawWindowHandle::WebOffscreenCanvas(web) => {
            use wasm_bindgen::convert::{IntoWasmAbi, RefFromWasmAbi};

            // Get the underlying canvas.
            // SAFETY: Guaranteed to be a valid `OffscreenCanvas`.
            let canvas = unsafe { web_sys::OffscreenCanvas::ref_from_abi(web.obj as _) };

            // Clone the underlying JS object so we own it.
            let canvas = (&*canvas).clone();

            // The refcount is already bumped by query_selector, convert it down.
            RawWindowHandle::WebOffscreenCanvas(
                raw_window_handle::WebOffscreenCanvasWindowHandle::new(canvas.into_abi() as usize),
            )
        }

        // Default case: platform this version of the code doesn't anticipate.
        _ => return Err(HandleError::NotSupported.into()),
    };

    // SAFETY: See above comments, this is always a valid handle.
    Ok(Impl::Direct(unsafe { WindowHandle::borrow_raw(raw) }))
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

        RawWindowHandle::Wayland(_) => unreachable!("inc_refcount never creates this variant"),

        RawWindowHandle::Drm(_) => {
            // We did nothing with the window above, so no need to do anything
            // here either.
        }

        #[cfg(not(target_os = "android"))]
        RawWindowHandle::AndroidNdk(_) => {
            return Err(Error(Repr::PlatformMismatch {
                expected: "android",
            }))
        }

        #[cfg(target_os = "android")]
        RawWindowHandle::AndroidNdk(android) => {
            // Use ANativeWindow_acquire to bump the reference count.
            // SAFETY: `android` is a valid pointer to an `ANativeWindow`.
            unsafe { ndk_sys::ANativeWindow_release(android.a_native_window.as_ptr().cast()) };
        }

        #[cfg(not(target_vendor = "apple"))]
        RawWindowHandle::AppKit(_) | RawWindowHandle::UiKit(_) => {
            return Err(Error(Repr::PlatformMismatch { expected: "apple" }))
        }

        #[cfg(target_vendor = "apple")]
        RawWindowHandle::AppKit(appkit) => {
            use objc2::runtime::NSObject;

            // Use the "release" message to release the object.
            // SAFETY: We know this is a valid `NsView`.
            let view: *mut NSObject = appkit.ns_view.as_ptr().cast();
            let _: () = unsafe { objc2::msg_send![view, release] };
        }

        #[cfg(target_vendor = "apple")]
        RawWindowHandle::UiKit(uikit) => {
            use objc2::runtime::NSObject;

            // Use the "release" message to release the object.
            // SAFETY: We know this is a valid `UiView`.
            let view: *mut NSObject = uikit.ui_view.as_ptr().cast();
            let _: () = unsafe { objc2::msg_send![view, release] };
        }

        RawWindowHandle::Web(_) => unreachable!("inc_refcount never constructs this variant"),

        #[cfg(not(target_family = "wasm"))]
        RawWindowHandle::WebCanvas(_) | RawWindowHandle::WebOffscreenCanvas(_) => {
            return Err(Error(Repr::PlatformMismatch { expected: "wasm" }))
        }

        #[cfg(target_family = "wasm")]
        RawWindowHandle::WebCanvas(web) => {
            use wasm_bindgen::convert::FromWasmAbi;

            // We created a new object here. Drop it.
            // SAFETY: This is a valid, owned object as constructed above.
            let canvas = unsafe { web_sys::HtmlCanvasElement::from_abi(web.obj as _) };
            drop(canvas);
        }

        #[cfg(target_family = "wasm")]
        RawWindowHandle::WebOffscreenCanvas(web) => {
            use wasm_bindgen::convert::FromWasmAbi;

            // We created a new object here. Drop it.
            // SAFETY: This is a valid, owned object as constructed above.
            let canvas = unsafe { web_sys::OffscreenCanvas::from_abi(web.obj as _) };
            drop(canvas);
        }

        // Default case: platform this version of the code doesn't anticipate.
        _ => return Err(HandleError::NotSupported.into()),
    }

    Ok(())
}

/// Possible error codes.
#[allow(dead_code)]
#[derive(Debug)]
enum Repr {
    /// Underlying [`raw-window-handle`] error.
    Raw(HandleError),

    /// This is the wrong platform to use this function.
    PlatformMismatch {
        /// The platform we expected.
        expected: &'static str,
    },

    /// Crucial elements are missing on web.
    MissingWebElements,

    /// Canvas not found with the specific ID.
    CanvasNotFound(u32),

    /// Retain failed.
    RetainFailed,

    /// Wayland is not enabled.
    WaylandNotEnabled,

    /// The resulting Wayland handle was not created by Rust's `wayland-backend`.
    WaylandNotRust,
}

#[cfg(any(
    not(feature = "wayland"),
    not(all(
        unix,
        not(any(
            target_os = "redox",
            target_family = "wasm",
            target_os = "android",
            target_vendor = "apple"
        ))
    ))
))]
mod wayland {
    /// Wayland handle.
    pub(super) type WaylandHandle = core::convert::Infallible;

    /// Create a new `WaylandHandle` from the raw wayland handle.
    pub(super) unsafe fn clone_handle(
        _handle: raw_window_handle::WaylandWindowHandle,
    ) -> Result<WaylandHandle, crate::Error> {
        Err(crate::Error(crate::Repr::WaylandNotEnabled))
    }

    /// Convert the `WaylandHandle` into a window handle.
    pub(super) fn as_ptr(
        handle: &WaylandHandle,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        match *handle {}
    }
}

#[cfg(all(
    feature = "wayland",
    unix,
    not(any(
        target_os = "redox",
        target_family = "wasm",
        target_os = "android",
        target_vendor = "apple"
    ))
))]
mod wayland {
    use wayland_backend::sys::client as wc;
    use wayland_client::Proxy;

    /// Tracked Wayland handle.
    #[derive(Clone)]
    pub(super) struct WaylandHandle {
        /// The Wayland backend.
        backend: wc::Backend,

        /// The Wayland object ID.
        id: wc::ObjectId,
    }

    /// Get a `WaylandHandle` from a `*mut wl_proxy`.
    pub(super) unsafe fn clone_handle(
        handle: raw_window_handle::WaylandWindowHandle,
    ) -> Result<WaylandHandle, crate::Error> {
        let ptr = handle.surface;

        // Get the `Backend`.
        let backend = backend_from_ptr(ptr.as_ptr().cast());

        // Create the `ObjectId` from the `wl_surface` pointer.
        let id = wc::ObjectId::from_ptr(
            wayland_client::protocol::wl_surface::WlSurface::interface(),
            ptr.as_ptr().cast(),
        )
        .map_err(|_| crate::Error(crate::Repr::WaylandNotRust))?;

        /* Ensure the object is owned by Rust's wayland-backend. */
        if backend.get_data(id.clone()).is_err() {
            return Err(crate::Error(crate::Repr::WaylandNotRust));
        }

        Ok(WaylandHandle { backend, id })
    }

    /// Convert the `WaylandHandle` into a window handle.
    pub(super) fn as_ptr(
        handle: &WaylandHandle,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        match core::ptr::NonNull::new(handle.id.as_ptr()) {
            None => Err(raw_window_handle::HandleError::Unavailable),
            Some(non_null) => {
                let raw = raw_window_handle::WaylandWindowHandle::new(non_null.cast()).into();

                // SAFETY: The proxy is being kept alive, so we know it's valid.
                Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
            }
        }
    }

    /// Gets the `Backend` from a `*mut wl_proxy`.
    ///
    /// # Safety
    ///
    /// The `wl_proxy` pointer must be valid and point to a Wayland object.
    pub(super) unsafe fn backend_from_ptr(ptr: *mut wayland_sys::client::wl_proxy) -> wc::Backend {
        use wayland_sys::client::*;

        let back_ptr =
            wayland_sys::ffi_dispatch!(wayland_client_handle(), wl_proxy_get_display, ptr);

        wc::Backend::from_foreign_display(back_ptr)
    }
}
