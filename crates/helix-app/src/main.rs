//! Helix demo binary — a thin winit shell (ADR-017). All assembly is modules.
use helix_app::engine::App;
use helix_core::config::Config;
use helix_ecs::EngineBuilder;

fn main() {
    env_logger::init();
    let config = Config::load();
    let event_loop = winit::event_loop::EventLoop::with_user_event()
        .build()
        .expect("event loop");
    event_loop.set_control_flow(winit::event::ControlFlow::Wait);

    // MOD-1: assembly = a list of modules. That is the whole engine.
    let mut builder = EngineBuilder::new().seed(config.seed);
    builder.module(Box::new(helix_sim::modules::CoreSimModule));
    builder.module(Box::new(helix_sim::modules::PillarPlazaModule));
    builder.module(Box::new(helix_app::modules::RenderModule));

    let mut app = App::new(config, builder);
    event_loop.run_app(&mut app).expect("event loop error");
}
