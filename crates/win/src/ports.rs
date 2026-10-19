//! TCP and UDP port tables (IPv4 and IPv6) with the owning process id.
//!
//! Process details (name, memory, command line) are not here; callers use sysinfo.

use crate::error::WinError;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, NO_ERROR, WIN32_ERROR,
};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID,
    MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, MIB_UDP6ROW_OWNER_PID, MIB_UDP6TABLE_OWNER_PID,
    MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

/// Transport protocol of a port entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    /// TCP.
    Tcp,
    /// UDP.
    Udp,
}

/// TCP connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    /// Closed.
    Closed,
    /// Listening.
    Listen,
    /// SYN sent.
    SynSent,
    /// SYN received.
    SynReceived,
    /// Established.
    Established,
    /// FIN wait 1.
    FinWait1,
    /// FIN wait 2.
    FinWait2,
    /// Close wait.
    CloseWait,
    /// Closing.
    Closing,
    /// Last ACK.
    LastAck,
    /// Time wait.
    TimeWait,
    /// Delete TCB.
    DeleteTcb,
    /// Unknown code.
    Unknown,
}

/// One row of the port table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortOwner {
    /// Protocol.
    pub proto: Proto,
    /// Local address.
    pub local_addr: IpAddr,
    /// Local port, host byte order.
    pub local_port: u16,
    /// Remote endpoint (TCP only).
    pub remote: Option<SocketAddr>,
    /// Connection state (TCP only).
    pub state: Option<TcpState>,
    /// Owning process id.
    pub pid: u32,
}

fn state_from(code: u32) -> TcpState {
    match code {
        1 => TcpState::Closed,
        2 => TcpState::Listen,
        3 => TcpState::SynSent,
        4 => TcpState::SynReceived,
        5 => TcpState::Established,
        6 => TcpState::FinWait1,
        7 => TcpState::FinWait2,
        8 => TcpState::CloseWait,
        9 => TcpState::Closing,
        10 => TcpState::LastAck,
        11 => TcpState::TimeWait,
        12 => TcpState::DeleteTcb,
        _ => TcpState::Unknown,
    }
}

/// The port is a u16 in network byte order stored in the low 16 bits of a DWORD.
fn port(raw: u32) -> u16 {
    u16::from_be(raw as u16)
}

fn v4(raw: u32) -> Ipv4Addr {
    // The address bytes are in memory (network) order.
    Ipv4Addr::from(raw.to_ne_bytes())
}

/// Call a table API with the size-query / grow-and-retry loop. The buffer is `u64`-aligned.
fn fetch(
    context: &'static str,
    call: impl Fn(*mut core::ffi::c_void, &mut u32) -> u32,
) -> Result<Vec<u64>, WinError> {
    let mut size = 0u32;
    for _ in 0..8 {
        let mut buf = vec![0u64; (size as usize).div_ceil(8).max(1)];
        let mut sz = size;
        let ptr: *mut core::ffi::c_void = if size == 0 {
            core::ptr::null_mut()
        } else {
            buf.as_mut_ptr().cast()
        };
        match WIN32_ERROR(call(ptr, &mut sz)) {
            NO_ERROR => return Ok(buf),
            ERROR_INSUFFICIENT_BUFFER => size = sz + 4096, // table may grow between calls
            e => {
                return Err(WinError::Os {
                    code: e.0 as i32,
                    context,
                });
            }
        }
    }
    Err(WinError::Os {
        code: ERROR_INSUFFICIENT_BUFFER.0 as i32,
        context,
    })
}

fn tcp4(out: &mut Vec<PortOwner>) -> Result<(), WinError> {
    let buf = fetch("GetExtendedTcpTable", |p, sz| {
        // SAFETY: `p` is null with size 0, or points to a buffer of `*sz` bytes; `sz` is valid.
        unsafe {
            GetExtendedTcpTable(
                (!p.is_null()).then_some(p),
                sz,
                false,
                AF_INET.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        }
    })?;
    // SAFETY: on success the buffer holds a table header then `dwNumEntries` rows; u64 alignment suffices.
    let rows: &[MIB_TCPROW_OWNER_PID] = unsafe {
        let t = &*(buf.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>());
        core::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize)
    };
    out.extend(rows.iter().map(|r| PortOwner {
        proto: Proto::Tcp,
        local_addr: v4(r.dwLocalAddr).into(),
        local_port: port(r.dwLocalPort),
        remote: Some(SocketAddr::new(
            v4(r.dwRemoteAddr).into(),
            port(r.dwRemotePort),
        )),
        state: Some(state_from(r.dwState)),
        pid: r.dwOwningPid,
    }));
    Ok(())
}

fn tcp6(out: &mut Vec<PortOwner>) -> Result<(), WinError> {
    let buf = fetch("GetExtendedTcpTable", |p, sz| {
        // SAFETY: as in `tcp4`.
        unsafe {
            GetExtendedTcpTable(
                (!p.is_null()).then_some(p),
                sz,
                false,
                AF_INET6.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        }
    })?;
    // SAFETY: as in `tcp4`, for the IPv6 layout.
    let rows: &[MIB_TCP6ROW_OWNER_PID] = unsafe {
        let t = &*(buf.as_ptr().cast::<MIB_TCP6TABLE_OWNER_PID>());
        core::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize)
    };
    out.extend(rows.iter().map(|r| PortOwner {
        proto: Proto::Tcp,
        local_addr: Ipv6Addr::from(r.ucLocalAddr).into(),
        local_port: port(r.dwLocalPort),
        remote: Some(SocketAddr::new(
            Ipv6Addr::from(r.ucRemoteAddr).into(),
            port(r.dwRemotePort),
        )),
        state: Some(state_from(r.dwState)),
        pid: r.dwOwningPid,
    }));
    Ok(())
}

fn udp4(out: &mut Vec<PortOwner>) -> Result<(), WinError> {
    let buf = fetch("GetExtendedUdpTable", |p, sz| {
        // SAFETY: as in `tcp4`.
        unsafe {
            GetExtendedUdpTable(
                (!p.is_null()).then_some(p),
                sz,
                false,
                AF_INET.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            )
        }
    })?;
    // SAFETY: as in `tcp4`, for the UDP layout.
    let rows: &[MIB_UDPROW_OWNER_PID] = unsafe {
        let t = &*(buf.as_ptr().cast::<MIB_UDPTABLE_OWNER_PID>());
        core::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize)
    };
    out.extend(rows.iter().map(|r| PortOwner {
        proto: Proto::Udp,
        local_addr: v4(r.dwLocalAddr).into(),
        local_port: port(r.dwLocalPort),
        remote: None,
        state: None,
        pid: r.dwOwningPid,
    }));
    Ok(())
}

fn udp6(out: &mut Vec<PortOwner>) -> Result<(), WinError> {
    let buf = fetch("GetExtendedUdpTable", |p, sz| {
        // SAFETY: as in `tcp4`.
        unsafe {
            GetExtendedUdpTable(
                (!p.is_null()).then_some(p),
                sz,
                false,
                AF_INET6.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            )
        }
    })?;
    // SAFETY: as in `tcp4`, for the UDP IPv6 layout.
    let rows: &[MIB_UDP6ROW_OWNER_PID] = unsafe {
        let t = &*(buf.as_ptr().cast::<MIB_UDP6TABLE_OWNER_PID>());
        core::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize)
    };
    out.extend(rows.iter().map(|r| PortOwner {
        proto: Proto::Udp,
        local_addr: Ipv6Addr::from(r.ucLocalAddr).into(),
        local_port: port(r.dwLocalPort),
        remote: None,
        state: None,
        pid: r.dwOwningPid,
    }));
    Ok(())
}

/// Every TCP and UDP endpoint on the machine, IPv4 and IPv6. Blocking; run in `spawn_blocking`.
pub fn all_ports() -> Result<Vec<PortOwner>, WinError> {
    let mut out = Vec::new();
    tcp4(&mut out)?;
    tcp6(&mut out)?;
    udp4(&mut out)?;
    udp6(&mut out)?;
    Ok(out)
}

/// Entries whose local port is `port`.
pub fn port_owners(port: u16) -> Result<Vec<PortOwner>, WinError> {
    let mut v = all_ports()?;
    v.retain(|p| p.local_port == port);
    Ok(v)
}

/// Closes the process handle on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful OpenProcess and is closed exactly once here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Terminate a process. Refuses pid 0, 4 (System) and the current process.
pub fn kill_process(pid: u32) -> Result<(), WinError> {
    if pid == 0 || pid == 4 {
        return Err(WinError::Unavailable(format!(
            "refusing to terminate system process {pid}"
        )));
    }
    if pid == std::process::id() {
        return Err(WinError::Unavailable(
            "refusing to terminate the current process".into(),
        ));
    }
    // SAFETY: plain FFI call; the returned handle is owned by the guard below.
    let h = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }
        .map_err(|e| WinError::os(e, "OpenProcess"))?;
    let h = Handle(h);
    // SAFETY: `h.0` is an open handle with PROCESS_TERMINATE.
    unsafe { TerminateProcess(h.0, 1) }.map_err(|e| WinError::os(e, "TerminateProcess"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_byte_order() {
        assert_eq!(port(0x5000), 80);
    }

    #[test]
    fn finds_own_sockets() {
        let t = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let u = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let me = std::process::id();
        for (proto, p) in [
            (Proto::Tcp, t.local_addr().unwrap().port()),
            (Proto::Udp, u.local_addr().unwrap().port()),
        ] {
            let owners = port_owners(p).unwrap();
            assert!(
                owners.iter().any(|o| o.proto == proto && o.pid == me),
                "{proto:?} {p}: {owners:?}"
            );
        }
    }

    #[test]
    fn kill_refusals() {
        for pid in [0, 4, std::process::id()] {
            assert!(matches!(kill_process(pid), Err(WinError::Unavailable(_))));
        }
    }
}
