use anyhow::Result;

pub trait Component {
    fn run(&mut self) -> Result<()>;
}
