pub mod rendering;
pub mod windowing;

pub fn init(_args: std::env::Args) -> anyhow::Result<()> {
	rendering::init()?;

	return Ok(());
}
