// SPDX-License-Identifier: GPL-3.0-or-later
//! How a [`Document`](crate::Document) is rendered: in a child process (default) or in this
//! process (weak; for `wasm32`, iOS and tvOS, which cannot start child processes).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Environment variable that names the render worker program (see [`WorkerSpec::discover`]).
pub const WORKER_ENV: &str = "COMETCAST_DOCS_WORKER";

/// File name of the worker program built from this crate (`cc-docs-worker`).
pub const WORKER_FILE_NAME: &str = if cfg!(windows) {
    "cc-docs-worker.exe"
} else {
    "cc-docs-worker"
};

/// The program that renders in a child process: a path and its fixed arguments. The program
/// must call [`worker_main`](crate::worker_main) (the `cc-docs-worker` binary of this crate
/// does; so does `cometcast-cli docs-worker`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSpec {
    /// Program to start.
    pub program: PathBuf,
    /// Arguments placed before nothing else (the request travels on stdin).
    pub args: Vec<OsString>,
}

impl WorkerSpec {
    /// A worker that is started as `program` without arguments.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    /// Add an argument.
    #[must_use]
    pub fn arg(mut self, a: impl Into<OsString>) -> Self {
        self.args.push(a.into());
        self
    }

    /// The running executable itself, started with one argument (for example the hidden
    /// `docs-worker` subcommand of `cometcast-cli`). The executable must handle that argument
    /// by calling [`worker_main`](crate::worker_main), otherwise renders fail.
    pub fn current_exe_with_arg(arg: impl Into<OsString>) -> std::io::Result<Self> {
        Ok(Self::new(std::env::current_exe()?).arg(arg))
    }

    /// Find a worker without help from the caller. In this order:
    /// 1. the program named by the environment variable `COMETCAST_DOCS_WORKER`;
    /// 2. a file `cc-docs-worker` next to the running executable;
    /// 3. the same file one directory up when the executable is in a `deps` directory (the
    ///    layout of Cargo test binaries).
    ///
    /// `None` when nothing is found. Applications that cannot guarantee any of these (for
    /// example a Flutter app) should ship the worker and pass the path explicitly with
    /// [`Isolation::Process`].
    pub fn discover() -> Option<Self> {
        if let Some(p) = std::env::var_os(WORKER_ENV).filter(|p| !p.is_empty()) {
            let p = PathBuf::from(p);
            return p.is_file().then(|| Self::new(p));
        }
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        let mut candidates = vec![dir.join(WORKER_FILE_NAME)];
        if dir.file_name().is_some_and(|n| n == "deps")
            && let Some(up) = dir.parent()
        {
            candidates.push(up.join(WORKER_FILE_NAME));
        }
        candidates
            .into_iter()
            .find(|c| Path::new(c).is_file())
            .map(Self::new)
    }
}

/// Where the renderer runs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Isolation {
    /// Child process where the platform allows it, found with [`WorkerSpec::discover`]
    /// (`WorkerUnavailable` error if none is found); the in-process engine on `wasm32`,
    /// iOS and tvOS. This is the default.
    #[default]
    Auto,
    /// Child process with this worker program.
    Process(WorkerSpec),
    /// In this process on a thread. **Memory is not bounded** and a stuck render keeps a
    /// thread; see `docs/documents.md`. Only for platforms without child processes and for
    /// trusted input.
    InProcess,
}
