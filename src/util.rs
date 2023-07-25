use core::future::Future;
use std::io::Result as IoResult;
use std::process::Output;
use std::{ffi::OsStr, fmt::Display};
use tokio::process::Command;

pub struct RunError {
    program: Box<String>,
    error: std::io::Error,
}
impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Expected to run {}, but failed abruptly: {}.",
            self.program, self.error
        )
    }
}
pub type RunResult<T> = Result<T, RunError>;
pub type RunStringResult = RunResult<String>;

/// This "couples" the program execution ([Future]), and the program's executable & arguments
/// ([ProgramAndArgs]). Why? So that we can later handle success & failure by the same function.
pub struct RunProgress<P, A, I, F>
where
    F: Future<Output = IoResult<Output>>,
    P: AsRef<OsStr> + Clone + Display,
    A: AsRef<OsStr> + Clone + Display,
    I: Iterator<Item = A> + Clone,
{
    program: ProgramAndArgs<P, A, I>,
    command: Command,
    /// This will always be [Some], but it has to be an [Option] because of ownership of `command`.
    future: Option<F>,
}

#[derive(Clone)]
pub struct ProgramAndArgs<P, A, I>
where
    P: AsRef<OsStr> + Clone + Display,
    A: AsRef<OsStr> + Clone + Display,
    I: Iterator<Item = A> + Clone,
{
    program: P,
    args: I,
}

impl<P, A, I> ProgramAndArgs<P, A, I>
where
    P: AsRef<OsStr> + Clone + Display,
    A: AsRef<OsStr> + Clone + Display,
    I: Iterator<Item = A> + Clone,
{
    /// Create a new [Command] based on `program` and `args`. Set it to `kill_on_drop`.
    fn command(&self) -> Command {
        let mut command = Command::new(self.program.clone());
        command.kill_on_drop(true);
        command.args(self.args.clone());
        command
    }

    pub fn run(self) -> RunProgress<P, A, I, impl Future<Output = IoResult<Output>>> {
        let mut run_progress = RunProgress {
            command: self.command(),
            program: self,
            future: None,
        };
        run_progress.future = Some(run_progress.command.output());
        run_progress
    }
}

impl<P, A, I, F> RunProgress<P, A, I, F>
where
    F: Future<Output = IoResult<Output>>,
    P: AsRef<OsStr> + Clone + Display,
    A: AsRef<OsStr> + Clone + Display,
    I: Iterator<Item = A> + Clone,
{
    pub async fn complete(self) -> RunStringResult {
        if let Some(future) = self.future {
            let out = future.await;
            let out = out.map_err(|error| RunError {
                program: Box::new(self.program.to_string()),
                error,
            })?;
            Ok("OUT:\n".to_owned()
                + &ascii_bytes_to_string(out.stdout)
                + "\n\nERR:\n"
                + &ascii_bytes_to_string(out.stderr))
        } else {
            unreachable!();
        }
    }
}

impl<P, A, I> Display for ProgramAndArgs<P, A, I>
where
    P: AsRef<OsStr> + Clone + Display,
    A: AsRef<OsStr> + Clone + Display,
    I: Iterator<Item = A> + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.program)?;
        let args = self.args.clone().collect::<Vec<_>>();
        for a in args {
            write!(f, " {}", a)?;
        }
        Ok(())
    }
}

pub fn assert_linux() {
    assert!(cfg!(target_os = "linux"), "For Linux only.");
}

pub fn ascii_bytes_to_string(bytes: Vec<u8>) -> String {
    let mut result = String::with_capacity(bytes.len());
    for byte in bytes {
        result.push(char::from(byte));
    }
    result
}

/// Start the program, with any arguments or other adjustments done in `modify` closure. Kill on
/// drop.
///
/// On success, return the program's output, treated as ASCII.
pub async fn modify_and_run<P, F>(program: &P, modify: F) -> RunStringResult
where
    P: AsRef<OsStr> + Display,
    F: Fn(&mut Command),
{
    let mut command: Command = loop {}; //command(program); // @TODO
    modify(&mut command);
    let out = command.output().await.map_err(|error| RunError {
        program: Box::new(program.to_string()),
        error,
    })?;
    Ok(ascii_bytes_to_string(out.stdout))
}

pub async fn run<P, A, I>(mut command: Command) -> RunStringResult
where
    P: AsRef<OsStr> + Clone + Display,
    A: AsRef<OsStr> + Clone + Display,
    I: Iterator<Item = A> + Clone,
{
    let out = command.output().await.map_err(|error| RunError {
        program: Box::new("TODO program".to_string()),
        error,
    })?;
    Ok(ascii_bytes_to_string(out.stdout))
}

pub fn where_is(
    program_to_locate: &'static str,
) -> RunProgress<
    &'static str,
    &'static str,
    impl Iterator<Item = &'static str> + Clone,
    impl Future<Output = IoResult<Output>>,
> {
    // - whereis, /bin/whereis, /usr/bin/whereis fail on Deta.Space
    // - which, /bin/which, /usr/bin/which fail, too.
    // - only /usr/bin/which is present.
    let program_and_args = ProgramAndArgs {
        program: "/usr/bin/which",
        args: [program_to_locate].into_iter(),
    };
    program_and_args.run()
}

/// Used to locate binaries. Why? See comments inside [content].
pub async fn content_locate_binaries() -> String {
    let free = where_is("free");
    let df = where_is("df");
    let mount = where_is("mount");
    let tree = where_is("tree");
    let (free, df, mount, tree) = (
        free.complete().await,
        df.complete().await,
        mount.complete().await,
        tree.complete().await,
    );
    stringify_errors(
        || Ok((free?, df?, mount?, tree?)),
        |(free, df, mount, tree)| "".to_owned() + &free + "\n" + &df + "\n" + &mount + "\n" + &tree,
    )
}

pub async fn content_ls() -> String {
    /*
    let ls_current = run("ls", |_| ());
    let ls_root = run("ls", |prog| {
        prog.arg("/");
    });
    let (ls_current, ls_root) = (ls_current.await, ls_root.await);
    stringify_errors(
        || Ok((ls_current?, ls_root?)),
        |(ls_current, ls_root)| {
            "ls current dir:\n".to_owned() + &ls_current + "\nls root:\n" + &ls_root
        },
    )*/
    "TODO".to_owned()
}

/// Invoke the given `generator`. If successful, pass its result to `formatter`. If an error, format
/// the first error (as reported by `generator`) into [String]. Useful especially when you have
/// multiple [RunStringResult] instances, when it's ergonomic to use `?` short circuit operator, but you
/// can't use it. What are such situations? When we want to use `?` operator in a function returning
/// [String].
///
/// The first parameter (closure `source`) has to be [FnOnce], and not [Fn], because [RunError] and
/// hence [RunResult<T>] is not [Copy].
pub fn stringify_errors<T, G, F>(generator: G, formatter: F) -> String
where
    G: FnOnce() -> RunResult<T>,
    F: Fn(T) -> String,
{
    match generator() {
        Ok(content) => formatter(content),
        Err(err) => format!("{err}"),
    }
}
