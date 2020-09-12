use std::env;
use std::ffi;
use std::fs;
use std::io::{self, BufRead, Write};
use std::iter::Iterator;
use std::os::unix::{ffi::OsStrExt, io::AsRawFd};
use std::path::{Path, PathBuf};
use std::process;

use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Error, Debug)]
enum Error {
    #[error("invalid file name list")]
    InvalidFileNameList,
    #[error("editor exited with non-zero return code")]
    EditorError,
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn source_files() -> io::Result<Vec<PathBuf>> {
    let files: Vec<PathBuf> = env::args().skip(1).map(From::from).collect();
    if files.is_empty() {
        io::stdin()
            .lock()
            .lines()
            .map(|line| line.map(From::from))
            .collect()
    } else {
        Ok(files)
    }
}

fn destination_files<P>(temp_path: P) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    io::BufReader::new(fs::File::open(temp_path)?)
        .lines()
        .filter(|line| line.as_ref().map(|line| !line.is_empty()).unwrap_or(true))
        .map(|line| line.map(From::from))
        .collect()
}

fn spawn_editor<P>(path: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let editor = std::env::var("EDITOR").unwrap_or("vi".into());
    let mut command = process::Command::new(editor);
    command.arg(path.as_ref());
    if unsafe {
        libc::isatty(io::stdin().lock().as_raw_fd()) == 0
    } {
        command.stdin(fs::File::open("/dev/tty")?);
    }
    if command
        .status()?
        .success()
    {
        Ok(())
    } else {
        Err(Error::EditorError)
    }
}

fn write_lines<L, T>(destination: &Path, lines: &mut L) -> io::Result<()>
where
    L: Iterator<Item = T>,
    T: AsRef<[u8]>,
{
    let mut writer = io::BufWriter::new(fs::OpenOptions::new().write(true).open(destination)?);
    lines.try_for_each(|line| {
        writer
            .write_all(line.as_ref())
            .and_then(|_| writer.write_all(&['\n' as u8]))
    })
}

fn bulk_rename<P>(source_files: &[P]) -> Result<usize, Error>
where
    P: AsRef<Path>,
{
    let temp = NamedTempFile::new()?;
    write_lines(
        temp.path(),
        &mut source_files
            .iter()
            .map(|path| AsRef::<ffi::OsStr>::as_ref(path.as_ref()).as_bytes()),
    )?;
    spawn_editor(temp.path())?;
    let destination_files = destination_files(temp.path())?;
    if destination_files.len() != source_files.len() {
        return Err(Error::InvalidFileNameList);
    }
    let mut count = 0;
    source_files
        .iter()
        .zip(destination_files.iter())
        .try_for_each(|(source, destination)| -> Result<(), Error> {
            if source.as_ref() != destination {
                fs::rename(source, destination)?;
                count += 1;
            }
            Ok(())
        })?;
    Ok(count)
}

fn run() -> Result<(), Error> {
    let source_files = source_files()?;
    if source_files.is_empty() {
        return Ok(());
    }
    let count = bulk_rename(source_files.as_ref())?;
    println!("{} files renamed", count);
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("bulkrename: {}", err);
        process::exit(1);
    }
}
