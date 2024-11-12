use chrono::Local;
use colored::*;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use indicatif::{ProgressBar, ProgressStyle};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use std::{fs, thread, time::Duration};
use directories::ProjectDirs;
use webbrowser;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    github_token: Option<String>,
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

struct TaskManager {
    tasks: Vec<Task>,
    save_path: std::path::PathBuf,
    github: Option<Octocrab>,
    config: Config,
    current_repo: Option<Repository>,
}

impl TaskManager {

    pub async fn new() -> Self {
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

    fn load_or_create_config(path: &std::path::Path) -> Config {
        let config_path = path.join("config.json");
        if config_path.exists() {
            let data = fs::read_to_string(&config_path).expect("Failed to read config");
            serde_json::from_str(&data).expect("Failed to parse config")
        } else {
            let config = Config {
                github_token: None,
                repositories: Vec::new(),
            };
            let data = serde_json::to_string_pretty(&config).expect("Failed to serialize config");
            fs::write(&config_path, data).expect("Failed to write config");
            config
        }
    }

    async fn first_time_setup(&mut self) {
        println!("\n{}", "👋 First time setup!".bold().blue());
        println!("Let's get your GitHub token...");

        let token = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter your GitHub token")
            .interact()
            .unwrap();

        self.config.github_token = Some(token);
        self.save_config().expect("Failed to save config");
        self.github = Self::setup_github(&self.config).await;

        self.add_repository().await;
    }

    async fn add_repository(&mut self) {
        println!("\n{}", "Let's add a GitHub repository:".bold());
        
    let owner: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Repository owner (username or organization)")
        .interact()
        .unwrap();
    
    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Repository name")
        .interact()
        .unwrap();
    
    let display_name: String = Input::with_theme(&ColorfulTheme::default())
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
        println!("\n{} {}", "🎯 Now working with:".bold(), 
            self.current_repo.as_ref().unwrap().display_name);
    }

    async fn add_task(&mut self) {
        println!("\n{}", "✨ Add New Task".bold().green());
        
        let title: String = Input::with_theme(&ColorfulTheme::default())
             .with_prompt("Task title")
             .interact()
             .unwrap();

        let description: String = Input::with_theme(&ColorfulTheme::default())
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
            title: title.clone(),
            description: description.clone(),
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
                    .create(&title)
                    .body(&description)
                    .send()
                    .await 
                {
                    Ok(issue) => {
                        println!("✅ GitHub issue created!");
                        println!("View it at: https://github.com/{}/{}/issues/{}", 
                            repo.owner, repo.name, issue.number);
                    },
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
                "🎨 Visualize Project",
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
                3 => self.visualize_project().await,
                4 => self.select_repository().await,
                5 => self.add_repository().await,
                _ => break,
            }
        }
    }

    async fn visualize_project(&self) {
        let choices = vec![
            "🎨 View Kanban Board",
            "🌐 Open in GitHub",
            "🔄 Create/Update Project Board",
            "🔙 Back"
        ];

        let choice = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("How would you like to view the project?")
            .items(&choices)
            .default(0)
            .interact()
            .unwrap();

        match choice {
            0 => self.show_kanban_board(),
            1 => self.open_project_in_browser().await,
            2 => self.create_github_project().await,
            _ => return,
        }
    }

    fn show_kanban_board(&self) {
        println!("\n{}", "🎨 Project Kanban Board".bold().magenta());
        println!("{}", "=".repeat(80));

        let columns = [
            ("📋 TO DO", Status::Todo),
            ("🔄 IN PROGRESS", Status::InProgress),
            ("🆘 NEEDS HELP", Status::NeedsHelp),
            ("✅ DONE", Status::Done),
        ];

        let width = 20;
        let separator = "│";

        // Print header
        for (title, _) in &columns {
            print!("{:^width$}{}", title.bold(), separator, width = width);
        }
        println!("\n{}", "─".repeat(80));

        // Get max tasks in any column
        let max_tasks = columns
            .iter()
            .map(|(_, status)| {
                self.tasks
                    .iter()
                    .filter(|t| t.status == *status)
                    .count()
            })
            .max()
            .unwrap_or(0);

        // Print tasks in columns
        for i in 0..max_tasks {
            for (_, status) in &columns {
                let task = self.tasks
                    .iter()
                    .filter(|t| t.status == *status)
                    .nth(i);

                if let Some(task) = task {
                    let priority_color = match task.priority {
                        Priority::High => task.title.red(),
                        Priority::Medium => task.title.yellow(),
                        Priority::Low => task.title.green(),
                    };
                    print!("{:width$}{}", priority_color, separator, width = width);
                } else {
                    print!("{:width$}{}", "", separator, width = width);
                }
            }
            println!();
        }
    }

    async fn open_project_in_browser(&self) {
        if let Some(repo) = &self.current_repo {
            let project_url = format!(
                "https://github.com/{}/{}/projects",
                repo.owner,
                repo.name
            );
            
            println!("🌐 Opening project in browser...");
            if webbrowser::open(&project_url).is_ok() {
                println!("✅ Browser opened successfully!");
            } else {
                println!("⚠️  Couldn't open browser. Visit: {}", project_url);
            }
        }
    }

    async fn create_github_project(&self) {
        if let (Some(github), Some(repo)) = (&self.github, &self.current_repo) {
            println!("\n{}", "✨ Creating new GitHub Project".bold().green());
            
            let name = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt("Project name")
                .default("TaskFlow Board".to_string())
                .interact_text()
                .unwrap();
    
            let description = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt("Project description")
                .default("Task management board".to_string())
                .interact_text()
                .unwrap();
    
            // Create an issue to track project setup
            let setup_issue = github.issues(&repo.owner, &repo.name)
                .create(&format!("Setup: {}", name))
                .body(&format!("Project Board Setup\n\n{}", description))
                .send()
                .await;
    
            match setup_issue {
                Ok(_) => {
                    println!("✅ Project tracking issue created!");
                    println!("\nProject Structure:");
                    println!("├── 📋 To Do");
                    println!("├── 🔄 In Progress");
                    println!("├── 🆘 Needs Help");
                    println!("└── ✅ Done");
                    
                    println!("\n💡 Tip: View and manage your project at:");
                    println!("https://github.com/{}/{}/projects", repo.owner, repo.name);
                },
                Err(_) => println!("⚠️  Couldn't create project setup"),
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