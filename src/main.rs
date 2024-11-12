use chrono::Local;
use colored::*;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use std::{fs, thread, time::Duration};
use directories::ProjectDirs;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    github_token: Option<String>,
    default_repo_owner: Option<String>,
    default_repo_name: Option<String>,
    repositories: Vec<Repository>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Repository {
    owner: String,
    name: String,
    display_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    id: usize,
    title: String,
    description: String,
    priority: Priority,
    status: Status,
    due_date: String,
    github_issue_number: Option<u64>,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
enum Priority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
enum Status {
    Todo,
    InProgress,
    NeedsHelp,
    Done,
}

struct TaskManager {
    tasks: Vec<Task>,
    save_path: std::path::PathBuf,
    github: Option<Octocrab>,
    config: Config,
    current_repo: Option<Repository>,
}

impl TaskManager {
    async fn new() -> Self {
        println!("{}", "🚀 Welcome to TaskFlow!".bold().magenta());
        println!("A friendly task manager for any GitHub project");
        
        let folders = ProjectDirs::from("com", "taskflow", "tasks")
            .expect("Could not determine config directory");
        
        let save_path = folders.config_dir().to_path_buf();
        fs::create_dir_all(&save_path).expect("Could not create config directory");
        
        let config = Self::load_or_create_config(&save_path);
        let github = Self::setup_github(&config).await;
        let tasks = Self::load_tasks(&save_path).unwrap_or_else(|_| Vec::new());
        
        let mut manager = TaskManager {
            tasks,
            save_path,
            github,
            config,
            current_repo: None,
        };

        if manager.config.repositories.is_empty() {
            manager.first_time_setup().await;
        }

        manager.select_repository().await;
        manager
    }

    async fn setup_github(config: &Config) -> Option<Octocrab> {
        if let Some(token) = &config.github_token {
            match Octocrab::builder()
                .personal_token(token.clone())
                .build() {
                    Ok(github) => {
                        println!("{}", "✅ Connected to GitHub!".green());
                        Some(github)
                    },
                    Err(_) => {
                        println!("{}", "⚠️  GitHub connection failed".yellow());
                        None
                    }
                }
        } else {
            println!("{}", "No GitHub token configured".yellow());
            None
        }
    }

    async fn first_time_setup(&mut self) {
        println!("\n{}", "👋 Looks like this is your first time here!".bold().blue());
        println!("Let's get you set up...");

        if self.config.github_token.is_none() {
            println!("\n{}", "First, you'll need a GitHub token.".bold());
            println!("1. Go to: https://github.com/settings/tokens");
            println!("2. Click 'Generate new token (classic)'");
            println!("3. Select: repo, workflow, read:org");
            println!("4. Copy the token and paste it here");

            let token = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter your GitHub token")
                .validate_with(|input: &String| -> Result<(), &str> {
                    if input.trim().is_empty() {
                        Err("Token cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact()
                .unwrap();

            self.config.github_token = Some(token);
            self.save_config().expect("Failed to save config");
            self.github = Self::setup_github(&self.config).await;
        }

        self.add_repository().await;
    }

    async fn add_repository(&mut self) {
        println!("\n{}", "Let's add a GitHub repository:".bold());
        
        let owner: String = Input::with_theme(&ColorfulTheme::default())  // Added type annotation
            .with_prompt("Repository owner (username or organization)")
            .interact()
            .unwrap();

        let name: String = Input::with_theme(&ColorfulTheme::default())  // Added type annotation
            .with_prompt("Repository name")
            .interact()
            .unwrap();

        let display_name: String = Input::with_theme(&ColorfulTheme::default())  // Added type annotation
            .with_prompt("Display name for this project")
            .default(name.clone())
            .interact()
            .unwrap();

        if let Some(github) = &self.github {
            println!("🔄 Verifying repository access...");
            match github.repos(owner.clone(), name.clone()).get().await {
                Ok(_) => {
                    println!("✅ Repository verified!");
                    let repo = Repository {
                        owner,
                        name,
                        display_name,
                    };
                    self.config.repositories.push(repo);
                    self.save_config().expect("Failed to save config");
                },
                Err(_) => {
                    println!("⚠️  Could not access repository. Please check the details and your permissions.");
                }
            }
        }
    }

    async fn select_repository(&mut self) {
        if self.config.repositories.is_empty() {
            println!("No repositories configured. Let's add one!");
            self.add_repository().await;
            return;
        }

        let repo_choices: Vec<String> = self.config.repositories
            .iter()
            .map(|r| format!("{} ({}/{})", r.display_name, r.owner, r.name))
            .collect();

        let repo_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select repository to work with")
            .items(&repo_choices)
            .default(0)
            .interact()
            .unwrap();

        self.current_repo = Some(self.config.repositories[repo_idx].clone());
        println!("\n{} {}", "🎯 Now working with:".bold(), self.current_repo.as_ref().unwrap().display_name);
    }

    async fn add_task(&mut self) {
        println!("\n{}", "✨ Add New Task".bold().green());
        
        let title = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Task title")
            .interact()
            .unwrap();

        let description = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Description (optional)")
            .allow_empty(true)
            .interact()
            .unwrap();

        let priorities = vec!["Low", "Medium", "High"];
        let priority_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Priority")
            .items(&priorities)
            .default(0)
            .interact()
            .unwrap();

        let priority = match priority_idx {
            0 => Priority::Low,
            1 => Priority::Medium,
            _ => Priority::High,
        };

        let due_date = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Due date (e.g., 'tomorrow', 'next week')")
            .interact()
            .unwrap();

        let task = Task {
            id: self.tasks.len(),
            title,
            description,
            priority,
            status: Status::Todo,
            due_date,
            github_issue_number: None,
            created_at: Local::now().format("%B %d, %Y").to_string(),
        };

        if let (Some(github), Some(repo)) = (&self.github, &self.current_repo) {
            if Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Create GitHub issue?")
                .items(&["Yes", "No"])
                .default(0)
                .interact()
                .unwrap() == 0 
            {
                match github.issues(&repo.owner, &repo.name)
                    .create(&task.title)
                    .body(&task.description)
                    .send()
                    .await 
                {
                    Ok(_) => println!("✅ GitHub issue created!"),
                    Err(_) => println!("⚠️  Couldn't create GitHub issue"),
                }
            }
        }

        self.tasks.push(task);
        self.save_tasks().expect("Failed to save tasks");
        
        let pb = ProgressBar::new(100);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("#>-"));

        for _ in 0..100 {
            pb.set_message("Saving task...");
            pb.inc(1);
            thread::sleep(Duration::from_millis(10));
        }

        println!("✅ Task added successfully!");
    }

    fn list_tasks(&self) {
        if self.tasks.is_empty() {
            println!("\n{}", "No tasks found.".yellow());
            return;
        }

        println!("\n{}", "📋 Your Tasks".bold().blue());
        println!("{}", "=".repeat(50));

        for task in &self.tasks {
            let status_icon = match task.status {
                Status::Todo => "🆕",
                Status::InProgress => "🔄",
                Status::NeedsHelp => "🆘",
                Status::Done => "✅",
            };

            let priority_icon = match task.priority {
                Priority::Low => "⭐".normal(),
                Priority::Medium => "⭐⭐".yellow(),
                Priority::High => "⭐⭐⭐".red(),
            };

            let description_text = if !task.description.is_empty() {
                task.description.clone().dimmed()
            } else {
                "".into()
            };

            println!(
                "\n{} {} {}\n{}\nDue: {}\nCreated: {}\n",
                status_icon,
                task.title.bold(),
                priority_icon,
                description_text,
                task.due_date.cyan(),
                task.created_at.dimmed()
            );
        }
    }

    fn update_task(&mut self) {
        if self.tasks.is_empty() {
            println!("\n{}", "No tasks to update.".yellow());
            return;
        }

        let task_list: Vec<String> = self.tasks
            .iter()
            .map(|t| format!("{}: {}", t.id, t.title))
            .collect();

        let task_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select task to update")
            .items(&task_list)
            .interact()
            .unwrap();

        let statuses = vec!["Todo", "In Progress", "Needs Help", "Done"];
        let status_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Update status")
            .items(&statuses)
            .interact()
            .unwrap();

        self.tasks[task_idx].status = match status_idx {
            0 => Status::Todo,
            1 => Status::InProgress,
            2 => Status::NeedsHelp,
            _ => Status::Done,
        };

        self.save_tasks().expect("Failed to save tasks");
        println!("\n{}", "✅ Task updated!".green());
    }

    fn load_or_create_config(path: &std::path::Path) -> Config {
        let config_path = path.join("config.json");
        if config_path.exists() {
            let data = fs::read_to_string(&config_path).expect("Failed to read config");
            serde_json::from_str(&data).expect("Failed to parse config")
        } else {
            let config = Config {
                github_token: None,
                default_repo_owner: None,
                default_repo_name: None,
                repositories: Vec::new(),
            };
            let data = serde_json::to_string_pretty(&config).expect("Failed to serialize config");
            fs::write(&config_path, data).expect("Failed to write config");
            config
        }
    }

    fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = self.save_path.join("config.json");
        let data = serde_json::to_string_pretty(&self.config)?;
        fs::write(config_path, data)?;
        Ok(())
    }

    fn load_tasks(path: &std::path::Path) -> Result<Vec<Task>, Box<dyn std::error::Error>> {
        let file_path = path.join("tasks.json");
        if !file_path.exists() {
            return Ok(Vec::new());
        }
        
        let data = fs::read_to_string(file_path)?;
        let tasks = serde_json::from_str(&data)?;
        Ok(tasks)
    }

    fn save_tasks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let file_path = self.save_path.join("tasks.json");
        let data = serde_json::to_string_pretty(&self.tasks)?;
        fs::write(file_path, data)?;
        Ok(())
    }

    async fn main_menu(&mut self) {
        loop {
            let choices = vec![
                "✨ Add new task",
                "📋 List tasks",
                "🔄 Update task",
                "📂 Switch repository",
                "⚙️  Add new repository",
                "👋 Exit",
            ];

            let choice = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("What would you like to do?")
                .items(&choices)
                .default(0)
                .interact()
                .unwrap();

            match choice {
                0 => self.add_task().await,
                1 => self.list_tasks(),
                2 => self.update_task(),
                3 => self.select_repository().await,
                4 => self.add_repository().await,
                _ => break,
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let mut manager = TaskManager::new().await;
    manager.main_menu().await;
    println!("\n{}", "👋 Thanks for using TaskFlow!".bold().blue());
}