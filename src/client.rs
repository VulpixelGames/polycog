use crate::error;

pub mod rendering;
pub mod windowing;

pub fn init(_args: std::env::Args) -> Result<(), error::GameError> {
	rendering::init()?;

	Ok(())
}
