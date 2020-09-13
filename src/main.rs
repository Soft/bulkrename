use std::env;
use std::ffi;
use std::fs;
use std::io::{self, BufRead, Write};
use std::iter::Iterator;
use std::os::raw;
use std::os::unix::{ffi::OsStrExt, io::AsRawFd};
use std::path::{Path, PathBuf};
use std::process;

use tempfile::NamedTempFile;
use thiserror::Error;

const USAGE: &str = r#"usage: bulkrename [-h|--help] [FILE]...
bulkrename is a tool for renaming large numbers of files.

options:
  -h, --help:        display this help
  -r, --replace:     allow replacing existing files
  -q, --quiet:       do not display information about operations being performed
"#;

#[derive(Error, Debug)]
enum Error {
    #[error("unknown option '{0}'")]
    UnknownOption(String),
    #[error("invalid file list")]
    InvalidFileList,
    #[error("editor exited with a non-zero return code")]
    Editor,
    #[error(transparent)]
    Io(#[from] io::Error),
}

struct Args {
    show_help: bool,
    replace: bool,
    quiet: bool,
    files: Vec<PathBuf>,
}

impl Args {
    fn parse() -> Result<Self, Error> {
        let mut args = Args {
            show_help: false,
            replace: false,
            quiet: false,
            files: vec![],
        };
        let mut iter = env::args().skip(1);
        for arg in &mut iter {
            match arg.as_ref() {
                "-h" | "--help" => args.show_help = true,
                "-r" | "--replace" => args.replace = true,
                "-q" | "--quiet" => args.quiet = true,
                "--" => break,
                flag if flag.starts_with('-') => return Err(Error::UnknownOption(flag.into())),
                file => {
                    args.files.push(From::from(file));
                    break;
                }
            }
        }
        args.files.extend(iter.map(From::from));
        Ok(args)
    }
}

fn source_files() -> io::Result<Vec<PathBuf>> {
    io::stdin()
        .lock()
        .lines()
        .map(|line| line.map(From::from))
        .collect()
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
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let mut command = process::Command::new(editor);
    command.arg(path.as_ref());
    if unsafe { libc::isatty(io::stdin().lock().as_raw_fd()) == 0 } {
        command.stdin(fs::File::open("/dev/tty")?);
    }
    if command.status()?.success() {
        Ok(())
    } else {
        Err(Error::Editor)
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
            .and_then(|_| writer.write_all(&[b'\n']))
    })
}

#[inline]
fn path_as_bytes<P>(path: &P) -> &[u8]
where
    P: AsRef<Path>,
{
    AsRef::<ffi::OsStr>::as_ref(path.as_ref()).as_bytes()
}

#[cfg(target_os = "linux")]
unsafe fn renameat2(
    old_dir_fd: raw::c_int,
    old_path: *const raw::c_char,
    new_dir_fd: raw::c_int,
    new_path: *const raw::c_char,
    flags: raw::c_uint,
) -> raw::c_int {
    libc::syscall(
        libc::SYS_renameat2,
        old_dir_fd,
        old_path,
        new_dir_fd,
        new_path,
        flags,
    ) as raw::c_int
}

#[cfg(target_os = "linux")]
fn rename<S, D>(source: S, dest: D, replace: bool) -> io::Result<()>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    let source = ffi::CString::new(path_as_bytes(&source))?;
    let dest = ffi::CString::new(path_as_bytes(&dest))?;
    let flags: raw::c_uint = if replace {
        0
    } else {
        libc::RENAME_NOREPLACE as raw::c_uint
    };
    if unsafe {
        renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            dest.as_ptr(),
            flags,
        )
    } == -1
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn rename<S, D>(source: S, dest: D, replace: bool) -> io::Result<()>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    if replace {
        fs::rename(source, dest)
    } else {
        fs::hard_link(&source, dest)?;
        fs::remove_file(source)
    }
}

fn bulk_rename<P>(source_files: &[P], replace: bool, quiet: bool) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let temp = NamedTempFile::new()?;
    write_lines(
        temp.path(),
        &mut source_files.iter().map(|path| path_as_bytes(path)),
    )?;
    spawn_editor(temp.path())?;
    let destination_files = destination_files(temp.path())?;
    if destination_files.len() != source_files.len() {
        return Err(Error::InvalidFileList);
    }
    let mut count = 0;
    source_files
        .iter()
        .zip(destination_files.iter())
        .try_for_each(|(source, destination)| -> Result<(), Error> {
            if source.as_ref() != destination {
                rename(source, destination, replace)?;
                if !quiet {
                    println!(
                        "renaming {} to {}",
                        source.as_ref().to_string_lossy(),
                        destination.to_string_lossy()
                    );
                }
                count += 1;
            }
            Ok(())
        })?;
    if !quiet {
        println!("{} files renamed", count);
    }
    Ok(())
}

fn run() -> Result<(), Error> {
    let args = Args::parse()?;
    if args.show_help {
        print!("{}", USAGE);
        return Ok(());
    }
    let source_files = if args.files.is_empty() {
        source_files()?
    } else {
        args.files
    };
    if source_files.is_empty() {
        return Ok(());
    }
    bulk_rename(source_files.as_ref(), args.replace, args.quiet)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("bulkrename: {}", err);
        process::exit(1);
    }
}
