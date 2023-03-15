use anyhow::Result;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6, TcpListener, ToSocketAddrs};

/// Try to bind to a socket using TCP
fn test_bind_tcp<A: ToSocketAddrs>(addr: A) -> Option<u16> {
    Some(TcpListener::bind(addr).ok()?.local_addr().ok()?.port())
}

/// Check if a port is free on TCP
pub fn is_free_tcp(port: u16) -> bool {
    let ipv4 = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    let ipv6 = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, port, 0, 0);
    test_bind_tcp(ipv6).is_some() && test_bind_tcp(ipv4).is_some()
}

pub struct PortFinder {
    range_start: u16,
    counter: u16,
}

impl PortFinder {
    pub fn new(range_start: u16) -> Self {
        PortFinder {
            range_start,
            counter: 0,
        }
    }

    pub fn next(&mut self) -> Result<u16> {
        let retries = 100;
        for _ in 0..retries {
            let port = self.range_start + self.counter;
            self.counter += 1;
            if is_free_tcp(port) {
                return Ok(port);
            }
        }
        Err(anyhow::anyhow!(
            "Could not find any free port in range {}-{}",
            self.range_start + self.counter - retries,
            self.range_start + self.counter
        ))
    }
}
