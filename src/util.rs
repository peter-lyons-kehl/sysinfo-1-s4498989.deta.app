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
pub type TwinResult<T> = Result<T, T>;
pub type StringTwinResult = TwinResult<String>;

/// Like [TwinResult], but instead of being an `enum`, this has the Ok/Err discriminant exposed as a
/// bool flag `is_ok`, and the wrapped value exposed as `value`.
pub struct FlagTwin<T> {
    pub is_ok: bool,
    pub value: T,
}
pub type FlagString = FlagTwin<String>;
pub type FlagTwinTuple<T> = (bool, T);

pub fn twin_result_value<T>(r: TwinResult<T>) -> T {
    match r {
        Ok(value) => value,
        Err(value) => value,
    }
}

impl<T> FlagTwin<T> {
    fn new(r: TwinResult<T>) -> Self {
        Self {
            is_ok: r.is_ok(),
            value: twin_result_value(r),
        }
    }
    fn chain<P>(self, next: TwinResult<P>) -> FlagTwin<(T, P)> {
        let is_ok = self.is_ok && next.is_ok();
        let next_value = twin_result_value(next);
        FlagTwin {
            is_ok,
            value: (self.value, next_value),
        }
    }
    fn and<P>(self, next: FlagTwin<P>) -> FlagTwin<(T, P)> {
        FlagTwin {
            is_ok: self.is_ok && next.is_ok,
            value: (self.value, next.value),
        }
    }
    // @TODO consider: -> (bool, T)
    fn get(self) -> FlagTwinTuple<T> {
        (self.is_ok, self.value)
    }
}
/// It's redundant, but it makes code somewhat ergonomic.
impl<T> FlagTwin</*FlagTwinTuple<*/ T> {
    fn pair<P>(self, next: FlagTwin<P>) -> FlagTwin<(FlagTwinTuple<T>, FlagTwinTuple<P>)> {
        FlagTwin {
            is_ok: self.is_ok && next.is_ok,
            value: ((self.is_ok, self.value), (next.is_ok, next.value)),
        }
    }
}
pub type FlagStringTwin = FlagTwin<String>;

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

    pub async fn complete_chainable(self) -> FlagStringTwin {
        if let Some(future) = self.future {
            let out = future.await;
            let out = out.map_err(|error| RunError {
                program: Box::new(self.program.to_string()),
                error,
            });
            match out {
                Ok(out) => FlagStringTwin::new(Ok("OUT:\n".to_owned()
                    + &ascii_bytes_to_string(out.stdout)
                    + "\n\nERR:\n"
                    + &ascii_bytes_to_string(out.stderr))),
                Err(err) => FlagStringTwin::new(Err(err.to_string())),
            }
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

pub fn run<I>(
    program: &'static str,
    args: I,
) -> RunProgress<&'static str, &'static str, I, impl Future<Output = IoResult<Output>>>
where
    I: Iterator<Item = &'static str> + Clone, //where I: IntoIterator<Item = &'static str> + Clone
{
    let program_and_args = ProgramAndArgs { program, args };
    program_and_args.run()
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
    run("/usr/bin/which", [program_to_locate].into_iter())
}

/// Used to locate binaries. Why? See comments inside [content].
pub async fn content_locate_binaries() -> StringTwinResult {
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
    all_ok_formatted_or_first_error(
        || Ok((free?, df?, mount?, tree?)),
        |(free, df, mount, tree)| "".to_owned() + &free + "\n" + &df + "\n" + &mount + "\n" + &tree,
    )
}

pub fn ls(
    path_to_ls: &'static str,
) -> RunProgress<
    &'static str,
    &'static str,
    impl Iterator<Item = &'static str> + Clone,
    impl Future<Output = IoResult<Output>>,
> {
    run("ls", [path_to_ls].into_iter())
}

pub async fn content_ls() -> StringTwinResult {
    let current = run("ls", [].into_iter());
    let root = ls("/");
    let bin = ls("/bin");
    let sbin = ls("/sbin");

    let (current, root, bin, sbin) = (
        current.complete().await,
        root.complete().await,
        bin.complete().await,
        sbin.complete().await,
    );
    all_ok_formatted_or_first_error(
        || Ok((current?, root?, bin?, sbin?)),
        |(current, root, bin, sbin)| {
            "ls current dir:\n".to_owned()
                + &current
                + "\n\nls /:\n"
                + &root
                + "\n\nls /bin:\n"
                + &bin
                + "\n\nls /sbin:\n"
                + &sbin
        },
    )
}

pub async fn content_ls2() -> StringTwinResult {
    let current = run("ls", [].into_iter());
    let root = ls("/");
    let bin = ls("/bin");
    let sbin = ls("/sbin");

    let (current, root, bin, sbin) = (
        current.complete_chainable().await,
        root.complete_chainable().await,
        bin.complete_chainable().await,
        sbin.complete_chainable().await,
    );
    let chained = current.and(root).and(bin).and(sbin);

    let (ok_err, (((current, root), bin), sbin)) = chained.get();
    // TODO: Consider (bool, String) for each part:
    //
    // let (ok_err, ((((current_ok, current), (root_ok, root)), (bin_ok, bin)), (sbin_ok, sbin))) =
    // chained.get();
    //
    // OR: Do return the structure containing (bool, String) pairs, but don't split them - keep them
    // together, and pass them to a formatter:
    //
    // let (ok_err, (((current, root), bin), sbin)) = chained.get();
    //
    // twin_result(ok_err, "Current:\n" +formatted(current) +... )
    //
    //all_ok_or_any_errors
    todo!()
}

/// Invoke the given `generator`. If successful, pass its result to `formatter`. If an error, format
/// the first error (as reported by `generator`) into [String]. Useful especially when you have
/// multiple [RunStringResult] instances, when it's ergonomic to use `?` short circuit operator, but
/// you can't use it. What are such situations? When we want to use `?` operator in a function
/// returning [String].
///
/// Do NOT use a simple `ok_filter`, like `Ok((current?, root?, bin?, sbin?))`, if you must have all
/// errors included.
///
/// The first parameter (closure `source`) has to be [FnOnce], and not [Fn], because [RunError] and
/// hence [RunResult<T>] is not [Copy].
pub fn all_ok_formatted_or_first_error<T, G, F>(ok_filter: G, ok_formatter: F) -> StringTwinResult
where
    G: FnOnce() -> RunResult<T>,
    F: Fn(T) -> String,
{
    match ok_filter() {
        Ok(content) => Ok(ok_formatter(content)),
        Err(err) => Err(err.to_string()),
    }
}
