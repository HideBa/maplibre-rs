//! Main (platform-specific) main loop which handles:
//! * Input (Mouse/Keyboard)
//! * Platform Events like suspend/resume
//! * Render a new frame

use maplibre::{
    event_loop::EventLoop,
    headless::map::HeadlessMap,
    io::apc::SchedulerAsyncProcedureCall,
    kernel::{Kernel, KernelBuilder},
    map::Map,
    platform::{http_client::ReqwestHttpClient, run_multithreaded, scheduler::TokioScheduler},
    render::builder::RenderBuilder,
    style::Style,
    window::{HeadedMapWindow, MapWindow, MapWindowConfig, WindowSize},
};
use winit::window::WindowBuilder;

use super::{RawWinitEventLoop, RawWinitWindow, WinitMapWindow, WinitMapWindowConfig};
use crate::{WinitEnvironment, WinitEventLoop};

impl<T> MapWindow for WinitMapWindow<T> {
    fn size(&self) -> WindowSize {
        let size = self.window.inner_size();
        #[cfg(target_os = "android")]
        // On android we can not get the dimensions of the window initially. Therefore, we use a
        // fallback until the window is ready to deliver its correct bounds.
        let window_size =
            WindowSize::new(size.width, size.height).unwrap_or(WindowSize::new(100, 100).unwrap());

        #[cfg(not(target_os = "android"))]
        let window_size =
            WindowSize::new(size.width, size.height).expect("failed to get window dimensions.");
        window_size
    }
}
impl<T> HeadedMapWindow for WinitMapWindow<T> {
    type RawWindow = RawWinitWindow;

    fn raw(&self) -> &Self::RawWindow {
        &self.window
    }

    fn request_redraw(&self) {
        self.window.request_redraw()
    }

    fn id(&self) -> u64 {
        self.window.id().into()
    }
}

impl<ET: 'static> MapWindowConfig for WinitMapWindowConfig<ET> {
    type MapWindow = WinitMapWindow<ET>;

    fn create(&self) -> Self::MapWindow {
        let raw_event_loop = winit::event_loop::EventLoopBuilder::<ET>::with_user_event().build();
        let window = WindowBuilder::new()
            .with_title(&self.title)
            .build(&raw_event_loop)
            .unwrap();

        Self::MapWindow {
            window,
            event_loop: Some(WinitEventLoop {
                event_loop: raw_event_loop,
            }),
        }
    }
}

pub fn run_headed_map(cache_path: Option<String>) {
    run_multithreaded(async {
        let client = ReqwestHttpClient::new(cache_path);
        let kernel: Kernel<WinitEnvironment<_, _, _, ()>> = KernelBuilder::new()
            .with_map_window_config(WinitMapWindowConfig::new("maplibre".to_string()))
            .with_http_client(client.clone())
            .with_apc(SchedulerAsyncProcedureCall::new(
                client,
                TokioScheduler::new(),
            ))
            .with_scheduler(TokioScheduler::new())
            .build();

        let uninitialized = RenderBuilder::new()
            .build()
            .initialize_with(&kernel)
            .await
            .expect("Failed to initialize renderer");
        let result = uninitialized.unwarp();

        let mut window = result.window;
        let renderer = result.renderer;
        window.event_loop.take().unwrap().run(
            window,
            Map::new(Style::default(), kernel, renderer).unwrap(),
            None,
        )
    })
}
