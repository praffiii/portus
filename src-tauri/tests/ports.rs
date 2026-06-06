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
