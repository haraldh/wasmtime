use crate::clocks::WasiClocks;
use crate::dir::{DirCaps, DirEntry, WasiDir};
use crate::file::{FileCaps, FileEntry, WasiFile};
use crate::sched::WasiSched;
use crate::string_array::{StringArray, StringArrayError};
use crate::table::Table;
use crate::Error;
use cap_rand::RngCore;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct WasiCtx {
    pub args: StringArray,
    pub env: StringArray,
    pub random: &'static (dyn Fn() -> Box<dyn RngCore + Send + Sync> + Send + Sync),
    pub clocks: WasiClocks,
    pub sched: Box<dyn WasiSched>,
    pub table: Table,
    pub unix_pair: &'static (dyn Fn(&Self) -> Result<(u32, u32), Error> + Send + Sync),
}

impl WasiCtx {
    pub fn new(
        random: &'static (dyn Fn() -> Box<dyn RngCore + Send + Sync> + Send + Sync),
        clocks: WasiClocks,
        sched: Box<dyn WasiSched>,
        table: Table,
        unix_pair: &'static (dyn Fn(&Self) -> Result<(u32, u32), Error> + Send + Sync),
    ) -> Self {
        let s = WasiCtx {
            args: StringArray::new(),
            env: StringArray::new(),
            random,
            clocks,
            sched,
            table,
            unix_pair,
        };
        s.set_stdin(Box::new(crate::pipe::ReadPipe::new(std::io::empty())));
        s.set_stdout(Box::new(crate::pipe::WritePipe::new(std::io::sink())));
        s.set_stderr(Box::new(crate::pipe::WritePipe::new(std::io::sink())));
        s
    }

    pub fn insert_file(&self, fd: u32, file: Box<dyn WasiFile>, caps: FileCaps) {
        self.table()
            .insert_at(fd, Arc::new(FileEntry::new(caps, file)));
    }

    pub fn push_file(&self, file: Box<dyn WasiFile>, caps: FileCaps) -> Result<u32, Error> {
        self.table().push(Arc::new(FileEntry::new(caps, file)))
    }

    pub fn insert_dir(
        &self,
        fd: u32,
        dir: Box<dyn WasiDir>,
        caps: DirCaps,
        file_caps: FileCaps,
        path: PathBuf,
    ) {
        self.table().insert_at(
            fd,
            Arc::new(DirEntry::new(caps, file_caps, Some(path), dir)),
        );
    }

    pub fn push_dir(
        &self,
        dir: Box<dyn WasiDir>,
        caps: DirCaps,
        file_caps: FileCaps,
        path: PathBuf,
    ) -> Result<u32, Error> {
        self.table()
            .push(Arc::new(DirEntry::new(caps, file_caps, Some(path), dir)))
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn push_arg(&mut self, arg: &str) -> Result<(), StringArrayError> {
        self.args.push(arg.to_owned())
    }

    pub fn push_env(&mut self, var: &str, value: &str) -> Result<(), StringArrayError> {
        self.env.push(format!("{}={}", var, value))?;
        Ok(())
    }

    pub fn set_stdin(&self, mut f: Box<dyn WasiFile>) {
        let rights = Self::stdio_rights(&mut *f);
        self.insert_file(0, f, rights);
    }

    pub fn set_stdout(&self, mut f: Box<dyn WasiFile>) {
        let rights = Self::stdio_rights(&mut *f);
        self.insert_file(1, f, rights);
    }

    pub fn set_stderr(&self, mut f: Box<dyn WasiFile>) {
        let rights = Self::stdio_rights(&mut *f);
        self.insert_file(2, f, rights);
    }

    fn stdio_rights(f: &mut dyn WasiFile) -> FileCaps {
        let mut rights = FileCaps::all();

        // If `f` is a tty, restrict the `tell` and `seek` capabilities, so
        // that wasi-libc's `isatty` correctly detects the file descriptor
        // as a tty.
        if f.isatty() {
            rights &= !(FileCaps::TELL | FileCaps::SEEK);
        }

        rights
    }

    pub fn push_preopened_dir(
        &self,
        dir: Box<dyn WasiDir>,
        path: impl AsRef<Path>,
    ) -> Result<(), Error> {
        let caps = DirCaps::all();
        let file_caps = FileCaps::all();
        self.table().push(Arc::new(DirEntry::new(
            caps,
            file_caps,
            Some(path.as_ref().to_owned()),
            dir,
        )))?;
        Ok(())
    }

    pub fn unix_pair(&self) -> Result<(u32, u32), Error> {
        (self.unix_pair)(self)
    }
}
