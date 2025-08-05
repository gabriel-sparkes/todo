use chrono::{Local, NaiveDateTime, TimeZone};
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use std::{
    env::home_dir,
    fs::{self, read_to_string, write},
    io::{self, Write},
    path::Path,
    sync::Arc,
};
use tokio::{
    signal,
    sync::Mutex,
    time::{Duration, sleep},
};

const TASKS_DIR: &str = "/todo/tasks.json";

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tasks_dir: String = match home_dir() {
        Some(path) => path.display().to_string() + TASKS_DIR,
        None => TASKS_DIR.to_string(),
    };

    // set up arc-mutex to share with ctrlc exit handler
    let tasks_init = load(&tasks_dir)?;
    println!("Loaded tasks: {:?}", tasks_init);
    let tasks_arc = Arc::new(Mutex::new(tasks_init));

    let mut name = String::new();
    let mut deadline = String::new();

    // get task name
    print!("Task name: ");
    io::stdout().flush().expect("Flush failed!");
    io::stdin().read_line(&mut name).unwrap();

    // get task deadline
    print!("Deadline (dd/mm/yyyy HH:MM): ");
    io::stdout().flush().expect("Flush failed!");
    io::stdin().read_line(&mut deadline).unwrap();

    let test_task = Task {
        content: name.trim().to_string(),
        deadline: timestamp_from_date(deadline.clone()),
        priority: Priority::Medium,
    };

    println!("{:?}", test_task);
    tasks_arc.lock().await.push(test_task);

    // new block so the program doesn't hang
    // limit the scope of the first lock
    {
        let tasks_guard = tasks_arc.lock().await;
        let mut handles = Vec::new();

        for task in tasks_guard.iter() {
            let task_clone = task.clone();
            handles.push(tokio::spawn(timer(task_clone))); // assuming async fn timer(Task)
        }

        for handle in handles {
            let _ = handle.await;
        }
    }

    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    println!("Exiting");
    let _ = save(&tasks_arc, &tasks_dir).await;

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

async fn save(arc: &Arc<Mutex<Vec<Task>>>, path: &str) -> io::Result<()> {
    let guard = arc.lock().await;
    let data = &serde_json::to_string(&*guard).unwrap();
    match write(path, data) {
        Ok(_) => (),
        Err(e) => {
            println!("An error occurred: {}", e);
            if e.kind() == io::ErrorKind::NotFound {
                let create_path = Path::new(path);

                // extract parent directory and create it
                if let Some(parent) = create_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                println!("Created missing directories");
                let _ = write(path, data);
            }
        }
    }
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
async fn timer(task: Task) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let date_time = Local.timestamp_opt(task.deadline as i64, 0).unwrap();
    println!(
        "Starting countdown for task {} scheduled for {}",
        task.content, date_time
    );
    // just seeing if it works
    let _ = sleep(Duration::from_secs(5)).await;
    Notification::new()
        .summary("Task aaa")
        .body("This will almost look like a real firefox notification.")
        .icon("firefox")
        .show()?;
    println!("Done");
    Ok(())
}
