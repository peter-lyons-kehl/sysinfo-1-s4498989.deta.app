use core::future::Future;
use std::fmt::Display;
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

pub fn assert_linux() {
    assert!(cfg!(target_os = "linux"), "For Linux only.");
}

/// Create a new [Command] based on `program`. Set it to `kill_on_drop`.
pub fn command(program: &'static str) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    command
}

pub fn ascii_bytes_to_string(bytes: Vec<u8>) -> String {
    let mut result = String::with_capacity(bytes.len());
    for byte in bytes {
        result.push(char::from(byte));
    }
    result
}

/// Start the program, with any arguments or other adjustments done in `modify` closure. Kill on drop.
///
/// On success, return the program's output, treated as ASCII.
pub async fn run<F: Fn(&mut Command)>(program: &'static str, modify: F) -> RunResult {
    let mut command = command(program);
    modify(&mut command);
    let out = command.output().await.map_err(|error| RunError {
        program: program.to_owned(),
        error,
    })?;
    Ok(ascii_bytes_to_string(out.stdout))
}

pub fn where_is(program: &'static str) -> impl Future<Output = RunResult> {
    // - whereis, /bin/whereis, /usr/bin/whereis fail on Deta.Space
    // - which, /bin/which, /usr/bin/which fail, too:
    //
    // "No such file or directory (os error 2)."
    run("/usr/bin/which", move |prog| {
        prog.arg(program);
    })
}

/// Used to locate binaries. Why? See comments inside [content].
pub async fn content_locate_binaries() -> String {
    let free = where_is("free");
    let df = where_is("df");
    let mount = where_is("mount");
    let (free, df, mount) = (free.await, df.await, mount.await);
    handle_errors(
        || Ok((free?, df?, mount?)),
        |(free, df, mount)| "".to_owned() + &free + "\n" + &df + "\n" + &mount,
    )

    //Ok("".to_owned() + &free + "\n" + &df + "\n" + &mount);
    //"".to_owned()
}

/// Handle errors, especially when you have multiple [RunResult] instances, when it's ergonomic to
/// use `?` short circuit operator. We can't use `?` in an (regardless of whether `async` or
/// ordinary) function returning [String].
///
/// The first parameter (closure `source`) has to be [FnOnce], and not [Fn], because [RunError] and hence [Result<T, RunError>] is not [Copy].
pub fn handle_errors<T, S: FnOnce() -> Result<T, RunError>, F: Fn(T) -> String>(
    source: S,
    format: F,
) -> String {
    match source() {
        Ok(content) => format(content),
        Err(err) => format!("{err}"),
    }
}

/* // Can't type these:
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
