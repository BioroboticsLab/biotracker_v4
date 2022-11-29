use super::message_bus::{Client, Server};
use crate::components::{Matcher, Sampler, Tracker};
use anyhow::Result;
use std::marker::PhantomData;
use std::thread::JoinHandle;

pub trait Component {
    fn new(msg_bus: Client) -> Self;
    fn run(&mut self) -> Result<()>;
}

struct ComponentThread<T> {
    join_handle: JoinHandle<Result<()>>,
    phantom: PhantomData<T>,
}

impl<T: Component> ComponentThread<T> {
    pub fn new() -> Result<Self> {
        let phantom = PhantomData::<T>;
        Ok(Self {
            join_handle: ComponentThread::<T>::spawn()?,
            phantom,
        })
    }

    pub fn check(mut self) -> Result<Self> {
        if self.join_handle.is_finished() {
            let _ = self.join_handle.join();
            self.join_handle = ComponentThread::<T>::spawn()?;
        }
        Ok(self)
    }

    fn spawn() -> Result<JoinHandle<Result<()>>> {
        let msg_bus = Client::new()?;
        let thread_name = std::any::type_name::<T>();
        Ok(std::thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || -> Result<()> {
                let mut component: T = Component::new(msg_bus);
                component.run()
            })?)
    }
}

pub fn run_components() -> Result<()> {
    let server = Server::new()?;
    std::thread::spawn(move || {
        server.run().unwrap();
    });

    std::thread::spawn(move || {
        let mut matcher = ComponentThread::<Matcher>::new().unwrap();
        let mut sampler = ComponentThread::<Sampler>::new().unwrap();
        let mut tracker = ComponentThread::<Tracker>::new().unwrap();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            matcher = matcher.check().unwrap();
            sampler = sampler.check().unwrap();
            tracker = tracker.check().unwrap();
        }
    });
    Ok(())
}
