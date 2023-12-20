pub mod cam_component;
pub mod input;
pub mod plugin;

pub mod prelude {
    pub use crate::{cam_component::*, plugin::*};
}
