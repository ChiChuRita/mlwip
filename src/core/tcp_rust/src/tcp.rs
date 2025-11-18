use std::collections::HashMap;

pub const DEFAULT_SND_BUF: u16 = 1_072;
pub const DEFAULT_RCV_WND: u16 = 20 * 1024;
pub const DEFAULT_PRIO: u8 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IpAddr(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    Established,
}

#[derive(Clone, Debug)]
pub struct TcpConn {
    pub local_ip: IpAddr,
    pub remote_ip: IpAddr,
    pub local_port: u16,
    pub remote_port: u16,
    pub state: TcpState,
    pub snd_buf: u16,
    pub snd_queuelen: u16,
    pub rcv_wnd: u16,
    pub prio: u8,
    pub listening: bool,
}

impl Default for TcpConn {
    fn default() -> Self {
        Self {
            local_ip: IpAddr(0),
            remote_ip: IpAddr(0),
            local_port: 0,
            remote_port: 0,
            state: TcpState::Closed,
            snd_buf: DEFAULT_SND_BUF,
            snd_queuelen: 0,
            rcv_wnd: DEFAULT_RCV_WND,
            prio: DEFAULT_PRIO,
            listening: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpError {
    Arg,
    Mem,
}

pub type TcpResult<T = ()> = Result<T, TcpError>;

pub struct TcpRuntime {
    timer_active: bool,
    timer_counter: u8,
    conns: HashMap<usize, TcpConn>,
}

impl TcpRuntime {
    pub fn new() -> Self {
        Self {
            timer_active: false,
            timer_counter: 0,
            conns: HashMap::new(),
        }
    }

    pub fn start_timer_if_needed(&mut self) -> bool {
        if self.timer_active {
            false
        } else {
            self.timer_active = true;
            true
        }
    }

    /// Returns true whenever the slow timer should run.
    pub fn on_fast_timer(&mut self) -> bool {
        self.timer_counter = self.timer_counter.wrapping_add(1);
        (self.timer_counter & 1) != 0
    }

    pub fn register_conn(&mut self, handle: usize) -> &mut TcpConn {
        self.conns.entry(handle).or_insert_with(TcpConn::default)
    }

    pub fn unregister_conn(&mut self, handle: usize) {
        self.conns.remove(&handle);
    }

    pub fn conn(&self, handle: usize) -> Option<&TcpConn> {
        self.conns.get(&handle)
    }

    pub fn conn_mut(&mut self, handle: usize) -> Option<&mut TcpConn> {
        self.conns.get_mut(&handle)
    }

    pub fn bind(&mut self, handle: usize, addr: Option<IpAddr>, port: u16) -> TcpResult {
        let conn = self.conns.get_mut(&handle).ok_or(TcpError::Arg)?;
        if let Some(ip) = addr {
            conn.local_ip = ip;
        }
        conn.local_port = port;
        Ok(())
    }

    pub fn connect(
        &mut self,
        handle: usize,
        addr: Option<IpAddr>,
        port: u16,
    ) -> TcpResult {
        let conn = self.conns.get_mut(&handle).ok_or(TcpError::Arg)?;
        if let Some(ip) = addr {
            conn.remote_ip = ip;
        }
        conn.remote_port = port;
        conn.state = TcpState::Established;
        Ok(())
    }

    pub fn write(&mut self, handle: usize, len: u16) -> TcpResult {
        let conn = self.conns.get_mut(&handle).ok_or(TcpError::Arg)?;
        if len > conn.snd_buf {
            return Err(TcpError::Mem);
        }
        conn.snd_buf = conn.snd_buf.saturating_sub(len);
        conn.snd_queuelen = conn.snd_queuelen.saturating_add(1);
        Ok(())
    }

    pub fn output(&mut self, handle: usize) -> TcpResult {
        let conn = self.conns.get_mut(&handle).ok_or(TcpError::Arg)?;
        conn.snd_buf = DEFAULT_SND_BUF;
        conn.snd_queuelen = 0;
        Ok(())
    }

    pub fn recved(&mut self, handle: usize, len: u16) -> TcpResult {
        let conn = self.conns.get_mut(&handle).ok_or(TcpError::Arg)?;
        let new_wnd = conn
            .rcv_wnd
            .saturating_add(len)
            .min(DEFAULT_RCV_WND);
        conn.rcv_wnd = new_wnd;
        Ok(())
    }

    pub fn listen(&mut self, handle: usize) -> TcpResult {
        let conn = self.conns.get_mut(&handle).ok_or(TcpError::Arg)?;
        conn.state = TcpState::Listen;
        conn.listening = true;
        Ok(())
    }

    pub fn set_priority(&mut self, handle: usize, prio: u8) -> TcpResult {
        let conn = self.conns.get_mut(&handle).ok_or(TcpError::Arg)?;
        conn.prio = prio;
        Ok(())
    }
}
