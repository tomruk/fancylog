mod field;
mod reader;
mod reader_json;
mod source;

use std::{path::Path, time::Duration};

use clap::{arg, ArgAction, Command};
use futures::{
    channel::mpsc::{channel, Receiver},
    future::join_all,
    SinkExt, StreamExt,
};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use reader::Reader;
use reader_json::JsonReader;
use source::Source;
use tokio::{fs::File, io::BufReader};

#[tokio::main]
async fn main() {
    let matches = Command::new("f")
        .arg(arg!([source] "Files to operate on").action(ArgAction::Append))
        .get_matches();

    let mut futs = vec![];
    if let Some(files) = matches.get_many::<String>("source") {
        for file_path in files {
            let file_path = file_path.clone();
            println!("file: {}", file_path);
            let file = File::open(file_path.clone()).await.unwrap();

            let source = BufReader::new(file);
            let source = Source::new(source);
            let mut reader = JsonReader::new(source);

            let fut = tokio::task::spawn(async move {
                let (mut watcher, mut rx) = new_async_watcher().unwrap();

                // Add a path to be watched. All files and directories at that path and
                // below will be monitored for changes.
                let mut watch = || {
                    watcher
                        .watch(Path::new(&file_path), RecursiveMode::NonRecursive)
                        .unwrap();
                };
                watch();

                //while let Some(res) = rx.next().await {
                loop {
                    let res = rx.next().await;
                    if let Some(res) = res {
                        match res {
                            Ok(event) => {
                                println!("changed: {:?}", event);
                                loop {
                                    let fields = reader.read_fields().await;
                                    if let Some(fields) = fields {
                                        println!("{:?}", fields);
                                    } else {
                                        break;
                                    }
                                }
                                //watch();
                            }
                            Err(e) => println!("watch error: {:?}", e),
                        }
                    } else {
                        println!("res None");
                    }
                }
            });
            futs.push(fut);
        }
    }

    join_all(futs).await;
}

fn new_async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        // TODO: Is 2 seconds a good idea?
        Config::default().with_poll_interval(Duration::from_millis(2000)),
    )?;

    Ok((watcher, rx))
}
