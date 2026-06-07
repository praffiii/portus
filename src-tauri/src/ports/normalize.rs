use std::collections::BTreeMap;
use std::net::SocketAddr;

use super::{AddressFamily, BindScope, PortListener, PortOwner, PortRow, Protocol};

/// Normalize raw TCP listeners into stable, deduped port rows (ADR 0011).
pub fn normalize(listeners: Vec<PortListener>) -> Vec<PortRow> {
    struct Group {
        port: u16,
        scope: BindScope,
        specific_addr: Option<String>,
        families: Vec<AddressFamily>,
        owners: Vec<PortOwner>,
    }

    let mut groups: BTreeMap<(u16, String), Group> = BTreeMap::new();

    for listener in listeners {
        let PortListener {
            protocol: _,
            socket,
            process,
        } = listener;
        let (scope, token, specific_addr) = classify(&socket);
        let family = if socket.is_ipv4() {
            AddressFamily::V4
        } else {
            AddressFamily::V6
        };

        let group = groups
            .entry((socket.port(), token))
            .or_insert_with(|| Group {
                port: socket.port(),
                scope,
                specific_addr,
                families: Vec::new(),
                owners: Vec::new(),
            });

        if !group.families.contains(&family) {
            group.families.push(family);
        }
        if !group.owners.iter().any(|owner| owner.pid == process.pid) {
            group.owners.push(PortOwner {
                pid: process.pid,
                name: process.name,
                path: process.path,
            });
        }
    }

    let mut rows: Vec<PortRow> = groups
        .into_values()
        .map(|mut group| {
            group.families.sort();
            group.owners.sort_by_key(|owner| owner.pid);
            let primary = group
                .owners
                .first()
                .map(|owner| owner.pid.to_string())
                .unwrap_or_else(|| "none".to_string());
            let token = scope_token(group.scope, group.specific_addr.as_deref());
            let key = format!("tcp|{token}|{}|{primary}", group.port);
            PortRow {
                protocol: Protocol::Tcp,
                port: group.port,
                scope: group.scope,
                specific_addr: group.specific_addr,
                families: group.families,
                owners: group.owners,
                key,
            }
        })
        .collect();

    rows.sort_by(|a, b| a.key.cmp(&b.key));
    rows
}

fn classify(socket: &SocketAddr) -> (BindScope, String, Option<String>) {
    let ip = socket.ip();
    if ip.is_unspecified() {
        (BindScope::AllInterfaces, "*".to_string(), None)
    } else if ip.is_loopback() {
        (BindScope::Loopback, "loopback".to_string(), None)
    } else {
        let addr = ip.to_string();
        (BindScope::Specific, addr.clone(), Some(addr))
    }
}

fn scope_token(scope: BindScope, specific: Option<&str>) -> String {
    match scope {
        BindScope::AllInterfaces => "*".to_string(),
        BindScope::Loopback => "loopback".to_string(),
        BindScope::Specific => specific.unwrap_or_default().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::normalize;
    use crate::ports::{AddressFamily, BindScope, ListenerProcess, PortListener, Protocol};

    fn listener(addr: &str, pid: u32) -> PortListener {
        PortListener {
            protocol: Protocol::Tcp,
            socket: addr.parse::<SocketAddr>().unwrap(),
            process: ListenerProcess {
                pid,
                name: format!("proc-{pid}"),
                path: format!("/bin/proc-{pid}"),
            },
        }
    }

    #[test]
    fn dual_stack_same_pid_collapses_to_one_row() {
        let rows = normalize(vec![
            listener("0.0.0.0:3000", 42),
            listener("[::]:3000", 42),
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].port, 3000);
        assert_eq!(rows[0].scope, BindScope::AllInterfaces);
        assert_eq!(rows[0].families, vec![AddressFamily::V4, AddressFamily::V6]);
        assert_eq!(rows[0].owners.len(), 1);
        assert_eq!(rows[0].key, "tcp|*|3000|42");
    }

    #[test]
    fn loopback_v4_and_v6_collapse_to_one_row() {
        let rows = normalize(vec![
            listener("127.0.0.1:5432", 7),
            listener("[::1]:5432", 7),
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].scope, BindScope::Loopback);
        assert_eq!(rows[0].families, vec![AddressFamily::V4, AddressFamily::V6]);
        assert_eq!(rows[0].key, "tcp|loopback|5432|7");
    }

    #[test]
    fn reuseport_multiple_pids_share_one_row() {
        let rows = normalize(vec![
            listener("0.0.0.0:8080", 20),
            listener("0.0.0.0:8080", 10),
        ]);
        assert_eq!(rows.len(), 1);
        let pids: Vec<u32> = rows[0].owners.iter().map(|owner| owner.pid).collect();
        assert_eq!(pids, vec![10, 20]);
        assert_eq!(rows[0].key, "tcp|*|8080|10");
    }

    #[test]
    fn distinct_specific_addresses_stay_separate() {
        let rows = normalize(vec![
            listener("192.168.1.5:80", 1),
            listener("10.0.0.1:80", 2),
        ]);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.scope == BindScope::Specific));
    }

    #[test]
    fn ordering_is_deterministic_regardless_of_input_order() {
        let forward = normalize(vec![
            listener("127.0.0.1:3000", 1),
            listener("127.0.0.1:8080", 2),
            listener("127.0.0.1:5432", 3),
        ]);
        let shuffled = normalize(vec![
            listener("127.0.0.1:5432", 3),
            listener("127.0.0.1:3000", 1),
            listener("127.0.0.1:8080", 2),
        ]);
        let keys: Vec<&str> = forward.iter().map(|row| row.key.as_str()).collect();
        let shuffled_keys: Vec<&str> = shuffled.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(keys, shuffled_keys);
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn empty_input_yields_no_rows() {
        assert!(normalize(Vec::new()).is_empty());
    }
}
