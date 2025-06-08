mod file_system;
mod room;
mod smart_xml;

use std::{fmt::Display, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use git2::Repository;
use iced::{
    Element, Length, Point, Rectangle, Size, Subscription, Task, Theme, keyboard,
    widget::{
        Scrollable, canvas, checkbox, column, combo_box, image, pick_list, row,
        scrollable::{self, Scrollbar},
    },
};
use log::{error, info};

use crate::file_system::{GitTreeFileSystem, LocalFileSystem};
use crate::room::render_room;

pub const MIN_PIXEL_SIZE: f32 = 1.0;
pub const MAX_PIXEL_SIZE: f32 = 8.0;

#[derive(Parser)]
struct Args {
    reference: Option<String>,
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

struct State {
    repo: git2::Repository,
    git_reference: String,
    project_list: combo_box::State<Project>,
    project: Project,
    room_list: combo_box::State<String>,
    room: String,
    room_state_list: combo_box::State<RoomState>,
    room_state: RoomState,
    show_layer_1: bool,
    show_layer_2: bool,
    highlight_transparency: bool,
    pixel_size: f32,
    source_selection: SourceSelection,
    working_images: Option<RoomData>,
    other_images: Option<RoomData>,
    diff_images: Option<RoomData>,
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
    SelectSource(SourceSelection),
    ShowLayer1(bool),
    ShowLayer2(bool),
    HighlightTransparency(bool),
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
        show_layer_1: true,
        show_layer_2: true,
        highlight_transparency: false,
        source_selection: SourceSelection::WorkingCopy,
        pixel_size: 1.0,
        working_images: None,
        other_images: None,
        diff_images: None,
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

fn diff_image(img1: &room::Image, img2: &room::Image) -> room::Image {
    let mut img = room::Image::new(img1.width, img1.height);
    for y in 0..img.height {
        for x in 0..img.width {
            if img1.get_pixel(x, y) != img2.get_pixel(x, y) {
                img.set_pixel(x, y, [255, 255, 255]);
            }
        }
    }
    img
}

fn diff_image_list(img1: &[room::Image], img2: &[room::Image]) -> Vec<room::Image> {
    img1.iter()
        .zip(img2.iter())
        .map(|(x, y)| diff_image(x, y))
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
    let width = working_images.layer1[0].width;
    let height = working_images.layer1[0].height;

    let reference = state.repo.find_reference(&state.git_reference)?;
    let tree = reference.peel_to_tree()?;
    let other_fs = GitTreeFileSystem {
        repo: &state.repo,
        tree,
    };
    let other_images = render_room(&state.project.0, &state.room, &other_fs)?;

    state.diff_images = Some(RoomData {
        width,
        height,
        layer1: convert_images(diff_image_list(
            &working_images.layer1,
            &other_images.layer1,
        )),
        layer2: convert_images(diff_image_list(
            &working_images.layer2,
            &other_images.layer2,
        )),
    });
    state.working_images = Some(RoomData {
        width,
        height,
        layer1: convert_images(working_images.layer1),
        layer2: convert_images(working_images.layer2),
    });
    state.other_images = Some(RoomData {
        width,
        height,
        layer1: convert_images(other_images.layer1),
        layer2: convert_images(other_images.layer2),
    });
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

        let Some(working_images) = &state.working_images else {
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
            SourceSelection::WorkingCopy => state.working_images.as_ref().unwrap(),
            SourceSelection::GitReference(_) => state.other_images.as_ref().unwrap(),
            SourceSelection::Difference => state.diff_images.as_ref().unwrap(),
        };

        if state.show_layer_2 {
            frame.draw_image(
                rect,
                canvas::Image::new(&images.layer2[0]).filter_method(image::FilterMethod::Nearest),
            );
        }
        if state.show_layer_1 {
            frame.draw_image(
                rect,
                canvas::Image::new(&images.layer1[0]).filter_method(image::FilterMethod::Nearest),
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
        combo_box(&state.room_list, "", Some(&state.room), Message::SelectRoom,),
        checkbox("Show layer 1", state.show_layer_1).on_toggle(Message::ShowLayer1),
        checkbox("Show layer 2", state.show_layer_2).on_toggle(Message::ShowLayer2),
        checkbox("Highlight transparency", state.highlight_transparency)
            .on_toggle(Message::HighlightTransparency),
        pick_list(
            [
                SourceSelection::WorkingCopy,
                SourceSelection::GitReference(state.git_reference.clone()),
                SourceSelection::Difference
            ],
            Some(&state.source_selection),
            Message::SelectSource,
        ),
    ]
    .spacing(10);

    let mut width = 256;
    let mut height = 256;
    if let Some(working_images) = &state.working_images {
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
