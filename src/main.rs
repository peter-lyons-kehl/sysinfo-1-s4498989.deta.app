use axum::{routing::get, Router};
//use std::process::{ExitStatus, Output};
use std::env;

mod util;

/// Content returned over HTTP.
async fn content() -> String {
    // Beware: Some Unix distributions (at least Manjaro, possibly Arch, too) have aliases set (for
    // example in ~/.bashrc). Those prettify the output, but are not available under non-personal
    // accounts, such as daemons/web services! Hence we use full paths to executables. (That may
    // make this not portable to other OS'es, but that doesn't matter.)
    //
    // To complicate, Manjaro has free & df under both /usr/bin & /bin. But: Deta.Space does NOT
    // have /usr/bin/df - only /bin/df.
    //
    // If your Linux or Mac OS doesn't support the following locations, and you can figure out how
    // to determine it, feel free to file a pull request.
    let free = util::run("/usr/bin/free", |prog| {
        prog.arg("-m");
    });
    let tmpfs = util::run("/bin/df", |prog| {
        prog.arg("-m").arg("/tmp");
    });
    let (free, tmpfs) = (free.await, tmpfs.await);
    util::handle_errors(
        || Ok((free?, tmpfs?)),
        |(free, tmpfs)| {
            "Sysinfo of (free tier) Deta.Space. Thank you Team Deta.Space & Love you.\n".to_owned()
                + "Format and URL routing/handling are subject to change!\n"
                + "(https://github.com/peter-kehl/sysinfo-1-s4498989.deta.app)\n\n"
                + "free -m:\n"
                + &free
                + "\n-----\n\n"
                + "df -m /tmp:\n"
                + &tmpfs
        },
    )
}

// @TODO: Consider using `#[axum::debug_handler]` if I want to call `router.route("/", get(XYZ));`
// on function XYZ that returns Result or other specialized type.
#[tokio::main]
async fn main() {
    util::assert_linux();

    // Get the port to listen on from the environment, or default to 8080 if not present.
    let addr = format!(
        "127.0.0.1:{}",
        env::var("PORT").unwrap_or("8080".to_string())
    );
    println!("Listening on http://{}", addr);

    let router = Router::new();
    let router = router.route("/locate_binaries", get(util::content_locate_binaries));
    let router = router.route("/", get(content));

    // Run it with hyper on localhost.
    axum::Server::bind(&addr.parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}
