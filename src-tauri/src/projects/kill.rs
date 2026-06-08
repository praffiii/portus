use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::registry::ProjectRegistry;

pub const QUIT_GRACE: Duration = Duration::from_millis(500);

#[cfg(unix)]
pub fn kill_group(pgid: u32, signal: libc::c_int) {
    unsafe {
        libc::killpg(pgid as libc::pid_t, signal);
    }
}

#[cfg(not(unix))]
pub fn kill_group(_pgid: u32, _signal: i32) {}

pub fn kill_all_managed(registry: &Arc<Mutex<ProjectRegistry>>) {
    let pgids: Vec<u32> = {
        let registry = registry.lock().unwrap_or_else(|e| e.into_inner());
        registry.pgids()
    };
    if pgids.is_empty() {
        return;
    }

    #[cfg(unix)]
    {
        for &pgid in &pgids {
            kill_group(pgid, libc::SIGTERM);
        }
        std::thread::sleep(QUIT_GRACE);
        for &pgid in &pgids {
            kill_group(pgid, libc::SIGKILL);
        }
    }

    registry.lock().unwrap_or_else(|e| e.into_inner()).drain();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use crate::projects::spawn_task;

    #[test]
    fn kill_group_terminates_a_running_process() {
        let mut p = spawn_task("/bin/sh", "sleep 30", Path::new("/")).unwrap();
        let pgid = p.pgid();

        kill_group(pgid, libc::SIGTERM);

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if p.try_status().unwrap().is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "group should have been killed");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
