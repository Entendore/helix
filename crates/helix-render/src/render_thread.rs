//! Render thread (THR-1..2, REN-6..9): renderer moved by value; bounded
//! sync_channel(2) = backpressure; a dead thread never hangs the main loop
//! (REN-8). FrameData is plain data — the extraction boundary (REN-9).
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use glam::{Mat4, Vec3};
use helix_assets::DrawCall;

pub struct ExtractedView {
    pub view_proj: Mat4,
    pub position: Vec3,
    pub draws: Vec<DrawCall>,
}

pub struct FrameData {
    pub size: (u32, u32),
    /// views[0] is the swapchain view (CAM-2 shape; more views at P2).
    pub views: Vec<ExtractedView>,
    /// RES-1: 3D renders scaled, UI/resolution independent.
    pub render_scale: f32,
    /// POST-2: exposure in EV.
    pub exposure_ev: f32,
}

pub enum RenderMessage {
    Resize(u32, u32),
    Frame(FrameData),
    Shutdown,
}

pub struct RenderProxy {
    tx: Option<SyncSender<RenderMessage>>,
    handle: Option<JoinHandle<()>>,
}

impl RenderProxy {
    /// Renderer built on the main thread (THR-3), then moved by value.
    pub fn spawn(renderer: crate::renderer::Renderer) -> Self {
        let (tx, rx) = sync_channel::<RenderMessage>(2); // REN-6 backpressure
        let handle = thread::Builder::new()
            .name("helix-render".into())
            .spawn(move || render_loop(renderer, rx))
            .expect("spawn render thread");
        Self { tx: Some(tx), handle: Some(handle) }
    }

    /// Blocks while the queue is full (the pacing mechanism). No-op after
    /// the render thread dies — a GPU crash can't hang the main loop.
    pub fn send(&mut self, message: RenderMessage) {
        let Some(tx) = &self.tx else { return };
        if tx.send(message).is_err() {
            log::error!("render thread terminated; discarding messages");
            self.tx = None;
        }
    }
}

impl Drop for RenderProxy {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(RenderMessage::Shutdown); // never blocks (REN-8)
        }
        self.tx = None;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// REN-7 protocol state machine.
fn render_loop(mut renderer: crate::renderer::Renderer, rx: Receiver<RenderMessage>) {
    loop {
        match rx.recv() {
            Ok(RenderMessage::Resize(w, h)) => renderer.resize(w, h),
            Ok(RenderMessage::Frame(frame)) => {
                if frame.size.0 == 0 || frame.size.1 == 0 {
                    continue; // WIN-1: minimized is a no-op
                }
                if renderer.size() != frame.size {
                    renderer.resize(frame.size.0, frame.size.1); // resize/frame race
                }
                match renderer.render(&frame) {
                    Ok(()) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        renderer.resize(frame.size.0, frame.size.1); // self-heal
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        log::error!("GPU OOM; render thread stopping");
                        break;
                    }
                    Err(e) => log::warn!("surface error: {e:?}"),
                }
            }
            Ok(RenderMessage::Shutdown) | Err(_) => break,
        }
    }
    log::info!("render thread exiting");
}
