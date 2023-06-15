use super::State;

pub fn start_observer_task(state: &State, task_handle: &mut Option<tokio::task::JoinHandle<()>>) {
    let experiment = state.experiment.clone();
    let mut observers = state.connections.observers();
    *task_handle = Some(tokio::spawn(async move {
        for observer in &mut observers {
            match observer.update(experiment.clone()).await {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Error updating observer: {}", err);
                }
            }
        }
    }));
}
