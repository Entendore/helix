//! Intrinsic gates (TST-11/12): properties of the TEST, not the CI matrix —
//! every job runs every test; tests no-op themselves where a gate is closed.
pub enum Gate {
    All,
    LinuxOnly,
    Gpu,
}

impl Gate {
    pub fn open(&self) -> bool {
        match self {
            Gate::All => true,
            Gate::LinuxOnly => cfg!(target_os = "linux"),
            Gate::Gpu => crate::gates::gpu_available(),
        }
    }
}

/// Adapter probe; None => no usable backend (skip, don't panic).
pub fn gpu_available() -> bool {
    pollster::block_on(async {
        wgpu::Instance::new(wgpu::InstanceDescriptor::default())
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .is_some()
    })
}
