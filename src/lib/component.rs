use anyhow::Result;

use crate::{message_bus::Client, CommandLineArguments};
use std::sync::Arc;

pub trait Component {
    fn new(msg_bus: Client, args: Arc<CommandLineArguments>) -> Self
    where
        Self: Sized;
    fn run(&mut self) -> Result<()>;
}
