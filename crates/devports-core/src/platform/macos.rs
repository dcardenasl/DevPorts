//! macOS [`ProcessSource`] on top of `libproc` and `sysinfo`.
//!
//! `libproc` lists the user's processes and, per process, its sockets, parent,
//! group and start time. The command line (`argv`) and the working directory
//! come from `sysinfo`, which reads them with `KERN_PROCARGS2` and
//! `PROC_PIDVNODEPATHINFO`: `libproc::proc_pid::pidcwd` is not implemented on
//! macOS, and `libproc` has no `argv` reader.
//!
//! This is the only module of the workspace allowed to use `unsafe`
//! (ADR-025): `libproc` hands back the socket protocol information as a C
//! union, and reading a union field is `unsafe` in Rust. The single read is
//! guarded by the `soi_kind` discriminant the kernel itself reports. The
//! architecture test fails if `unsafe` shows up anywhere else.

#![allow(unsafe_code)]

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use libproc::libproc::bsd_info::BSDInfo;
use libproc::libproc::file_info::{ListFDs, ProcFDType, pidfdinfo};
use libproc::libproc::net_info::{SocketFDInfo, SocketInfoKind, TcpSIState};
use libproc::libproc::proc_pid::{listpidinfo, pidinfo};
use libproc::processes::{ProcFilter, pids_by_type};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::process::{
    ListeningProcess, ProcessBrief, ProcessId, ProcessInfo, ProcessSource, ProcessSourceError,
    ProcessStart,
};

/// Observes the current user's processes through `libproc`.
#[derive(Debug, Clone, Copy)]
pub struct MacProcessSource {
    uid: u32,
}

impl MacProcessSource {
    /// Creates a source for the user that runs this process.
    ///
    /// # Errors
    ///
    /// Fails if the kernel will not describe the current process.
    pub fn new() -> Result<Self, ProcessSourceError> {
        let own_pid = i32::try_from(std::process::id())
            .map_err(|error| ProcessSourceError::CurrentUser(error.to_string()))?;
        let own = pidinfo::<BSDInfo>(own_pid, 0).map_err(ProcessSourceError::CurrentUser)?;
        Ok(Self { uid: own.pbi_uid })
    }
}

impl ProcessSource for MacProcessSource {
    fn snapshot(&self) -> Result<Vec<ListeningProcess>, ProcessSourceError> {
        let pids = pids_by_type(ProcFilter::ByUID { uid: self.uid })
            .map_err(|error| ProcessSourceError::Enumerate(error.to_string()))?;

        // A process may exit between the listing and the per-process reads;
        // those reads simply fail and the process is skipped.
        let listening: Vec<(u32, BSDInfo, Vec<u16>)> = pids
            .into_iter()
            .filter_map(|pid| {
                let bsd = bsd_info(pid)?;
                let ports = listening_ports(pid, &bsd);
                (!ports.is_empty()).then_some((pid, bsd, ports))
            })
            .collect();

        let mut details = read_details(listening.iter().map(|(pid, _, _)| *pid));
        Ok(listening
            .into_iter()
            .map(|(pid, bsd, ports)| ListeningProcess {
                info: build_info(pid, &bsd, details.remove(&pid).unwrap_or_default()),
                ports,
            })
            .collect())
    }

    fn all_processes(&self) -> Result<Vec<ProcessBrief>, ProcessSourceError> {
        let pids = pids_by_type(ProcFilter::ByUID { uid: self.uid })
            .map_err(|error| ProcessSourceError::Enumerate(error.to_string()))?;
        Ok(pids
            .into_iter()
            .filter_map(|pid| {
                let bsd = bsd_info(pid)?;
                Some(ProcessBrief {
                    id: ProcessId {
                        pid,
                        start: start_of(&bsd),
                    },
                    ppid: bsd.pbi_ppid,
                    uid: bsd.pbi_uid,
                })
            })
            .collect())
    }

    fn process_info(&self, pid: u32) -> Option<ProcessInfo> {
        let bsd = bsd_info(pid)?;
        if bsd.pbi_uid != self.uid {
            return None;
        }
        let details = read_details(std::iter::once(pid))
            .remove(&pid)
            .unwrap_or_default();
        Some(build_info(pid, &bsd, details))
    }
}

fn bsd_info(pid: u32) -> Option<BSDInfo> {
    let pid = i32::try_from(pid).ok()?;
    pidinfo::<BSDInfo>(pid, 0).ok()
}

/// What `sysinfo` could read about one process; empty when it could not.
#[derive(Debug, Default)]
struct Details {
    argv: Vec<String>,
    cwd: Option<PathBuf>,
}

fn build_info(pid: u32, bsd: &BSDInfo, details: Details) -> ProcessInfo {
    ProcessInfo {
        id: ProcessId {
            pid,
            start: start_of(bsd),
        },
        ppid: bsd.pbi_ppid,
        pgid: bsd.pbi_pgid,
        uid: bsd.pbi_uid,
        argv: details.argv,
        cwd: details.cwd,
    }
}

fn start_of(bsd: &BSDInfo) -> ProcessStart {
    ProcessStart(
        bsd.pbi_start_tvsec
            .saturating_mul(1_000_000)
            .saturating_add(bsd.pbi_start_tvusec),
    )
}

/// TCP ports the process listens on, sorted and without repeats.
fn listening_ports(pid: u32, bsd: &BSDInfo) -> Vec<u16> {
    let Ok(raw_pid) = i32::try_from(pid) else {
        return Vec::new();
    };
    let Ok(fds) = listpidinfo::<ListFDs>(raw_pid, bsd.pbi_nfiles as usize) else {
        return Vec::new();
    };
    let ports: BTreeSet<u16> = fds
        .iter()
        .filter(|fd| matches!(ProcFDType::from(fd.proc_fdtype), ProcFDType::Socket))
        .filter_map(|fd| pidfdinfo::<SocketFDInfo>(raw_pid, fd.proc_fd).ok())
        .filter_map(|socket| listening_port(&socket))
        .collect();
    ports.into_iter().collect()
}

/// The local port of `socket` if it is a TCP socket in the `LISTEN` state.
fn listening_port(socket: &SocketFDInfo) -> Option<u16> {
    if !matches!(
        SocketInfoKind::from(socket.psi.soi_kind),
        SocketInfoKind::Tcp
    ) {
        return None;
    }
    // SAFETY: `soi_kind` is `Tcp`, so the kernel filled the `pri_tcp` member of
    // the union. `TcpSockInfo` is `Copy`, so this is a plain read of that member.
    let tcp = unsafe { socket.psi.soi_proto.pri_tcp };
    if !matches!(TcpSIState::from(tcp.tcpsi_state), TcpSIState::Listen) {
        return None;
    }
    // The kernel stores the port in network byte order in the low 16 bits.
    let raw = u16::try_from(tcp.tcpsi_ini.insi_lport & 0xFFFF).ok()?;
    Some(u16::from_be(raw))
}

/// Command lines and working directories of `pids`, read in one pass. A pid
/// whose details cannot be read is absent from the map.
fn read_details(pids: impl Iterator<Item = u32>) -> HashMap<u32, Details> {
    let targets: Vec<Pid> = pids.map(Pid::from_u32).collect();
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&targets),
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always),
    );
    targets
        .iter()
        .filter_map(|pid| {
            let process = system.process(*pid)?;
            let argv = process
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy().into_owned())
                .collect();
            let cwd = process
                .cwd()
                .map(std::path::Path::to_path_buf)
                .filter(|path| !path.as_os_str().is_empty());
            Some((pid.as_u32(), Details { argv, cwd }))
        })
        .collect()
}
