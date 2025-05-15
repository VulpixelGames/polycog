mod client;
mod constants;
mod error;
mod event;

use std::panic::panic_any;

pub use ::log::{debug, error, info, trace, warn}; // easy logging anywhere
use winit::{
	event_loop::{ControlFlow, EventLoop},
	window::WindowAttributes,
};

fn handled_main() -> Result<(), error::GameError> {
	// Base initialization
	error::set_panic_hook();
	simple_logger::init_with_level(log::Level::Info).unwrap();

	// Event loop initialization
	let result: Result<_, error::InitError> = (|| {
		// Initialize event loop
		let event_loop = EventLoop::new()?;
		event_loop.set_control_flow(ControlFlow::Poll);

		// Initialize app
		let window_attributes = WindowAttributes::default().with_title(constants::NAME);

		Ok((
			event::App::new_windowed(window_attributes.clone()),
			event_loop,
		))
	})();
	let (mut app, event_loop) = result.map_err(Into::<error::GameError>::into)?;

	event_loop.run_app(&mut app)?;

	Ok(())
}

fn main() {
	let result = handled_main();

	if let Err(error) = result {
		panic_any(error); // Should we really be using this instead of panic!?
	}
}
