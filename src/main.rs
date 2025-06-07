use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use git2::Repository;
use iced::{
    Element, Size, Task,
    widget::{ComboBox, column, combo_box, row, text},
};
use log::error;

#[derive(Parser)]
struct Args {
    reference: String,
}

#[derive(Debug, Clone)]
struct Project(PathBuf);

type Room = String;

struct State {
    args: Args,
    repo: git2::Repository,
    project_list: combo_box::State<Project>,
    project: Project,
    room_list: combo_box::State<String>,
    room: String,
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

#[derive(Debug, Clone)]
enum Message {
    SelectProject(Project),
    SelectRoom(Room),
}

fn get_initial_state() -> Result<State> {
    let args = Args::parse();

    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(_) => {
            bail!("Failed to open git repository");
        }
    };

    let mut projects: Vec<Project> = vec![];
    for path in glob::glob("./**/project.xml")? {
        let path = path?.parent().unwrap().to_path_buf();
        projects.push(Project(path));
    }
    if projects.len() == 0 {
        bail!("No SMART projects found");
    }
    projects.sort();

    // let reference = repo.find_reference(&args.reference)?;
    // let tree = reference.peel_to_tree()?;
    // let projects_obj = tree.get_path(Path::new("Projects"))?.to_object(&repo)?;
    // let projects_tree = projects_obj
    //     .as_tree()
    //     .context("Expecting 'Projects' to be a directory")?;
    // let mut projects: Vec<String> = vec![];
    // for p in projects_tree {
    //     if let Some(name) = p.name() {
    //         projects.push(name.to_string());
    //     }
    // }

    let mut state = State {
        args,
        repo,
        project: projects[0].clone(),
        project_list: combo_box::State::new(projects),
        room_list: combo_box::State::new(vec![]),
        room: String::new(),
    };
    refresh_room_list(&mut state)?;
    Ok(state)
}

fn refresh_room_list(state: &mut State) -> Result<()> {
    let mut room_list: Vec<String> = vec![];
    for room in glob::glob(&format!("{}/Export/Rooms/*.xml", state.project))? {
        let room = room?;
        room_list.push(
            room.file_stem()
                .context("file_stem")?
                .to_string_lossy()
                .to_string(),
        )
    }
    room_list.sort();
    if room_list.len() == 0 {
        bail!("No rooms found in project {}", state.project);
    }
    if !room_list.contains(&state.room) {
        state.room = room_list[0].clone();
    }
    state.room_list = combo_box::State::new(room_list);
    Ok(())
}

fn try_update(state: &mut State, message: Message) -> Result<Task<Message>> {
    match message {
        Message::SelectProject(project) => {
            state.project = project;
            refresh_room_list(state)?;
        }
        Message::SelectRoom(room) => {
            state.room = room;
        }
    }
    Ok(Task::none())
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match try_update(state, message) {
        Ok(t) => t,
        Err(e) => {
            error!("Error: {}\n{}", e, e.backtrace());
            Task::none()
        }
    }
}

fn view(state: &State) -> Element<Message> {
    column![
        row![
            combo_box(
                &state.project_list,
                "Select a project",
                Some(&state.project),
                Message::SelectProject,
            ),
            combo_box(
                &state.room_list,
                "Select a room",
                Some(&state.room),
                Message::SelectRoom,
            ),
        ]
        .spacing(10)
        .padding(10)
    ]
    .into()
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let state = get_initial_state()?;

    iced::application("SMART diff", update, view)
        .window_size(Size {
            width: 1440.0,
            height: 960.0,
        })
        .run_with(|| (state, Task::none()))?;

    // let b = repo.find_branch(&args.reference, git2::BranchType::Local)?;
    // println!("branch: {:?}", b.name());
    Ok(())
}
