//! GPU compute opcional. Solo analiza las secuencias ya generadas por CPU.
//! Nunca participa en RNG, claves, pagos ni generacion de resultados.

use rng_grid::audit::{CHECKPOINTS, Trace};
use rng_grid::Catalog;
use std::sync::mpsc;
use std::time::Duration;
use wgpu::util::DeviceExt;

pub struct GpuAnalyzer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    device_name: String,
}

impl GpuAnalyzer {
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_without_display_handle(),
        );
        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                ..wgpu::RequestAdapterOptions::default()
            },
        ))
        .map_err(|error| format!(
            "No hay una GPU compatible con Vulkan o DirectX 12: {error}. \
             Ejecute la auditoria sin --gpu."
        ))?;
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            return Err("El adaptador encontrado es software, no una GPU. \
                        Ejecute sin --gpu.".to_owned());
        }
        let name = format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type);
        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor::default())
        )
        .map_err(|error| format!("Error al inicializar GPU {name}: {error}"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RNG conditional EV audit"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("gpu_compute.wgsl").into()
            ),
        });
        let pipeline = device.create_compute_pipeline(
            &wgpu::ComputePipelineDescriptor {
                label: Some("RNG audit compute"),
                layout: None,
                module: &shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            },
        );
        Ok(Self {
            device,
            queue,
            pipeline,
            device_name: name,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Requiere lotes de <= 100.000 paquetes. Compara con CPU en
    /// el llamador y descarta resultados si difieren.
    pub fn compute(
        &mut self,
        traces: &[Trace],
        catalog: &Catalog,
    ) -> Result<Vec<[u32; CHECKPOINTS.len()]>, String> {
        if traces.is_empty() {
            return Ok(Vec::new());
        }
        if traces.len() > 100_000 {
            return Err("Lote GPU demasiado grande (limite: 100000)".to_owned());
        }
        let ids: Vec<u32> = traces
            .iter()
            .flat_map(|trace| trace.iter().copied())
            .collect();
        let rewards: Vec<u32> = catalog
            .outcomes
            .iter()
            .map(|outcome| outcome.payout_bps)
            .collect();
        let encode = |values: &[u32]| {
            let mut bytes = Vec::with_capacity(values.len() * 4);
            for &value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            bytes
        };

        let ids_bytes = encode(&ids);
        let rewards_bytes = encode(&rewards);
        let ids_buffer = self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("128 outcome type ID traces"),
                contents: &ids_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            },
        );
        let payout_buffer = self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("128 outcome paytable"),
                contents: &rewards_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            },
        );
        let output_size = (traces.len() * CHECKPOINTS.len() * 4) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU conditional ROI results"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RNG audit GPU readback"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = self.pipeline.get_bind_group_layout(0);
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RNG audit bind group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ids_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: payout_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("RNG GPU audit encoder"),
            },
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("RNG conditional RTP"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups((traces.len() as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging, 0, output_size);
        self.queue.submit(Some(encoder.finish()));

        let slice = staging.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(60)),
            })
            .map_err(|error| format!("GPU timeout o fallo de polling: {error}"))?;
        receiver
            .recv_timeout(Duration::from_secs(60))
            .map_err(|error| format!("GPU no devolvio datos: {error}"))?
            .map_err(|error| format!("No se pudo mapear salida GPU: {error}"))?;
        let buffer_view = slice
            .get_mapped_range()
            .map_err(|error| format!("No se pudo leer GPU: {error}"))?;
        let flat: Vec<u32> = buffer_view
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(
                chunk.try_into().expect("chunks_exact produce 4 bytes")
            ))
            .collect();
        drop(buffer_view);
        staging.unmap();

        if flat.len() != traces.len() * CHECKPOINTS.len() {
            return Err("La GPU produjo una cantidad de resultados inesperada".to_owned());
        }
        let result = flat
            .chunks_exact(CHECKPOINTS.len())
            .map(|chunk| {
                let mut scores = [0_u32; CHECKPOINTS.len()];
                scores.copy_from_slice(chunk);
                scores
            })
            .collect();
        Ok(result)
    }
}
