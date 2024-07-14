mod app;
mod config;

use std::{
    env, fs,
    path::Path,
    sync::{mpsc, Arc, RwLock},
    thread,
    time::Duration,
};

use app::App;
use console::{Key, Term};
use notify::{Event as WatchEvent, EventKind as WatchEventKind, RecursiveMode, Watcher};

fn main() {
    let mut args = env::args();
    let _ = args.next();

    let language = match args.next().as_ref().map(|s| s.as_str()) {
        Some("rust") => tree_sitter_rust::language(),
        Some("tsx") | Some("typescript") => tree_sitter_typescript::language_tsx(),
        Some("javascript") => tree_sitter_javascript::language(),
        Some("python") => tree_sitter_python::language(),
        Some("ruby") => tree_sitter_ruby::language(),
        Some("markdown") => tree_sitter_md::language(),
        Some(s) => panic!("invalid language passed: {s}"),
        None => panic!("no language passed"),
    };
    let path = args.next().expect("no arg passed");
    let query_path = args.next();
    let src = fs::read_to_string(&path).expect("unable to read file");

    let app = Arc::new(RwLock::new(App::new(
        src.as_bytes(),
        &path,
        query_path.as_ref(),
        language,
    )));

    let watch_fn = |watcher_app: Arc<RwLock<App>>| {
        move |ev| {
            if let Ok(WatchEvent {
                kind: WatchEventKind::Modify(..),
                ..
            }) = ev
            {
                if let Ok(mut locked) = watcher_app.try_write() {
                    locked.reload();
                    locked.draw();
                };
            }
        }
    };

    let mut watcher1 = notify::recommended_watcher(watch_fn(Arc::clone(&app))).unwrap();
    watcher1
        .watch(Path::new(&path), RecursiveMode::NonRecursive)
        .unwrap();

    let mut watcher2 = notify::recommended_watcher(watch_fn(Arc::clone(&app))).unwrap();
    if let Some(query_path) = query_path {
        watcher2
            .watch(Path::new(&query_path), RecursiveMode::NonRecursive)
            .unwrap();
    }

    let (tx, rx) = mpsc::channel();
    let tx0 = tx.clone();
    thread::spawn(move || {
        let term = Term::stdout();
        loop {
            if let Ok(Key::Char(ev)) = term.read_key() {
                tx0.send(ev).unwrap();
            }
        }
    });

    if let Ok(locked) = app.try_read() {
        locked.draw();
    }

    loop {
        match rx.try_recv() {
            Ok(ev) => {
                if let Ok(mut locked) = app.try_write() {
                    match ev {
                        '>' => locked.increase_indent(),
                        '<' => locked.decrease_indent(),
                        'n' => locked.toggle_ranges(),
                        's' => locked.toggle_source(),
                        'r' => locked.reload(),
                        _ => (),
                    }
                    locked.draw();
                }
            }
            _ => (),
        }
        thread::sleep(Duration::from_millis(10));
    }
}
