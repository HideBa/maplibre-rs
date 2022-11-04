use std::rc::Rc;

use crate::{
    context::MapContext,
    coords::{LatLon, WorldCoords, Zoom, TILE_SIZE},
    environment::Environment,
    error::Error,
    kernel::Kernel,
    render::{
        builder::{
            InitializationResult, InitializedRenderer, RendererBuilder, UninitializedRenderer,
        },
        create_default_render_graph, register_default_render_stages,
        settings::{RendererSettings, WgpuSettings},
        Renderer,
    },
    schedule::{Schedule, Stage},
    stages::register_stages,
    style::Style,
    window::{HeadedMapWindow, MapWindow, MapWindowConfig, WindowSize},
    world::World,
};

pub enum MapContextState {
    Ready(MapContext),
    Pending { style: Style },
}

pub struct Map<E: Environment> {
    kernel: Rc<Kernel<E>>,
    schedule: Schedule,
    map_context: MapContextState,
    window: <E::MapWindowConfig as MapWindowConfig>::MapWindow,
}

impl<E: Environment> Map<E>
where
    <<E as Environment>::MapWindowConfig as MapWindowConfig>::MapWindow: HeadedMapWindow,
{
    pub fn new(style: Style, kernel: Kernel<E>) -> Result<Self, Error> {
        let mut schedule = Schedule::default();

        let graph = create_default_render_graph().unwrap(); // TODO: Remove unwrap
        register_default_render_stages(graph, &mut schedule);

        let kernel = Rc::new(kernel);

        register_stages::<E>(&mut schedule, kernel.clone());

        let mut window = kernel.map_window_config().create();

        let map = Self {
            kernel,
            map_context: MapContextState::Pending { style },
            schedule,
            window,
        };
        Ok(map)
    }

    pub async fn initialize_renderer(
        &mut self,
        render_builder: RendererBuilder,
    ) -> Result<(), Error> {
        let result = render_builder
            .build()
            .initialize_renderer::<E::MapWindowConfig>(&self.window)
            .await
            .expect("Failed to initialize renderer");

        match &mut self.map_context {
            MapContextState::Ready(_) => Err(Error::Generic("Renderer is already set".into())),
            MapContextState::Pending { style } => {
                let window_size = self.window.size();

                let center = style.center.unwrap_or_default();

                let world = World::new_at(
                    window_size,
                    LatLon::new(center[0], center[1]),
                    style.zoom.map(|zoom| Zoom::new(zoom)).unwrap_or_default(),
                    cgmath::Deg::<f64>(style.pitch.unwrap_or_default()),
                );

                match result {
                    InitializationResult::Initialized(InitializedRenderer { renderer, .. }) => {
                        *&mut self.map_context = MapContextState::Ready(MapContext {
                            world,
                            style: std::mem::take(style),
                            renderer,
                        });
                    }
                    InitializationResult::Uninizalized(UninitializedRenderer { .. }) => {}
                    _ => panic!("Rendering context gone"),
                };
                Ok(())
            }
        }
    }

    pub fn window_mut(&mut self) -> &mut <E::MapWindowConfig as MapWindowConfig>::MapWindow {
        &mut self.window
    }
    pub fn window(&self) -> &<E::MapWindowConfig as MapWindowConfig>::MapWindow {
        &self.window
    }

    pub fn has_renderer(&self) -> bool {
        match &self.map_context {
            MapContextState::Ready(_) => true,
            MapContextState::Pending { .. } => false,
        }
    }

    #[tracing::instrument(name = "update_and_redraw", skip_all)]
    pub fn run_schedule(&mut self) -> Result<(), Error> {
        match &mut self.map_context {
            MapContextState::Ready(map_context) => {
                self.schedule.run(map_context);
                Ok(())
            }
            MapContextState::Pending { .. } => {
                Err(Error::Generic("Renderer is already set".into()))
            }
        }
    }

    pub fn context(&self) -> Result<&MapContext, Error> {
        match &self.map_context {
            MapContextState::Ready(map_context) => Ok(map_context),
            MapContextState::Pending { .. } => {
                Err(Error::Generic("Renderer is already set".into()))
            }
        }
    }

    pub fn context_mut(&mut self) -> Result<&mut MapContext, Error> {
        match &mut self.map_context {
            MapContextState::Ready(map_context) => Ok(map_context),
            MapContextState::Pending { .. } => {
                Err(Error::Generic("Renderer is already set".into()))
            }
        }
    }

    pub fn kernel(&self) -> &Rc<Kernel<E>> {
        &self.kernel
    }
}