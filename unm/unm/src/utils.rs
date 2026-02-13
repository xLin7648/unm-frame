use wgpu::{Buffer, BufferAddress, BufferDescriptor, BufferUsages, Device, Queue, util::{self, DeviceExt}};

pub struct SizedBuffer {
    pub buffer: Buffer,
    pub size: usize,
    pub buffer_type: BufferType,
    pub label: String,
}

impl SizedBuffer {
    pub fn new(label: &str, device: &Device, size: usize, buffer_type: BufferType) -> Self {
        let desc = BufferDescriptor {
            label: Some(label),
            usage: buffer_type.usage(),
            size: size as BufferAddress,
            mapped_at_creation: false,
        };

        let buffer = device.create_buffer(&desc);

        Self {
            label: label.to_string(),
            size,
            buffer_type,
            buffer,
        }
    }

    pub fn ensure_size_and_copy(
        &mut self,
        device: &Device,
        queue: &Queue,
        data: &[u8],
    ) {
        if data.len() > self.size {
            self.buffer.destroy();
            self.size = data.len();
            self.buffer = device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some(&self.label),
                usage: self.buffer_type.usage(),
                contents: data,
            });
        } else {
            queue.write_buffer(&self.buffer, 0, data);
        }
    }
}

pub enum BufferType {
    Vertex,
    Index,
    Instance,
    Uniform,
    Storage,
    Read,
}

impl BufferType {
    pub fn usage(&self) -> BufferUsages {
        match self {
            BufferType::Vertex => BufferUsages::VERTEX | BufferUsages::COPY_DST,
            BufferType::Index => BufferUsages::INDEX | BufferUsages::COPY_DST,
            BufferType::Instance => BufferUsages::VERTEX | BufferUsages::COPY_DST,
            BufferType::Uniform => BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            BufferType::Read => BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            BufferType::Storage => {
                todo!()
            }
        }
    }
}