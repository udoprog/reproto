use std::fmt;
use std::io;

pub trait InternalLog {
    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()>;
}

impl<W> InternalLog for Option<W>
where
    W: io::Write,
{
    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()> {
        match *self {
            Some(ref mut w) => w.write_fmt(args),
            None => Ok(()),
        }
    }
}
