//! Bounded blocking slide work. The caller runs outside Tokio's async workers.
use super::{SlideResult, SlideTemplateError};

pub(super) const SLIDE_CONCURRENCY: usize = 4;

/// Join every worker before returning, preserving input order and propagating failures.
pub(super) fn map<T: Send, U: Send>(
    inputs: Vec<T>,
    process: impl Fn(T) -> SlideResult<U> + Sync,
) -> SlideResult<Vec<U>> {
    let mut batches: Vec<Vec<_>> = (0..SLIDE_CONCURRENCY).map(|_| Vec::new()).collect();
    for (index, input) in inputs.into_iter().enumerate() {
        batches[index % SLIDE_CONCURRENCY].push((index, input));
    }
    let results = std::thread::scope(|scope| {
        let mut workers = Vec::new();
        let mut spawn_error = None;
        for batch in batches.into_iter().filter(|batch| !batch.is_empty()) {
            let process = &process;
            match std::thread::Builder::new().spawn_scoped(scope, move || {
                batch
                    .into_iter()
                    .map(|(index, input)| process(input).map(|output| (index, output)))
                    .collect::<SlideResult<Vec<_>>>()
            }) {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    spawn_error = Some(SlideTemplateError::internal(error));
                    break;
                }
            }
        }
        // Collect joins before propagating an error so no child panic escapes the scope.
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| {
                worker.join().unwrap_or_else(|_| {
                    Err(SlideTemplateError::internal(anyhow::anyhow!(
                        "slide worker panicked"
                    )))
                })
            })
            .collect();
        if let Some(error) = spawn_error {
            return Err(error);
        }
        results.into_iter().collect::<SlideResult<Vec<_>>>()
    })?;
    let mut outputs: Vec<_> = results.into_iter().flatten().collect();
    outputs.sort_by_key(|(index, _)| *index);
    Ok(outputs.into_iter().map(|(_, output)| output).collect())
}
