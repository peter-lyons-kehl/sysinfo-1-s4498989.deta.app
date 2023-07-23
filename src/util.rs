use core::future::Future;
use std::{ffi::OsStr, fmt::Display};
use tokio::process::Command;

pub struct RunError {
    program: String,
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
pub type RunResult = Result<String, RunError>;

/* Generic parmas:
/// - P: program (executable) name/path
/// - A: argument (each)
/// - RC: reference to a collection (of arguments)
/// - C: collection (of arguments)
pub struct ProgramAndArgsNoClone<'a, P, A, RC, C>
where
    P: AsRef<OsStr> + ToString,
    A: AsRef<OsStr> + ToString + 'a,
    RC: Deref<Target = C>,
    II: IntoIterator<Item = &'a A>,
{
    program: P,
    args: I,
}*/

pub struct ProgramAndArgs<P, A, I>
where
    P: AsRef<OsStr> + Display,
    A: AsRef<OsStr> + Display,
    I: Iterator<Item = A> + Clone,
{
    program: P,
    args: I,
}

impl<P, A, I> ProgramAndArgs<P, A, I>
where
    P: AsRef<OsStr> + Clone + Display,
    A: AsRef<OsStr> + Display,
    I: Iterator<Item = A> + Clone,
{
    /// Create a new [Command] based on `program` and `args`. Set it to `kill_on_drop`.
    pub fn command(&self) -> Command {
        let mut command = Command::new(self.program.clone());
        command.kill_on_drop(true);
        command.args(self.args.clone());
        // OR:
        for a in self.args.clone().into_iter() {
            command.arg(a);
        }
        command
    }
}

impl<P, A, I> Display for ProgramAndArgs<P, A, I>
where
    P: AsRef<OsStr> + Display,
    A: AsRef<OsStr> + Display,
    I: Iterator<Item = A> + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.program)?;
        //self.args.clone().into
        Ok(())
    }
}

fn _use_program_and_args() {
    let program = "ls".to_owned();
    let args = vec!["-l".to_owned(), "-a".to_owned()];
    let args_str = vec!["-l", "-a"];
    #[allow(unused)]
    let pa = ProgramAndArgs {
        program: &program,
        args: args.iter(),
    };

    let v = vec!["hi", "mate"];
    let slice = &v[..];
    let mut slice_iter = slice.iter();
    slice_iter = slice.into_iter();
    // WOW: A result of `map` (of an Iterator that is Clone) IS itself Clone.
    let iter = slice_iter.map(|_| 1).clone();
    #[allow(unused)]
    iter.clone();
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
pub async fn run<P, F>(program: &P, modify: F) -> RunResult
where
    P: AsRef<OsStr> + ToString,
    F: Fn(&mut Command),
{
    let mut command: Command = loop {}; //command(program); // @TODO
    modify(&mut command);
    let out = command.output().await.map_err(|error| RunError {
        program: program.to_string(),
        error,
    })?;
    Ok(ascii_bytes_to_string(out.stdout))
}

pub async fn run_with_one_arg<P, F, A>(program: &P, arg: A) -> RunResult
where
    P: AsRef<OsStr> + ToString,
    F: Fn(&mut Command),
    A: AsRef<OsStr>,
{
    /*run(program, move |prog| {
        prog.arg(arg);
    })*/
    todo!()
}

/*pub fn where_is(program: &'static str) -> impl Future<Output = RunResult> {
    // - whereis, /bin/whereis, /usr/bin/whereis fail on Deta.Space
    // - which, /bin/which, /usr/bin/which fail, too:
    //
    // "No such file or directory (os error 2)."
    run("/usr/bin/which", move |prog| {
        prog.arg(program);
    })
}*/

/// Used to locate binaries. Why? See comments inside [content].
pub async fn content_locate_binaries() -> String {
    /*
    let free = where_is("free");
    let df = where_is("df");
    let mount = where_is("mount");
    let tree = where_is("tree");
    let (free, df, mount, tree) = (free.await, df.await, mount.await, tree.await);
    stringify_errors(
        || Ok((free?, df?, mount?, tree?)),
        |(free, df, mount, tree)| "".to_owned() + &free + "\n" + &df + "\n" + &mount + "\n" + &tree,
    )*/

    //Ok("".to_owned() + &free + "\n" + &df + "\n" + &mount); "".to_owned()
    todo!()
}

pub async fn ls() -> String {
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
    todo!()
}

/// Invoke the given `generator`. If successful, pass its result to `formatter`. If an error, format
/// into [String]. Useful especially when you have multiple [RunResult] instances, when it's
/// ergonomic to use `?` short circuit operator, but you can't use it. What are such situations?
/// When we want to use `?` operator in a function returning [String].
///
/// The first parameter (closure `source`) has to be [FnOnce], and not [Fn], because [RunError] and
/// hence [Result<T, RunError>] is not [Copy].
pub fn stringify_errors<T, S, F>(generator: S, formatter: F) -> String
where
    S: FnOnce() -> Result<T, RunError>,
    F: Fn(T) -> String,
{
    match generator() {
        Ok(content) => formatter(content),
        Err(err) => format!("{err}"),
    }
}

/* **
   ** Can't type these:

pub async fn invoke_and_handle_errors<R: Future<Output = RunResult>, F: Fn() -> R>(
    f: F,
) -> Box<impl Fn() -> dyn Future<Output = String>> {
    Box::new(async || {
        let result = f().await;
    match result {
        Ok(content) => content,
        Err(err) => format!("{err}"),
    }
    })
}

pub async fn invoke_and_handle_errors2<
    R: Future<Output = RunResult>,
    F: Fn() -> R,
    //RR: Fn() -> dyn Future<Output = String>,
>(
    f: F,
) -> Box<impl Fn() -> dyn Future<Output = String>> {
    Box::new(async || {
        let result = f().await;
    match result {
        Ok(content) => content,
        Err(err) => format!("{err}"),
    }
    })
}*/
