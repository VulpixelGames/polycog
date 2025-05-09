mod client;
mod constants;
mod error;

use std::panic::panic_any;

use log::*;

fn handled_main() -> Result<(), error::GameError> {
	// Base initialization
	error::set_panic_hook();

	// Client initialization
	client::init(std::env::args())?;

	Ok(())
}

fn main() {
	let result = handled_main();

	if let Err(error) = result {
		panic_any(error);
	}

	panic!("Yeet!");
}
