use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use portus_lib::ports::{FakePortProbe, ListenerProcess, PortListener, PortProbe, Protocol};

#[test]
fn fake_probe_returns_configured_listeners() {
    let expected = vec![PortListener {
        protocol: Protocol::Tcp,
        socket: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000),
        process: ListenerProcess {
            pid: 42,
            name: "node".to_string(),
            path: "/usr/local/bin/node".to_string(),
        },
    }];
    let probe = FakePortProbe::new(expected.clone());

    assert_eq!(probe.scan().unwrap(), expected);
}

#[cfg(target_os = "macos")]
#[test]
fn system_probe_detects_dummy_tcp_listener() {
    use std::net::TcpListener;

    use portus_lib::ports::SystemPortProbe;

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();

    let detected = SystemPortProbe.scan().unwrap();

    assert!(
        detected
            .iter()
            .any(|entry| entry.protocol == Protocol::Tcp && entry.socket.port() == port),
        "expected TCP listener on port {port}, got {detected:#?}"
    );
}

#[test]
fn normalize_collapses_dual_stack_into_one_row() {
    use portus_lib::ports::{normalize, AddressFamily, BindScope};

    let rows = normalize(vec![
        PortListener {
            protocol: Protocol::Tcp,
            socket: "0.0.0.0:3000".parse().unwrap(),
            process: ListenerProcess {
                pid: 1,
                name: "web".to_string(),
                path: "/web".to_string(),
            },
        },
        PortListener {
            protocol: Protocol::Tcp,
            socket: "[::]:3000".parse().unwrap(),
            process: ListenerProcess {
                pid: 1,
                name: "web".to_string(),
                path: "/web".to_string(),
            },
        },
    ]);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].scope, BindScope::AllInterfaces);
    assert_eq!(rows[0].families, vec![AddressFamily::V4, AddressFamily::V6]);
}
