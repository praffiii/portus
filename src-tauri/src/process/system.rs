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
        let mut system = self.lock_system()?;
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
        let mut system = self.lock_system()?;
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
        let mut system = self.lock_system()?;
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
    fn lock_system(&self) -> Result<std::sync::MutexGuard<'_, System>, ProcessError> {
        self.system
            .lock()
            .map_err(|error| ProcessError::Inspect(error.to_string()))
    }
}

fn process_info(process: &sysinfo::Process) -> ProcessInfo {
    ProcessInfo {
        pid: process.pid().as_u32(),
        parent_pid: process.parent().map(Pid::as_u32),
        name: process.name().to_os_string(),
        command: process.cmd().to_vec(),
        executable: process.exe().map(ToOwned::to_owned),
        cwd: process.cwd().map(ToOwned::to_owned),
        start_time: process.start_time(),
        cpu_usage: process.cpu_usage(),
        memory_bytes: process.memory(),
    }
}
