use chrono::{Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::{
    env::home_dir,
    fs::{self, write},
    io::{self, Write},
    path::Path,
    process::exit,
    sync::{Arc, Mutex},
};

const TASKS_DIR: &str = "/todo/tasks.json";

#[derive(Debug, Serialize, Deserialize)]
enum Priority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    content: String,
    deadline: u64,
    priority: Priority,
}

fn main() {
    let tasks_dir: String = match home_dir() {
        Some(path) => path.display().to_string() + TASKS_DIR,
        None => TASKS_DIR.to_string(),
    };
    let tasks_dir_clone = tasks_dir.clone();

    // set up arc-mutex to share with ctrlc exit handler
    let tasks = Arc::new(Mutex::new(Vec::<Task>::new()));
    let tasks_clone = Arc::clone(&tasks);

    let mut name = String::new();
    let mut deadline = String::new();

    // set up ctrlc handler
    ctrlc::set_handler(move || {
        let _ = save(&tasks_clone, &tasks_dir_clone);
        exit(0);
    })
    .expect("Failed to set Ctrl+C handler");

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
    tasks.lock().unwrap().push(test_task);
    let _ = save(&tasks, &tasks_dir);
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

fn save(arc: &Arc<Mutex<Vec<Task>>>, path: &str) -> std::io::Result<()> {
    let guard = arc.lock().unwrap();
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
