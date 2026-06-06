use std::sync::Mutex;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind};

use super::{ProcessError, ProcessInfo, ProcessProbe, ProcessSignal, ProcessSnapshot};

pub struct SystemProcessProbe {
    system: Mutex<System>,
}

impl Default for SystemProcessProbe {
    fn default() -> Self {
        Self {
            system: Mutex::new(System::new()),
        }
    }
}

impl ProcessProbe for SystemProcessProbe {
    fn info_for_pids(&self, pids: &[u32]) -> Result<Vec<ProcessInfo>, ProcessError> {
        let sysinfo_pids: Vec<Pid> = pids.iter().copied().map(Pid::from_u32).collect();
        let mut system = self.lock_system();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&sysinfo_pids),
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_cmd(UpdateKind::Always)
                .with_exe(UpdateKind::Always)
                .with_cwd(UpdateKind::Always)
                .without_tasks(),
        );

        Ok(sysinfo_pids
            .into_iter()
            .filter_map(|pid| system.process(pid))
            .map(process_info)
            .collect())
    }

    fn all_snapshots(&self) -> Result<Vec<ProcessSnapshot>, ProcessError> {
        let mut system = self.lock_system();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().without_tasks(),
        );

        Ok(system
            .processes()
            .values()
            .map(|process| ProcessSnapshot {
                pid: process.pid().as_u32(),
                parent_pid: process.parent().map(Pid::as_u32),
            })
            .collect())
    }

    fn signal(&self, pid: u32, signal: ProcessSignal) -> Result<bool, ProcessError> {
        let sysinfo_pid = Pid::from_u32(pid);
        let mut system = self.lock_system();
        system.refresh_processes(ProcessesToUpdate::Some(&[sysinfo_pid]), true);
        let Some(process) = system.process(sysinfo_pid) else {
            return Ok(true);
        };
        let signal = match signal {
            ProcessSignal::Terminate => Signal::Term,
            ProcessSignal::Kill => Signal::Kill,
        };
        Ok(process.kill_with(signal).unwrap_or(false))
    }
}

impl SystemProcessProbe {
    fn lock_system(&self) -> std::sync::MutexGuard<'_, System> {
        self.system
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

fn process_info(process: &sysinfo::Process) -> ProcessInfo {
    ProcessInfo {
        pid: process.pid().as_u32(),
        parent_pid: process.parent().map(Pid::as_u32),
        name: lossy_os_string(process.name()),
        command: process
            .cmd()
            .iter()
            .map(|value| lossy_os_string(value))
            .collect(),
        executable: process.exe().map(|path| lossy_os_string(path.as_os_str())),
        cwd: process.cwd().map(|path| lossy_os_string(path.as_os_str())),
        start_time: process.start_time(),
        cpu_usage: process.cpu_usage(),
        memory_bytes: process.memory(),
    }
}

fn lossy_os_string(value: &std::ffi::OsStr) -> String {
    value.to_string_lossy().into_owned()
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    use serde_json::json;

    use super::{lossy_os_string, SystemProcessProbe};
    use crate::process::{ProcessInfo, ProcessProbe};
    use crate::state::{Snapshot, SnapshotSection};

    #[test]
    fn non_utf8_process_fields_serialize_lossily_as_strings() {
        let invalid = OsString::from_vec(vec![b'n', b'o', 0x80, b'e']);
        let process = ProcessInfo {
            pid: 42,
            parent_pid: None,
            name: lossy_os_string(&invalid),
            command: vec![lossy_os_string(&invalid)],
            executable: Some(lossy_os_string(PathBuf::from(&invalid).as_os_str())),
            cwd: Some(lossy_os_string(PathBuf::from(&invalid).as_os_str())),
            start_time: 1,
            cpu_usage: 0.0,
            memory_bytes: 0,
        };
        let snapshot = Snapshot {
            ports: SnapshotSection {
                data: vec![],
                error: None,
            },
            processes: SnapshotSection {
                data: vec![process],
                error: None,
            },
        };

        let value = serde_json::to_value(snapshot).expect("lossy strings must serialize");

        assert_eq!(value["processes"]["data"][0]["name"], json!("no\u{fffd}e"));
        assert!(value.to_string().find("\"Unix\"").is_none());
    }

    #[test]
    fn poisoned_system_mutex_recovers_on_the_next_probe() {
        let probe = SystemProcessProbe::default();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = probe.system.lock().unwrap();
            panic!("poison system mutex");
        }));

        let processes = probe.info_for_pids(&[std::process::id()]).unwrap();

        assert_eq!(processes.len(), 1);
    }
}
