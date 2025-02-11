use amethyst::Error;

use std::{
    env::current_exe,
    net::SocketAddr,
    process::{Child, Command},
    time::Instant,
};

pub struct LastAcknowledgedUpdate {
    pub id: u64,
    pub frame_number: u64,
}

pub struct ServerCommand {
    process: Option<ServerProcess>,
}

impl ServerCommand {
    pub fn new() -> Self {
        Self { process: None }
    }

    pub fn start(&mut self, addr: SocketAddr) -> Result<(), Error> {
        self.process = Some(ServerProcess::new(addr)?);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn kill(&mut self) {
        self.process = None;
    }

    pub fn process(&self) -> Option<&ServerProcess> {
        self.process.as_ref()
    }
}

pub struct ServerProcess {
    cmd: Child,
    addr: SocketAddr,
    created_at: Instant,
}

impl ServerProcess {
    pub fn new(addr: SocketAddr) -> Result<Self, Error> {
        let executable_path = {
            let mut path = current_exe()?;
            path.pop();
            path.join("gv_server")
        };

        let cmd = Command::new(executable_path)
            .arg("--addr")
            .arg(addr.to_string())
            .spawn()?;

        Ok(ServerProcess {
            cmd,
            addr,
            created_at: Instant::now(),
        })
    }

    pub fn socket_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn created_at(&self) -> Instant {
        self.created_at
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        if self.cmd.kill().is_err() {
            log::warn!(
                "Tried to kill the ServerProcess (id: {}) which wasn't running",
                self.cmd.id()
            );
        }
    }
}
