mod room;
mod smart_xml;

use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use git2::Repository;
use iced::{
    Element, Length, Size, Task,
    widget::{self, ComboBox, column, combo_box, image, row, text},
};
use log::{error, info};

use crate::room::{LocalFileSystem, render_room};

#[derive(Parser)]
struct Args {
    reference: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq)]
struct Project(PathBuf);
type Room = String;

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq)]
struct RoomState(usize, String);

impl Display for RoomState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.0, self.1)
    }
}

struct State {
    args: Args,
    repo: git2::Repository,
    project_list: combo_box::State<Project>,
    project: Project,
    room_list: combo_box::State<String>,
    room: String,
    room_state_list: combo_box::State<RoomState>,
    room_state: RoomState,
    working_images: Option<RoomData>,
    other_images: Option<RoomData>,
}

struct RoomData {
    layer1: Vec<image::Handle>,
    layer2: Vec<image::Handle>,
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
        room_state_list: combo_box::State::new(vec![]),
        room_state: RoomState(0, String::new()),
        working_images: None,
        other_images: None,
    };
    refresh_room_list(&mut state)?;
    refresh_room_images(&mut state)?;

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

fn convert_images(images: Vec<room::Image>) -> Vec<image::Handle> {
    images
        .into_iter()
        .map(|x| image::Handle::from_rgba(x.width as u32, x.height as u32, x.pixels))
        .collect()
}

fn refresh_room_images(state: &mut State) -> Result<()> {
    let working_fs = LocalFileSystem {};
    let working_images = render_room(&state.project.0, &state.room, &working_fs)?;
    let room_states: Vec<RoomState> = working_images
        .room_state_names
        .into_iter()
        .enumerate()
        .map(|(i, x)| RoomState(i, x))
        .collect();
    if room_states.len() == 0 {
        bail!("Empty list of room states");
    }
    state.room_state = room_states[0].clone();
    state.room_state_list = combo_box::State::new(room_states);
    state.working_images = Some(RoomData {
        layer1: convert_images(working_images.layer1),
        layer2: convert_images(working_images.layer2),
    });
    info!("refreshed room images");
    Ok(())
}

fn try_update(state: &mut State, message: Message) -> Result<Task<Message>> {
    match message {
        Message::SelectProject(project) => {
            state.project = project;
            refresh_room_list(state)?;
            refresh_room_images(state)?;
        }
        Message::SelectRoom(room) => {
            state.room = room;
            refresh_room_images(state)?;
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
    let mut column = widget::Column::new();
    column = column.push(
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
        .spacing(10),
    );
    if let Some(images) = &state.working_images {
        column = column.push(
            image::viewer(images.layer1[0].clone())
                .content_fit(iced::ContentFit::Contain)
                .filter_method(image::FilterMethod::Nearest)
                .min_scale(1.0)
                .width(Length::Fill)
                .height(Length::Fill),
        );
    }
    column.spacing(10).padding(10).into()
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
