use chrono::{Local, NaiveDateTime, TimeZone};
use cursive::{views::TextView, Cursive, CursiveExt};
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use cursive::views::{ScrollView, LinearLayout};
use std::{
    env::home_dir,
    fs::{self, File, read_to_string, write},
    io::{self, Write},
    path::Path,
    sync::Arc,
};
use tokio::{
    signal,
    sync::Mutex,
    time::{Duration, sleep},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
enum Priority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    content: String,
    deadline: u64,
    priority: Priority,
    completed: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut siv = Cursive::new();

    siv.add_layer(TextView::new("Hello World!\nPress q to quit."));
    siv.add_global_callback('q', |s| s.quit());

    let working_dir = match home_dir() {
        Some(path) => path.display().to_string() + "/todo/",
        None => "/todo/".to_string(),
    };
    let tasks_location = working_dir.clone() + "tasks.json";
    let icon_location = working_dir + "icon.png";
    if !Path::new(&icon_location).exists() {
        println!("Icon not set");
    }

    // set up arc-mutex to share with ctrlc exit handler
    let tasks_init = load(&tasks_location)?;
    println!("Loaded tasks: {:?}", tasks_init);
    let tasks_arc = Arc::new(Mutex::new(tasks_init));

    /* let task = Task {
        content: name.trim().to_string(),
        deadline: timestamp_from_date(deadline.clone()),
        priority: Priority::Medium,
        completed: false,
    };

    println!("{:?}", task);
    tasks_arc.lock().await.push(task); */

    let tasks_guard = tasks_arc.lock().await;
    let mut layout = LinearLayout::vertical();
    for task in tasks_guard.iter() {
        layout.add_child(TextView::new(task.content.clone()));
    }
    let scrollable = ScrollView::new(layout);
    siv.add_layer(scrollable);
    drop(tasks_guard);

    // new block so the program doesn't hang
    // limit the scope of the first lock
    {
        let tasks_guard = tasks_arc.lock().await;
        let mut handles = Vec::new();

        for task in tasks_guard.iter() {
            let task_clone = task.clone();
            handles.push(tokio::spawn(timer(task_clone, icon_location.clone())));
        }

        for handle in handles {
            let _ = handle.await;
        }
    }

    siv.run();

    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    println!("Exiting");
    let _ = save(&tasks_arc, &tasks_location).await;

    Ok(())
}

// get unix timestamp of the given date in the local time zone
// this doesn't take potential time zone shifts/daylight savings into account for long-term tasks but whatever
fn timestamp_from_date(deadline: String) -> u64 {
    let now_timestamp = Local::now().timestamp();

    // try parsing as full datetime: "dd/mm/yyyy HH:MM"
    let parsed = NaiveDateTime::parse_from_str(deadline.trim(), "%d/%m/%Y %H:%M").or_else(|_| {
        // fallback: append "00:00" and try again
        println!("Warning: No time provided or format was wrong. Defaulting to 00:00.");
        let fallback = deadline.trim().to_owned() + " 00:00";
        NaiveDateTime::parse_from_str(&fallback, "%d/%m/%Y %H:%M")
    });

    let deadline_timestamp = match parsed {
        Ok(date) => {
            let datetime_local = Local
                .from_local_datetime(&date)
                .single()
                .expect("Ambiguous or non-existent local time");

            let timestamp = datetime_local.timestamp();
            if timestamp < now_timestamp {
                panic!("Date must be in the future");
            } else if timestamp > now_timestamp + 3153600000 {
                // panic if date provided is more than 100 years in the future
                // mostly because unix time overflows after a while and 100 years is more than enough
                // if someone finally figures out this immortality thing please tell me
                panic!(
                    "Are you sure you're going to be around that long?\nPlease enter a date within 100 years from now (that's generous enough, right?)"
                );
            }
            timestamp
        }
        Err(e) => panic!(
            "Failed to parse date! Expected format: dd/mm/yyyy or dd/mm/yyyy HH:MM\nError: {}",
            e
        ),
    };
    deadline_timestamp as u64
}

fn file_exists(path: &str, create: bool) -> Result<bool, io::Error> {
    let create_path = Path::new(path);
    if !create_path.exists() {
        if create {
            if let Some(parent) = create_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let _ = File::create(path)?;
            return Ok(true);
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn save(arc: &Arc<Mutex<Vec<Task>>>, path: &str) -> io::Result<()> {
    let guard = arc.lock().await;
    let data = &serde_json::to_string(&*guard).unwrap();
    let _ = file_exists(path, true)?;
    let _ = write(path, data)?;
    println!("Saved tasks");
    Ok(())
}

fn load(path: &str) -> io::Result<Vec<Task>> {
    let data = match read_to_string(path) {
        Ok(data) => data,
        Err(e) => {
            println!("An error occurred: {}", e);
            if e.kind() == io::ErrorKind::NotFound {
                let create_path = Path::new(path);

                // extract parent directory and create it
                if let Some(parent) = create_path.parent() {
                    fs::create_dir_all(parent)?;
                    println!("Created missing directories");
                }
                let _ = write(path, "");
            }
            "[]".to_string()
        }
    };
    let deserialised = serde_json::from_str::<Vec<Task>>(&data)?;
    Ok(deserialised)
}

// I have no idea what this box thing is yet
// or + Send + Sync
async fn timer(
    task: Task,
    icon_location: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let date_time = Local.timestamp_opt(task.deadline as i64, 0).unwrap();
    println!(
        "Starting countdown for task {} scheduled for {}",
        task.content, date_time
    );
    // just seeing if it works
    let _ = sleep(Duration::from_secs(5)).await;
    Notification::new()
        .summary(&task.content)
        .body("Time's up")
        .icon(&icon_location)
        .show()?;
    println!("Done");
    Ok(())
}
