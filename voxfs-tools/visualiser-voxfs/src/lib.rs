#[macro_use]
mod macros;
mod application;
mod error;
mod user_interface;

pub use application::Application;
use error::VisualiserError;
use user_interface::UI;
