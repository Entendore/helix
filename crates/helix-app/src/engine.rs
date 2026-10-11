//! Windowed Engine (spec §3.2 frame loop): fixed-step Input/Sim, per-frame
//! Late + interpolated camera ease, cull+sort extraction, render thread.
use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;
use helix_assets::{DrawCall, MeshAssets};
use helix_core::config::Config;
use helix_core::FIXED_DT;
use helix_ecs::{
    Channel, CollisionEvent, EngineBuilder, GlobalTransform, MeshId as GameMeshId, Player,
    PrevGlobal,
};
use helix_render::{
    Camera, ExtractedView, FrameData, Frustum, RenderMessage, RenderProxy, Renderer,
    DEPTH_REVERSED,
};
use helix_scene::{MeshColor, MeshRadius};
use winit::application::ApplicationHandler;
use winit::event::{ControlFlow, ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::capabilities;
use crate::console::{self, Console};
use crate::input::{build_actions, Bindings, Input};

pub struct App {
    pub inner: Option<Engine>,
    builder: Option<EngineBuilder>,
    pub config: Config,
}

impl App {
    pub fn new(config: Config, builder: EngineBuilder) -> Self {
        Self { inner: None, builder: Some(builder), config }
    }
}

pub struct Engine {
    window: Arc<Window>,
    asm: helix_ecs::Assembled,
    input: Input,
    bindings: Bindings,
    actions: helix_ecs::ActionState,
    camera: Camera,
    events: Channel<CollisionEvent>,
    render: RenderProxy,
    console: Console,
    accumulator: f32,
    last_frame: Instant,
    fps_time: f32,
    fps_frames: u32,
    interp: bool,
    max_steps: u32,
}

impl Engine {
    pub async fn new(window: Arc<Window>, mut builder: EngineBuilder, config: Config) -> Engine {
        let (mut renderer, caps) = Renderer::new(window.clone(), config.vsync).await;
        capabilities::seed_cvars(builder.cvars_mut(), &caps); // CAP-1
        let asm = builder.finish().expect("engine assembly (MOD-3 guarded)");

        let (verts, indices) = helix_assets::cube();
        let radius = helix_assets::bounds(&verts);
        let cube = renderer.register_mesh(&verts, &indices);
        let mut assets = MeshAssets::new();
        assets.insert("cube", cube, radius);

        let level_src = std::fs::read_to_string("assets/level.ron")
            .unwrap_or_else(|_| helix_scene::level::DEFAULT_LEVEL.to_string());
        helix_scene::level::load(&mut asm.world, &assets, &level_src).expect("level load");
        let e = asm.world.spawn(); // engine-owned RNG entity (RNG-1)
        asm.world.insert(e, helix_ecs::WorldRng::new(asm.seed));
        asm.world.insert(e, helix_ecs::Name("__rng".into()));

        let bindings_src = std::fs::read_to_string("assets/bindings.ron")
            .unwrap_or_else(|_| crate::input::DEFAULT_BINDINGS.to_string());
        let render = RenderProxy::spawn(renderer);

        Engine {
            window,
            asm,
            input: Input::default(),
            bindings: Bindings::from_source(&bindings_src),
            actions: helix_ecs::ActionState::default(),
            camera: Camera::new(),
            events: Channel::new(),
            render,
            console: Console::default(),
            accumulator: 0.0,
            last_frame: Instant::now(),
            fps_time: 0.0,
            fps_frames: 0,
            interp: config.interpolation,
            max_steps: config.max_sim_steps,
        }
    }

    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.render.send(RenderMessage::Resize(size.width, size.height));
            }
            WindowEvent::Focused(false) => {
                self.input.clear(); // WIN-2
                self.console.open = false;
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state, text, .. },
                ..
            } => {
                if physical_key == PhysicalKey::Code(KeyCode::Escape)
                    && state == ElementState::Pressed
                {
                    event_loop.exit();
                }
                if cfg!(debug_assertions)
                    && physical_key == PhysicalKey::Code(KeyCode::Backquote)
                    && state == ElementState::Pressed
                {
                    self.console.toggle();
                    return;
                }
                if self.console.open {
                    self.console.key(physical_key, text.as_deref());
                } else {
                    self.input.set_key(physical_key, state);
                }
            }
            _ => {}
        }
    }

    pub fn redraw(&mut self) {
        let now = Instant::now();
        let frame_dt = (now - self.last_frame).as_secs_f32().min(0.25); // TIME-3
        self.last_frame = now;

        self.actions = build_actions(&self.input, &self.bindings); // ACT-4

        // CVAR-2: boundary execution, never mid-stage.
        for cmd in self.console.queue.drain(..) {
            let out = console::execute(
                &cmd,
                &mut self.asm.cvars,
                &mut self.asm.world,
                &self.asm.registry,
                self.asm.seed,
            );
            log::info!("console: {cmd} -> {out}");
        }

        // ---- fixed-step simulation (TIME-1/4) ----
        self.accumulator += frame_dt;
        let mut steps = 0;
        while self.accumulator >= FIXED_DT {
            if steps >= self.max_steps {
                log::warn!("sim overloaded; dropped backlog");
                self.accumulator %= FIXED_DT;
                break;
            }
            self.run_stage(helix_ecs::Stage::Input, FIXED_DT, 1.0);
            self.run_stage(helix_ecs::Stage::Sim, FIXED_DT, 1.0);
            self.accumulator -= FIXED_DT;
            steps += 1;
        }
        self.asm.world.tick += 1; // CHG-1 stamps advance per frame
        self.events.update(); // EVT-7: windowed granularity = per frame
        let alpha = (self.accumulator / FIXED_DT).clamp(0.0, 1.0);

        // ---- Late (hierarchy inside; camera eased on interpolated pose) ----
        self.run_stage(helix_ecs::Stage::Late, frame_dt, alpha);
        if let Some((e, g, _)) =
            self.asm.world.query2::<GlobalTransform, Player>().next()
        {
            let prev = self.asm.world.get::<PrevGlobal>(e).map(|p| p.0);
            let pose = if self.interp {
                helix_ecs::hierarchy::interpolate_pose(
                    prev,
                    g.0,
                    alpha,
                    helix_sim::components::SNAP,
                )
            } else {
                g.0
            };
            let wp = pose.w_axis.truncate();
            let k = 1.0 - (-6.0 * frame_dt).exp();
            self.camera.position =
                self.camera.position.lerp(wp + Vec3::new(0.0, 6.5, 9.5), k);
            self.camera.look_at = self.camera.look_at.lerp(wp, k);
        }

        // ---- extract: interpolate, cull, sort (INTP-4 pure, REN-13/14) ----
        let size = self.window.inner_size();
        let size = (size.width, size.height);
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        let view_proj = self.camera.view_proj(aspect);
        let frustum = Frustum::from_view_proj(&view_proj, DEPTH_REVERSED);
        let cam_pos = self.camera.position;

        let mut draws: Vec<(DrawCall, f32)> = Vec::new();
        for (e, g, m, c) in self
            .asm
            .world
            .query3::<GlobalTransform, GameMeshId, MeshColor>()
        {
            let curr = g.0;
            let model = if self.interp {
                let prev = self.asm.world.get::<PrevGlobal>(e).map(|p| p.0);
                helix_ecs::hierarchy::interpolate_pose(
                    prev,
                    curr,
                    alpha,
                    helix_sim::components::SNAP,
                )
            } else {
                curr
            };
            let radius = self.asm.world.get::<MeshRadius>(e).map(|r| r.0).unwrap_or(0.5);
            let center = model.w_axis.truncate();
            let col_scale = model
                .to_cols_array_2d()
                .iter()
                .take(3)
                .map(|c2| Vec3::from_array([c2[0], c2[1], c2[2]]).length())
                .fold(0.0f32, f32::max);
            if !frustum.sphere_visible(center, radius * col_scale) {
                continue; // REN-13
            }
            draws.push((
                DrawCall { mesh: *m, model, color: c.0 },
                center.distance_squared(cam_pos),
            ));
        }
        draws.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let draws: Vec<DrawCall> = draws.into_iter().map(|(d, _)| d).collect();

        let render_scale = self.asm.cvars.f32("render.scale");
        let exposure_ev = self.asm.cvars.f32("render.exposure");
        self.render.send(RenderMessage::Frame(FrameData {
            size,
            views: vec![ExtractedView { view_proj, position: cam_pos, draws }],
            render_scale,
            exposure_ev,
        }));

        // ---- TIME-7 dev overlay (title bar) ----
        self.fps_time += frame_dt;
        self.fps_frames += 1;
        if self.fps_time >= 0.5 {
            let fps = self.fps_frames as f32 / self.fps_time;
            let mut title = format!("Helix — {:.0} fps", fps);
            if self.console.open {
                title.push_str(&self.console.prompt());
            }
            self.window.set_title(&title);
            self.fps_time = 0.0;
            self.fps_frames = 0;
        }
    }

    fn run_stage(&mut self, stage: helix_ecs::Stage, dt: f32, alpha: f32) {
        let mut ctx = helix_ecs::Ctx {
            world: &mut self.asm.world,
            actions: &self.actions,
            events: &mut self.events,
            cvars: &mut self.asm.cvars,
            dt,
            alpha,
            interp: self.interp,
        };
        self.asm.scheduler.run_stage(stage, &mut ctx, &self.asm.hooks);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.inner.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Helix — hybrid ECS + wgpu demo")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.window.0 as f64,
                self.config.window.1 as f64,
            ));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let builder = self.builder.take().expect("builder present at resume");
        let engine = pollster::block_on(Engine::new(window.clone(), builder, self.config));
        engine.window.request_redraw();
        self.inner = Some(engine);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(engine) = self.inner.as_mut() {
            engine.window_event(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(engine) = self.inner.as_ref() {
            engine.window.request_redraw();
        }
    }
}
