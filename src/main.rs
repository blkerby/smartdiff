mod file_system;
mod room;
mod smart_xml;

use std::{fmt::Display, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use git2::Repository;
use hashbrown::HashMap;
use iced::{
    Element, Font, Length, Point, Rectangle, Size, Subscription, Task, Theme, keyboard,
    widget::{
        Scrollable, canvas, checkbox, column, combo_box, image, pick_list, row,
        scrollable::{self, Scrollbar},
        slider, text,
    },
};
use iced_aw::SelectionList;
use log::{error, info};

use crate::room::render_room;
use crate::{
    file_system::{GitTreeFileSystem, LocalFileSystem},
    room::RoomImages,
};

pub const MIN_PIXEL_SIZE: f32 = 1.0;
pub const MAX_PIXEL_SIZE: f32 = 8.0;

#[derive(Parser)]
struct Args {
    reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
struct Project(PathBuf);
type Room = String;

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
struct RoomState(usize, String);

impl Display for RoomState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.0, self.1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceSelection {
    WorkingCopy,
    GitReference(String),
    Difference,
}

impl Display for SourceSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceSelection::WorkingCopy => write!(f, "Working copy"),
            SourceSelection::GitReference(s) => write!(f, "{}", s),
            SourceSelection::Difference => write!(f, "Difference"),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct ModifiedRoom {
    project: Project,
    room_name: String,
}

impl Display for ModifiedRoom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let project_short_name = self.project.0.components().last().unwrap().as_os_str();
        write!(
            f,
            "{}/{}",
            project_short_name.to_str().unwrap(),
            self.room_name,
        )
    }
}

struct State {
    repo: git2::Repository,
    git_reference: String,
    project_list: combo_box::State<Project>,
    project: Project,
    room_list: combo_box::State<String>,
    room: String,
    room_state_list: combo_box::State<RoomState>,
    room_state: RoomState,
    modified_room_list: Vec<ModifiedRoom>,
    modified_room_idx: Option<usize>,
    show_layer_1: bool,
    show_layer_2: bool,
    highlight_transparency: bool,
    difference_baseline: f32,
    pixel_size: f32,
    source_selection: SourceSelection,
    working_images: Option<RoomImages>,
    other_images: Option<RoomImages>,
    working_image_handles: Option<RoomData>,
    other_image_handles: Option<RoomData>,
    diff_image_handles: Option<RoomData>,
}

#[derive(Clone)]
struct RoomData {
    width: usize,
    height: usize,
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
    Event(iced::Event),
    SelectProject(Project),
    SelectRoom(Room),
    SelectRoomState(RoomState),
    SelectSource(SourceSelection),
    ShowLayer1(bool),
    ShowLayer2(bool),
    HighlightTransparency(bool),
    AdjustDifferenceBaseline(f32),
    SelectModifiedRoom(usize),
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

    let git_reference = match args.reference {
        Some(r) => r,
        None => {
            info!("Git reference not supplied, defaulting to HEAD.");
            "HEAD".to_string()
        }
    };

    let mut state = State {
        repo,
        git_reference,
        project: projects[0].clone(),
        project_list: combo_box::State::new(projects),
        room_list: combo_box::State::new(vec![]),
        room: String::new(),
        room_state_list: combo_box::State::new(vec![]),
        room_state: RoomState(0, String::new()),
        modified_room_list: vec![],
        modified_room_idx: None,
        show_layer_1: true,
        show_layer_2: true,
        highlight_transparency: false,
        difference_baseline: 0.3,
        source_selection: SourceSelection::WorkingCopy,
        pixel_size: 1.0,
        working_images: None,
        other_images: None,
        working_image_handles: None,
        other_image_handles: None,
        diff_image_handles: None,
    };
    refresh_modified_room_list(&mut state)?;
    refresh_room_list(&mut state)?;
    refresh_room_images(&mut state)?;

    Ok(state)
}

fn refresh_modified_room_list(state: &mut State) -> Result<()> {
    // List modified rooms across all projects
    let mut room_map: HashMap<PathBuf, ModifiedRoom> = HashMap::new();
    for project in state.project_list.options() {
        for room in glob::glob(&format!("{}/Export/Rooms/*.xml", project))? {
            let room = room?;
            room_map.insert(
                room.clone(),
                ModifiedRoom {
                    project: project.clone(),
                    room_name: room.file_stem().unwrap().to_str().unwrap().to_string(),
                },
            );
        }
    }

    let reference = state.repo.revparse_single(&state.git_reference)?;
    let tree = reference.peel_to_tree()?;
    let diff = state
        .repo
        .diff_tree_to_workdir_with_index(Some(&tree), None)?;
    let mut modified_room_list: Vec<ModifiedRoom> = vec![];
    for d in diff.deltas() {
        if let Some(path) = d.new_file().path() {
            if room_map.contains_key(path) {
                modified_room_list.push(room_map[path].clone());
            }
        }
    }
    modified_room_list.sort();
    state.modified_room_list = modified_room_list;
    Ok(())
}

fn refresh_room_list(state: &mut State) -> Result<()> {
    // List rooms in current project:
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

fn diff_image(img1: &room::Image, img2: &room::Image, baseline: f32) -> room::Image {
    let mut img = room::Image::new(img1.width, img1.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let p1 = img1.get_pixel(x, y);
            let p2 = img2.get_pixel(x, y);
            if p1 != p2 {
                img.set_pixel(x, y, [255, 255, 255]);
            } else if !img1.get_transparent(x, y) {
                img.set_pixel(
                    x,
                    y,
                    [
                        (p1[0] as f32 * baseline) as u8,
                        (p1[1] as f32 * baseline) as u8,
                        (p1[2] as f32 * baseline) as u8,
                    ],
                );
            }
        }
    }
    img
}

fn diff_image_list(img1: &[room::Image], img2: &[room::Image], baseline: f32) -> Vec<room::Image> {
    img1.iter()
        .zip(img2.iter())
        .map(|(x, y)| diff_image(x, y, baseline))
        .collect()
}

fn refresh_diff_images(state: &mut State) -> Result<()> {
    let Some(working_images) = state.working_images.as_ref() else {
        return Ok(());
    };
    let Some(other_images) = state.other_images.as_ref() else {
        return Ok(());
    };

    state.diff_image_handles = Some(RoomData {
        width: working_images.layer1[0].width,
        height: working_images.layer1[0].height,
        layer1: convert_images(diff_image_list(
            &working_images.layer1,
            &other_images.layer1,
            state.difference_baseline,
        )),
        layer2: convert_images(diff_image_list(
            &working_images.layer2,
            &other_images.layer2,
            state.difference_baseline,
        )),
    });
    Ok(())
}

fn refresh_room_images(state: &mut State) -> Result<()> {
    let working_fs = LocalFileSystem {};
    let working_images = render_room(&state.project.0, &state.room, &working_fs)?;
    let room_states: Vec<RoomState> = working_images
        .room_state_names
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, x)| RoomState(i, x))
        .collect();
    if room_states.len() == 0 {
        bail!("Empty list of room states");
    }
    state.room_state = room_states[0].clone();
    state.room_state_list = combo_box::State::new(room_states);
    let width = working_images.layer1[0].width;
    let height = working_images.layer1[0].height;

    let reference = state.repo.revparse_single(&state.git_reference)?;
    let tree = reference.peel_to_tree()?;
    let other_fs = GitTreeFileSystem {
        repo: &state.repo,
        tree,
    };
    let other_images = render_room(&state.project.0, &state.room, &other_fs)?;

    state.working_images = Some(working_images.clone());
    state.other_images = Some(other_images.clone());
    state.working_image_handles = Some(RoomData {
        width,
        height,
        layer1: convert_images(working_images.layer1),
        layer2: convert_images(working_images.layer2),
    });
    state.other_image_handles = Some(RoomData {
        width,
        height,
        layer1: convert_images(other_images.layer1),
        layer2: convert_images(other_images.layer2),
    });
    drop(reference);
    drop(other_fs);
    refresh_diff_images(state)?;
    Ok(())
}

fn try_update(state: &mut State, message: Message) -> Result<Task<Message>> {
    match message {
        Message::Event(e) => match e {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                modified_key: keyboard::Key::Character(c),
                ..
            }) => match c.as_str() {
                "1" => {
                    state.show_layer_1 = !state.show_layer_1;
                }
                "2" => {
                    state.show_layer_2 = !state.show_layer_2;
                }
                "w" => {
                    state.source_selection = SourceSelection::WorkingCopy;
                }
                "r" => {
                    state.source_selection =
                        SourceSelection::GitReference(state.git_reference.clone());
                }
                "d" => {
                    state.source_selection = SourceSelection::Difference;
                }
                "t" => {
                    state.highlight_transparency = !state.highlight_transparency;
                }
                "-" => {
                    state.pixel_size = (state.pixel_size - 1.0).max(MIN_PIXEL_SIZE);
                }
                "=" => {
                    state.pixel_size = (state.pixel_size + 1.0).min(MAX_PIXEL_SIZE);
                }
                _ => {}
            },
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::ArrowDown),
                ..
            }) => {
                let new_idx = match state.modified_room_idx {
                    Some(idx) => idx + 1,
                    None => 0,
                };
                if new_idx < state.modified_room_list.len() {
                    state.modified_room_idx = Some(new_idx);
                    return Ok(Task::done(Message::SelectModifiedRoom(new_idx)));
                }
            }
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::ArrowUp),
                ..
            }) => {
                let new_idx = match state.modified_room_idx {
                    Some(idx) if idx > 0 => idx - 1,
                    _ => 0,
                };
                if new_idx < state.modified_room_list.len() {
                    state.modified_room_idx = Some(new_idx);
                    return Ok(Task::done(Message::SelectModifiedRoom(new_idx)));
                }
            }
            _ => {}
        },
        Message::SelectProject(project) => {
            state.project = project;
            refresh_room_list(state)?;
            refresh_room_images(state)?;
        }
        Message::SelectRoom(room) => {
            state.room = room;
            refresh_room_images(state)?;
        }
        Message::SelectRoomState(room_state) => {
            state.room_state = room_state;
        }
        Message::SelectSource(src) => {
            state.source_selection = src;
        }
        Message::ShowLayer1(b) => {
            state.show_layer_1 = b;
        }
        Message::ShowLayer2(b) => {
            state.show_layer_2 = b;
        }
        Message::HighlightTransparency(b) => {
            state.highlight_transparency = b;
        }
        Message::AdjustDifferenceBaseline(f) => {
            state.difference_baseline = f;
            refresh_diff_images(state)?;
        }
        Message::SelectModifiedRoom(idx) => {
            state.modified_room_idx = Some(idx);
            let modified_room = &state.modified_room_list[idx];
            let project_changed = state.project != modified_room.project;
            state.project = modified_room.project.clone();
            state.room = modified_room.room_name.clone();
            if project_changed {
                refresh_room_list(state)?;
            }
            refresh_room_images(state)?;
        }
    }
    Ok(Task::none())
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match try_update(state, message) {
        Ok(t) => t,
        Err(e) => {
            error!("Error: {:?}", e);
            Task::none()
        }
    }
}

struct RoomCanvas<'a> {
    state: &'a State,
}

impl<'a> canvas::Program<Message> for RoomCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _internal_state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let state = self.state;
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let Some(working_images) = &state.working_image_handles else {
            return vec![];
        };
        let width = working_images.width;
        let height = working_images.height;
        let rect = Rectangle::new(
            Point::new(0.0, 0.0),
            Size {
                width: width as f32 * state.pixel_size,
                height: height as f32 * state.pixel_size,
            },
        );

        let bg_color = if state.highlight_transparency {
            iced::Color::from_rgb8(255, 105, 180)
        } else {
            iced::Color::BLACK
        };
        frame.fill_rectangle(
            Point { x: 0.0, y: 0.0 },
            Size {
                width: width as f32 * state.pixel_size,
                height: height as f32 * state.pixel_size,
            },
            bg_color,
        );

        let images = match state.source_selection {
            SourceSelection::WorkingCopy => state.working_image_handles.as_ref().unwrap(),
            SourceSelection::GitReference(_) => state.other_image_handles.as_ref().unwrap(),
            SourceSelection::Difference => state.diff_image_handles.as_ref().unwrap(),
        };
        let state_idx = state.room_state.0;

        if state.show_layer_2 {
            frame.draw_image(
                rect,
                canvas::Image::new(&images.layer2[state_idx])
                    .filter_method(image::FilterMethod::Nearest),
            );
        }
        if state.show_layer_1 {
            frame.draw_image(
                rect,
                canvas::Image::new(&images.layer1[state_idx])
                    .filter_method(image::FilterMethod::Nearest),
            );
        }

        vec![frame.into_geometry()]
    }
}

fn view(state: &State) -> Element<Message> {
    let controls = column![
        combo_box(
            &state.project_list,
            "",
            Some(&state.project),
            Message::SelectProject,
        ),
        combo_box(&state.room_list, "", Some(&state.room), Message::SelectRoom),
        combo_box(
            &state.room_state_list,
            "",
            Some(&state.room_state),
            Message::SelectRoomState
        ),
        row![
            checkbox("Show layer 1", state.show_layer_1).on_toggle(Message::ShowLayer1),
            checkbox("Show layer 2", state.show_layer_2).on_toggle(Message::ShowLayer2),
        ]
        .spacing(10),
        checkbox("Highlight transparency", state.highlight_transparency)
            .on_toggle(Message::HighlightTransparency),
        row![
            text("Difference baseline"),
            slider(
                0.0..=1.0,
                state.difference_baseline,
                Message::AdjustDifferenceBaseline
            )
            .step(0.01)
        ]
        .spacing(10),
        pick_list(
            [
                SourceSelection::WorkingCopy,
                SourceSelection::GitReference(state.git_reference.clone()),
                SourceSelection::Difference
            ],
            Some(&state.source_selection),
            Message::SelectSource,
        ),
        SelectionList::new_with(
            &state.modified_room_list,
            |idx, _| Message::SelectModifiedRoom(idx),
            14.0,
            5.0,
            iced_aw::style::selection_list::primary,
            state.modified_room_idx.clone(),
            Font::default(),
        )
    ]
    .spacing(10);

    let mut width = 256;
    let mut height = 256;
    if let Some(working_images) = &state.working_image_handles {
        width = working_images.width;
        height = working_images.height;
    }

    let image = Scrollable::with_direction(
        canvas(RoomCanvas { state })
            .width(width as f32 * state.pixel_size + 15.0)
            .height(height as f32 * state.pixel_size + 15.0),
        scrollable::Direction::Both {
            vertical: Scrollbar::default(),
            horizontal: Scrollbar::default(),
        },
    );

    row![controls.width(350), image.width(Length::Fill)]
        .spacing(10)
        .padding(10)
        .into()
}

fn theme(_state: &State) -> Theme {
    match dark_light::detect().unwrap_or(dark_light::Mode::Unspecified) {
        dark_light::Mode::Light => Theme::Light,
        dark_light::Mode::Dark | dark_light::Mode::Unspecified => Theme::Dark,
    }
}

fn subscription(_state: &State) -> Subscription<Message> {
    iced::event::listen().map(Message::Event)
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("smartdiff=info"))
        .format_timestamp_millis()
        .init();

    let state = get_initial_state()?;

    iced::application("SMART diff", update, view)
        .theme(theme)
        .subscription(subscription)
        .window_size(Size {
            width: 1440.0,
            height: 960.0,
        })
        .run_with(|| (state, Task::none()))?;

    Ok(())
}
