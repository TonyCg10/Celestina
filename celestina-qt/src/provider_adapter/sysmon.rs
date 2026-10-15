//! CPU and memory, read from `/proc`.

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use celestina_shell_core::sysmon::{self, CpuSampler, Load};
use serde_json::Value;

use super::tools::{launch, lock_runtime};

/// A panel number nobody stares at. Two seconds is often enough to read as
/// live and rare enough that the shell is not a reason the machine is busy.
const INTERVAL: Duration = Duration::from_secs(2);
/// The one entry of the author's configured monitor chain that is installed.
/// The panel opens what the session already opens; it does not become a system
/// monitor of its own.
const EXTERNAL_MONITOR: &str = "missioncenter";

pub const NAME: &str = "sysmon";

/// Registers the provider and starts reading. Registering first means a command
/// can reach it while its first sample is still being taken.
pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: sysmon: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id))?;
    Ok(())
}

pub fn action(verb: &str) -> Result<(), String> {
    match verb {
        "open-monitor" => launch(EXTERNAL_MONITOR),
        _ => Err(format!("'{NAME}' does not serve the verb '{verb}'")),
    }
}

/// A read that fails withdraws the provider instead of freezing its last
/// value: the panel would rather show nothing than a number from a minute ago.
fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let mut sampler = CpuSampler::new();

    loop {
        let reading = std::fs::read_to_string("/proc/stat")
            .map_err(|error| error.to_string())
            .and_then(|stat| sysmon::parse_cpu(&stat).map_err(|error| error.to_string()))
            .and_then(|ticks| sampler.sample(ticks).map_err(|error| error.to_string()))
            .and_then(|cpu| {
                let memory = std::fs::read_to_string("/proc/meminfo")
                    .map_err(|error| error.to_string())
                    .and_then(|meminfo| {
                        sysmon::parse_memory(&meminfo).map_err(|error| error.to_string())
                    })?;
                Ok((cpu, memory))
            });

        match reading {
            // The first sample carries no rate yet; there is nothing truthful
            // to publish until the second one.
            Ok((None, _)) => {}
            Ok((Some(cpu), memory)) => {
                let ram = memory.used_percent();
                let mut payload = Payload::new();
                payload.insert("cpu".to_owned(), Value::from(cpu));
                payload.insert("cpuLoad".to_owned(), Value::from(Load::of(cpu).as_str()));
                payload.insert("ram".to_owned(), Value::from(ram));
                payload.insert("ramLoad".to_owned(), Value::from(Load::of(ram).as_str()));
                payload.insert("ramUsedKib".to_owned(), Value::from(memory.used_kib));
                payload.insert("ramTotalKib".to_owned(), Value::from(memory.total_kib));

                if let Err(error) = lock_runtime(runtime).publish(id, payload) {
                    eprintln!("celestina-provider-adapter: sysmon: {error}");
                }
            }
            Err(reason) => {
                eprintln!("celestina-provider-adapter: sysmon: {reason}");
                // A machine whose counters went away is not a machine at 0 %.
                sampler.reset();
                lock_runtime(runtime).withdraw(id);
            }
        }

        thread::sleep(INTERVAL);
    }
}
