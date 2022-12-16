use super::message_bus::{Client, Server};
use anyhow::Result;
use std::thread::JoinHandle;

pub trait Component {
    fn run(&mut self) -> Result<()>;
}

pub struct ComponentRunner {
    _server_handle: JoinHandle<()>,
    threads: Vec<JoinHandle<Result<()>>>,
}

impl ComponentRunner {
    pub fn new() -> Result<Self> {
        let server = Server::new()?;
        let server_handle = std::thread::spawn(move || {
            server.run().unwrap();
        });
        Ok(Self {
            _server_handle: server_handle,
            threads: vec![],
        })
    }

    pub fn add_component<F, T: Component>(&mut self, component_builder: F) -> Result<()>
    where
        F: FnOnce(Client) -> T + Send + 'static,
    {
        let thread_name = std::any::type_name::<T>();
        let msg_bus = Client::new()?;
        let thread_handle = std::thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || -> Result<()> {
                let mut component = component_builder(msg_bus);
                match component.run() {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        eprintln!("Error in {}: {}", thread_name, e);
                        Err(e)
                    }
                }
            })?;
        self.threads.push(thread_handle);
        Ok(())
    }
}
