use futures::future::join_all;

use super::State;

pub fn start_observer_task(state: &State, task_handle: &mut Option<tokio::task::JoinHandle<()>>) {
    let experiment = state.experiment.clone();
    let mut observers = state.connections.observers();
    *task_handle = Some(tokio::spawn(async move {
        let start = std::time::Instant::now();
        let futures = observers
            .iter_mut()
            .map(|observer| observer.update(experiment.clone()));
        join_all(futures)
            .await
            .iter()
            .for_each(|result| match result {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Error updating observer: {}", err);
                }
            });
        metrics::histogram!("latency.observers", start.elapsed());
    }));
}
