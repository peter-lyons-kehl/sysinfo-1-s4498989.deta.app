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
//pub type FlagString = FlagTwin<String>;
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

    fn link<P>(self, next: TwinResult<P>) -> FlagTwin<(T, P)> {
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
    fn split(self) -> FlagTwinTuple<T> {
        (self.is_ok, self.value)
    }

    /// Goal:
    /// 1. "Combine" the `is_ok` of any number (known in compile time) of [FlagTwin] instance(s), so
    ///    that we have an overall logical AND of their [FlagTwin::is_ok], yet we
    /// 2. Keep each "inner/original" [FlagTwin] instance's `is_ok` and `value`. Transform them into
    ///    [FlagTwinTuple], so that they can be more concisely assigned to local variables ("pattern
    ///    matched").
    ///
    /// The result's [FlagTwin::is_ok] is redundant, but it allows the consumer code to be shorter.
    ///
    /// This function "starts" the "chain".
    pub fn pairable(self) -> FlagTwinPairable<T> {
        FlagTwin {
            is_ok: self.is_ok,
            value: ((self.is_ok, self.value)),
        }
    }
    /// An alternative to separate methods [FlagTwin::pair_first] and [FlagTwin::pair]. But having
    /// just one method allowed more mistakes. How? This method would allows `T` to be of any type.
    /// Instead, [FlagTwin::pair_first] requires `T` to be [FlagTwinTuple].)
    ///
    /// (This method is here for documentation only. Not planned to be used.)
    #[allow(dead_code)]
    #[doc(hidden)]
    fn pair_one_method_instead_of_two<P>(
        self,
        next: FlagTwin<P>,
    ) -> FlagTwin<(T, FlagTwinTuple<P>)> {
        if true {
            panic!("Don't use.");
        }
        FlagTwin {
            is_ok: self.is_ok && next.is_ok,
            value: (self.value, (next.is_ok, next.value)),
        }
    }
}

// This doesn't need to exist as a `type` alias, but it helps with understanding.
pub type FlagTwinPairable<T> = FlagTwin<FlagTwinTuple<T>>;
impl<T> FlagTwinPairable<T> {
    pub fn pair_first<P>(self, next: FlagTwin<P>) -> FlagTwinPaired<FlagTwinTuple<T>, P> {
        FlagTwin {
            is_ok: self.is_ok && next.is_ok,
            value: (self.value, (next.is_ok, next.value)),
        }
    }
}

// This doesn't need to exist as a `type` alias, but it helps with understanding.
pub type FlagTwinPaired<T, P> = FlagTwin<(T, FlagTwinTuple<P>)>;
impl<T, P> FlagTwinPaired<T, P> {
    pub fn pair<R>(self, next: FlagTwin<R>) -> FlagTwinPaired<(T, FlagTwinTuple<P>), R> {
        FlagTwin {
            is_ok: self.is_ok && next.is_ok,
            value: (self.value, (next.is_ok, next.value)),
        }
    }
}
pub type FlagStringTwin = FlagTwin<String>;

/// Restructure into a chain.
///
/// Used with `let` keyword (in place of a variable name) for destructuring/pattern matching results
/// of [FlagTwinPairable::pair_first] and [FlagTwinPaired::pair] into local variables (whose names
/// are passed to this macro).
#[macro_export]
macro_rules! flatten {
    ($all_ok:pat, $single_tuple:pat) => {
        ($all_ok, $single_tuple)
    };
    /*($all_ok:pat, $left_tuple:pat, $right_tuple:pat) => {
        flatten!(PARTIAL $all_ok, ($left_tuple, $right_tuple))
    };*/
    ($all_ok:pat, $left_tuple:pat, $right_tuple:pat $(, $another_tuple:pat)*) => {
        flatten!(PARTIAL $all_ok, ($left_tuple, $right_tuple) $(, $another_tuple)*)
    };
    (PARTIAL $all_ok:pat, $cumulated:tt, $next_tuple:pat $(, $another_tuple:pat)*) => {
        flatten!(PARTIAL $all_ok, ($cumulated, $next_tuple) $(, $another_tuple)*)
    };
    (PARTIAL $all_ok:pat, $cumulated:tt) => {
        ($all_ok, $cumulated)
    }
}
/*macro_rules! expre {
    ($expre:pat) => {
        ($expre,)
    };
}
fn _f() {
    let ((a, b),) = ((0, 1),);
    let expre!((a, b)) =  ((0, 1),);
    let _a = a;

    let expre!(x) = (1,);
    let _x = x;

    let (mut c, mut d) = (0, 0);
    (d, c) = (a, b);
}*/
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
    I: Iterator<Item = &'static str> + Clone,
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

pub fn ls_l(
    path_to_ls: &'static str,
) -> RunProgress<
    &'static str,
    &'static str,
    impl Iterator<Item = &'static str> + Clone,
    impl Future<Output = IoResult<Output>>,
> {
    run("ls", ["-l", path_to_ls].into_iter())
}

pub async fn content_ls() -> StringTwinResult {
    let current = ls_l(".");
    let root = ls_l("/");
    let bin = ls("/bin");
    let sbin = ls("/sbin");
    let tmp = ls_l("/tmp");

    // This could be optimized more. On any error, we print only the leftmost failed process. Hence,
    // we could skip `.await` for the rest - but code would get complicated.
    let (current, root, bin, sbin, tmp) = (
        current.complete().await,
        root.complete().await,
        bin.complete().await,
        sbin.complete().await,
        tmp.complete().await,
    );
    all_ok_formatted_or_first_error(
        || Ok((current?, root?, bin?, sbin?, tmp?)),
        |(current, root, bin, sbin, tmp)| {
            "ls -l . :\n".to_owned()
                + &current
                + "\n\nls -l / :\n"
                + &root
                + "\n\nls /bin :\n"
                + &bin
                + "\n\nls /sbin :\n"
                + &sbin
                + "\n\nls -l /tmp :\n"
                + &tmp
        },
    )
}

pub async fn content_ls2() -> StringTwinResult {
    let current = ls(".");
    let root = ls("/");
    let bin = ls("/bin");
    let sbin = ls("/sbin");

    let (current, root, bin, sbin) = (
        current.complete_chainable().await,
        root.complete_chainable().await,
        bin.complete_chainable().await,
        sbin.complete_chainable().await,
    );
    #[allow(unused)]
    if true {
        let chained = current.and(root).and(bin).and(sbin);

        let (all_ok, (((current, root), bin), sbin)) = chained.split();
    } else {
        // (bool, String) for each pair:
        if true {
            let chained = current.pairable();
            let (all_ok, (current_ok, current)) = chained.split();
            // -------
        } else if true {
            let chained = current.pairable().pair_first(root);
            let (all_ok, (rest, (root_ok, root))) = chained.split();
        } else if true {
            let chained = current.pairable().pair_first(root);
            let (all_ok, ((current_ok, current), (root_ok, root))) = chained.split();
            // -------
        } else if true {
            let chained = current.pairable().pair_first(root).pair(bin);
            let (all_ok, ((rest, (root_ok, root)), (bin_ok, bin))) = chained.split();
        } else if true {
            let chained = current.pairable().pair_first(root).pair(bin);
            let (all_ok, (((current_ok, current), (root_ok, root)), (bin_ok, bin))) =
                chained.split();
            // -------
        } else if true {
            let chained = current.pairable().pair_first(root).pair(bin).pair(sbin);
            let (all_ok, (((rest, (root_ok, root)), (bin_ok, bin)), (sbin_ok, sbin))) =
                chained.split();
        } else if true {
            let chained = current.pairable().pair_first(root).pair(bin).pair(sbin);
            if true {
                let (
                    all_ok,
                    ((((current_ok, current), (root_ok, root)), (bin_ok, bin)), (sbin_ok, sbin)),
                ) = chained.split();
            } else {
                let flatten!(
                    all_ok,
                    (current_ok, current),
                    (root_ok, root),
                    (bin_ok, bin),
                    (sbin_ok, sbin)
                ) = chained.split();
            }
        }
    }
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
