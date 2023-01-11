use super::{Matcher, PythonRunner, VideoDecoder, VideoEncoder};
use anyhow::Result;
use libtracker::{
    message_bus::{Client, Server},
    CommandLineArguments, Component,
};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct Core {
    _server_handle: JoinHandle<()>,
    args: Arc<CommandLineArguments>,
    threads: Vec<JoinHandle<Result<()>>>,
}

impl Core {
    pub fn new(args: &CommandLineArguments) -> Result<Self> {
        let server = Server::new()?;
        let server_handle = std::thread::spawn(move || {
            server.run().unwrap();
        });
        Ok(Self {
            _server_handle: server_handle,
            args: Arc::new(args.clone()),
            threads: vec![],
        })
    }

    pub fn start(&mut self) -> Result<()> {
        if let (Some(venv), Some(cmd)) = (&self.args.tracker_venv, &self.args.tracker_cmd) {
            let (venv, cmd) = (venv.to_owned(), cmd.to_owned());
            self.add_component(|_msg_bus, _args| PythonRunner::new(venv, cmd))?;
        }

        if let (Some(venv), Some(cmd)) = (&self.args.robofish_venv, &self.args.robofish_cmd) {
            let (venv, cmd) = (venv.to_owned(), cmd.to_owned());
            self.add_component(|_msg_bus, _args| PythonRunner::new(venv, cmd))?;
        }

        self.add_component(VideoEncoder::new)?;
        self.add_component(Matcher::new)?;
        self.add_component(VideoDecoder::new)?;
        Ok(())
    }

    fn add_component<F, T: Component + 'static>(&mut self, component_builder: F) -> Result<()>
    where
        F: FnOnce(Client, Arc<CommandLineArguments>) -> T + Send + 'static,
    {
        let thread_name = std::any::type_name::<T>();
        let msg_bus = Client::new()?;
        let args = self.args.clone();
        let thread_handle = std::thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || -> Result<()> {
                let mut component = component_builder(msg_bus, args);
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
