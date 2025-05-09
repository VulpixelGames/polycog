mod client;
mod constants;
mod error;

use log::*;

fn main() {
	info!("Initializing {}", constants::NAME);
	client::init(std::env::args());
}
