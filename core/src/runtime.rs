use std::num::NonZeroUsize;
use std::sync::Arc;

use nmp::{Engine, EngineConfig, LiveQuery, RelayUrl, Window};
use nmp_nip29::group_content_demand;

use crate::model::{KernelConfiguration, KernelPhase, QueueSnapshot};
use crate::projection::project;
use crate::{Control, Emitter};

pub fn run(configuration: KernelConfiguration, emitter: Emitter, control: Arc<Control>) {
    emitter.emit(&QueueSnapshot::lifecycle(
        &configuration,
        KernelPhase::Starting,
    ));
    if let Err(error) = run_inner(&configuration, &emitter, &control) {
        emitter.emit(&QueueSnapshot::failed(&configuration, error));
    }
    emitter.emit(&QueueSnapshot::lifecycle(
        &configuration,
        KernelPhase::Stopped,
    ));
}

fn run_inner(
    configuration: &KernelConfiguration,
    emitter: &Emitter,
    control: &Arc<Control>,
) -> Result<(), String> {
    let engine = Engine::new(EngineConfig {
        store_path: configuration.store_path.clone(),
        ..EngineConfig::default()
    })
    .map_err(|error| format!("NMP engine refused startup: {error}"))?;

    let host = RelayUrl::parse(&configuration.relay)
        .map_err(|_| "The configured NIP-29 host is invalid.".to_string())?;
    let demand = group_content_demand(host, &configuration.group_id);
    let window = Window::Expandable {
        initial: NonZeroUsize::new(40).expect("nonzero initial window"),
        max: NonZeroUsize::new(100).expect("nonzero maximum window"),
    };
    let subscription = engine
        .observe(LiveQuery(demand), Some(window))
        .map_err(|error| format!("NMP refused the group observation: {error}"))?;
    if !control.install(subscription.cancel_handle()) {
        subscription.cancel();
        engine.shutdown();
        return Ok(());
    }

    while let Ok(frame) = subscription.recv() {
        if let Some(contents) = frame.window {
            emitter.emit(&project(configuration, &contents.rows, &frame.evidence));
        }
    }

    engine.shutdown();
    Ok(())
}
