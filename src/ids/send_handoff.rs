//! A local responsiveness deadline hands off a pending job; it is not an IDS
//! rejection deadline. The worker owns its existing bounded wire retry policy.
use std::time::Duration;
use tokio::{sync::broadcast, task::JoinHandle};

pub(super) async fn handoff<T: Clone, E>(
    mut task: JoinHandle<Result<(), E>>,
    mut progress: broadcast::Receiver<T>,
) -> Result<Option<JoinHandle<Result<(), E>>>, E> {
    let mut received = false;
    let mut progress_open = true;
    loop {
        tokio::select! {
            finished = &mut task => {
                finished.expect("Send worker panicked")?;
                return Ok(None);
            },
            update = progress.recv(), if progress_open => {
                match update {
                    Ok(_) => received = true,
                    Err(broadcast::error::RecvError::Lagged(_)) => received = true,
                    Err(broadcast::error::RecvError::Closed) => progress_open = false,
                }
            },
            _ = tokio::time::sleep(Duration::from_millis(if received { 500 } else { 15000 })) => {
                return Ok(Some(task));
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn delayed_confirmation_survives_initial_deadline() {
        let (tx, rx) = broadcast::channel(1);
        // Production retains SendJob.process after the progress watcher leaves.
        let _retained_status = tx.subscribe();
        let job = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(33)).await;
            tx.send(()).unwrap();
            Ok::<_, &'static str>(())
        });
        let pending = handoff(job, rx).await.unwrap().expect("job remains pending");
        assert!(!pending.is_finished());
        assert_eq!(pending.await.unwrap(), Ok(()));
    }

    #[tokio::test(start_paused = true)]
    async fn genuine_immediate_failure_is_preserved() {
        let (_tx, rx) = broadcast::channel::<()>(1);
        let job = tokio::spawn(async { Err::<(), _>("rejected") });
        assert_eq!(handoff(job, rx).await.unwrap_err(), "rejected");
    }

    #[tokio::test(start_paused = true)]
    async fn terminal_failure_after_handoff_is_preserved() {
        let (_tx, rx) = broadcast::channel::<()>(1);
        let job = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Err::<(), _>("exhausted")
        });
        let pending = handoff(job, rx).await.unwrap().unwrap();
        assert_eq!(pending.await.unwrap(), Err("exhausted"));
    }

    #[tokio::test(start_paused = true)]
    async fn closed_progress_channel_does_not_spin() {
        let (tx, rx) = broadcast::channel::<()>(1);
        drop(tx);
        let job = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(20)).await;
            Ok::<_, &'static str>(())
        });
        let pending = handoff(job, rx).await.unwrap().unwrap();
        assert_eq!(pending.await.unwrap(), Ok(()));
    }
}
