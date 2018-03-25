use super::{LockableWrite, Output};
use core::errors::*;
use core::{self, ErrorPos};
use log;
use std::io;

pub struct NonColored<T> {
    out: T,
}

pub struct NonColoredLogger<T> {
    out: T,
}

impl<T> NonColored<T>
where
    T: LockableWrite,
{
    pub fn new(out: T) -> NonColored<T> {
        NonColored { out: out }
    }

    fn print_positional(&self, m: &str, p: &ErrorPos) -> Result<()> {
        use std::cmp::max;
        use std::iter::repeat;

        let mut o = self.out.lock();

        let (line_str, line, (s, e)) = core::utils::find_line(p.object.read()?, (p.start, p.end))?;

        let line_no = format!("{:>3}:", line + 1);

        let mut indicator = String::new();

        indicator.extend(repeat(' ').take(line_no.len() + s + 1));
        indicator.extend(repeat('^').take(max(1, e - s)));

        writeln!(o, "{}:{}:{}-{}:", p.object, line + 1, s + 1, e + 1)?;
        writeln!(o, "{} {}", line_no, line_str)?;
        writeln!(o, "{}{}{}", indicator, " - ", m)?;

        Ok(())
    }
}

impl<T> log::Log for NonColoredLogger<T>
where
    T: LockableWrite,
{
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= log::LogLevel::Debug
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            let mut out = self.out.lock();
            writeln!(out, "{}: {}", record.level(), record.args()).unwrap();
        }
    }
}

impl<T> Output for NonColored<T>
where
    T: 'static + LockableWrite,
{
    fn lock<'a>(&'a self) -> Box<io::Write + 'a> {
        self.out.lock()
    }

    fn logger(&self) -> Box<log::Log + 'static> {
        Box::new(NonColoredLogger {
            out: self.out.open_new(),
        })
    }

    fn print(&self, m: &str) -> Result<()> {
        let mut o = self.out.lock();
        writeln!(o, "ERROR: {}", m)?;
        Ok(())
    }

    fn print_info(&self, m: &str, p: &ErrorPos) -> Result<()> {
        self.print_positional(m, p)
    }

    fn print_error(&self, m: &str, p: &ErrorPos) -> Result<()> {
        self.print_positional(m, p)
    }
}
