// MIT/Apache2/ZLib License

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop};

use owned_window_handle::OwnedWindowHandle;

/// Run the application event loop.
pub(crate) fn run(event_loop: EventLoop<()>) {
    #[cfg(not(target_family = "wasm"))]
    event_loop.run_app(&mut Application).unwrap();

    #[cfg(target_family = "wasm")]
    winit::platform::web::EventLoopExtSys::spawn_app(event_loop, Application)
}

/// Application to run.
struct Application;

impl ApplicationHandler for Application {
    #[inline]
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Actual test: create a window.
        let window = event_loop.create_window(Default::default()).unwrap();

        // Create the window handle, which clones it.
        let handle = OwnedWindowHandle::new(&window).unwrap();

        // Create another.
        let handle2 = OwnedWindowHandle::new(&window).unwrap();

        // Drop the current window.
        drop(window);

        // Drop both handles.
        drop((handle, handle2));

        // Stop the loop now.
        event_loop.exit();
    }

    #[inline]
    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
        // Intentionally left blank.
    }
}
