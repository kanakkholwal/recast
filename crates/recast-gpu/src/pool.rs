use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
    pub sample_count: u32,
}

impl TextureDesc {
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            width,
            height,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            sample_count: 1,
        }
    }

    pub fn with_usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn byte_size(&self) -> u64 {
        let bytes_per_pixel = self.format.block_copy_size(None).unwrap_or(4).max(1) as u64;
        (self.width as u64) * (self.height as u64) * bytes_per_pixel * (self.sample_count as u64)
    }
}

pub trait TextureAllocator {
    type Texture;

    fn create(&self, desc: &TextureDesc) -> Self::Texture;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoolStats {
    pub live: u32,
    pub idle: u32,
    pub created: u64,
    pub reused: u64,
    pub evicted: u64,
    pub idle_bytes: u64,
    pub live_bytes: u64,
}

impl PoolStats {
    pub fn total_bytes(&self) -> u64 {
        self.idle_bytes + self.live_bytes
    }
}

struct Idle<T> {
    texture: T,
    unused_for: u32,
}

/// A texture handed out by the pool. Dropping it does NOT recycle: the frame
/// graph returns textures explicitly at the end of a frame, so a pass that
/// forgets one shows up as a leak in `stats().live` instead of silently
/// recycling a surface another pass is still sampling.
pub struct Lease<T> {
    pub texture: T,
    desc: TextureDesc,
}

impl<T> Lease<T> {
    pub fn desc(&self) -> TextureDesc {
        self.desc
    }
}

pub struct TexturePool<A: TextureAllocator> {
    allocator: A,
    idle: HashMap<TextureDesc, Vec<Idle<A::Texture>>>,
    max_idle_bytes: u64,
    max_unused_frames: u32,
    stats: PoolStats,
}

pub const DEFAULT_MAX_IDLE_BYTES: u64 = 256 * 1024 * 1024;
pub const DEFAULT_MAX_UNUSED_FRAMES: u32 = 120;

impl<A: TextureAllocator> TexturePool<A> {
    pub fn new(allocator: A) -> Self {
        Self {
            allocator,
            idle: HashMap::new(),
            max_idle_bytes: DEFAULT_MAX_IDLE_BYTES,
            max_unused_frames: DEFAULT_MAX_UNUSED_FRAMES,
            stats: PoolStats::default(),
        }
    }

    pub fn with_limits(mut self, max_idle_bytes: u64, max_unused_frames: u32) -> Self {
        self.max_idle_bytes = max_idle_bytes;
        self.max_unused_frames = max_unused_frames;
        self
    }

    pub fn stats(&self) -> PoolStats {
        self.stats
    }

    pub fn acquire(&mut self, desc: TextureDesc) -> Lease<A::Texture> {
        let recycled = self
            .idle
            .get_mut(&desc)
            .and_then(|bucket| bucket.pop())
            .map(|idle| idle.texture);

        let texture = match recycled {
            Some(texture) => {
                self.stats.reused += 1;
                self.stats.idle -= 1;
                self.stats.idle_bytes -= desc.byte_size();
                texture
            }
            None => {
                self.stats.created += 1;
                self.allocator.create(&desc)
            }
        };

        self.stats.live += 1;
        self.stats.live_bytes += desc.byte_size();
        Lease { texture, desc }
    }

    pub fn release(&mut self, lease: Lease<A::Texture>) {
        let desc = lease.desc;
        self.stats.live -= 1;
        self.stats.live_bytes -= desc.byte_size();
        self.stats.idle += 1;
        self.stats.idle_bytes += desc.byte_size();
        self.idle.entry(desc).or_default().push(Idle {
            texture: lease.texture,
            unused_for: 0,
        });
        self.enforce_budget();
    }

    /// Ages every idle texture by one frame and drops the ones past the limit.
    /// Called once per rendered frame, not per pass.
    pub fn end_frame(&mut self) {
        let max_unused = self.max_unused_frames;
        let mut evicted = 0u64;
        let mut freed_bytes = 0u64;
        for (desc, bucket) in self.idle.iter_mut() {
            let before = bucket.len();
            for entry in bucket.iter_mut() {
                entry.unused_for += 1;
            }
            bucket.retain(|entry| entry.unused_for <= max_unused);
            let dropped = (before - bucket.len()) as u64;
            evicted += dropped;
            freed_bytes += dropped * desc.byte_size();
        }
        self.idle.retain(|_, bucket| !bucket.is_empty());
        self.stats.evicted += evicted;
        self.stats.idle -= evicted as u32;
        self.stats.idle_bytes -= freed_bytes;
    }

    pub fn clear(&mut self) {
        let freed: u64 = self
            .idle
            .iter()
            .map(|(desc, bucket)| bucket.len() as u64 * desc.byte_size())
            .sum();
        let count: u64 = self.idle.values().map(|b| b.len() as u64).sum();
        self.idle.clear();
        self.stats.evicted += count;
        self.stats.idle -= count as u32;
        self.stats.idle_bytes -= freed;
    }

    /// Drops the oldest idle textures until the idle set fits the byte budget.
    fn enforce_budget(&mut self) {
        while self.stats.idle_bytes > self.max_idle_bytes {
            let Some((desc, index)) = self.oldest_idle() else {
                return;
            };
            let Some(bucket) = self.idle.get_mut(&desc) else {
                return;
            };
            bucket.remove(index);
            if bucket.is_empty() {
                self.idle.remove(&desc);
            }
            self.stats.evicted += 1;
            self.stats.idle -= 1;
            self.stats.idle_bytes -= desc.byte_size();
        }
    }

    fn oldest_idle(&self) -> Option<(TextureDesc, usize)> {
        self.idle
            .iter()
            .filter_map(|(desc, bucket)| {
                bucket
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, entry)| entry.unused_for)
                    .map(|(index, entry)| (*desc, index, entry.unused_for))
            })
            .max_by_key(|(_, _, unused_for)| *unused_for)
            .map(|(desc, index, _)| (desc, index))
    }
}

pub struct WgpuAllocator {
    device: wgpu::Device,
    label: &'static str,
}

impl WgpuAllocator {
    pub fn new(device: wgpu::Device, label: &'static str) -> Self {
        Self { device, label }
    }
}

impl TextureAllocator for WgpuAllocator {
    type Texture = wgpu::Texture;

    fn create(&self, desc: &TextureDesc) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(self.label),
            size: wgpu::Extent3d {
                width: desc.width,
                height: desc.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: desc.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: desc.format,
            usage: desc.usage,
            view_formats: &[],
        })
    }
}

pub type GpuTexturePool = TexturePool<WgpuAllocator>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Default)]
    struct CountingAllocator {
        next_id: Cell<u32>,
        created: Rc<Cell<u32>>,
    }

    impl TextureAllocator for CountingAllocator {
        type Texture = u32;

        fn create(&self, _desc: &TextureDesc) -> u32 {
            let id = self.next_id.get();
            self.next_id.set(id + 1);
            self.created.set(self.created.get() + 1);
            id
        }
    }

    fn pool() -> TexturePool<CountingAllocator> {
        TexturePool::new(CountingAllocator::default())
    }

    fn desc(width: u32, height: u32) -> TextureDesc {
        TextureDesc::new(width, height, wgpu::TextureFormat::Rgba16Float)
    }

    #[test]
    fn a_released_texture_is_handed_back_instead_of_reallocated() {
        let mut pool = pool();
        let first = pool.acquire(desc(1920, 1080));
        let id = first.texture;
        pool.release(first);

        let second = pool.acquire(desc(1920, 1080));
        assert_eq!(second.texture, id);
        assert_eq!(pool.stats().created, 1);
        assert_eq!(pool.stats().reused, 1);
    }

    #[test]
    fn a_different_size_does_not_reuse() {
        let mut pool = pool();
        let first = pool.acquire(desc(1920, 1080));
        pool.release(first);
        let second = pool.acquire(desc(1280, 720));
        pool.release(second);
        assert_eq!(pool.stats().created, 2);
        assert_eq!(pool.stats().reused, 0);
    }

    #[test]
    fn a_different_usage_does_not_reuse() {
        let mut pool = pool();
        let a = pool.acquire(desc(64, 64));
        pool.release(a);
        let b = pool.acquire(desc(64, 64).with_usage(wgpu::TextureUsages::COPY_SRC));
        pool.release(b);
        assert_eq!(pool.stats().created, 2);
    }

    #[test]
    fn two_simultaneous_leases_of_the_same_desc_are_distinct_textures() {
        let mut pool = pool();
        let a = pool.acquire(desc(64, 64));
        let b = pool.acquire(desc(64, 64));
        assert_ne!(a.texture, b.texture);
        assert_eq!(pool.stats().live, 2);
    }

    #[test]
    fn live_count_tracks_outstanding_leases() {
        let mut pool = pool();
        let a = pool.acquire(desc(64, 64));
        let b = pool.acquire(desc(64, 64));
        assert_eq!(pool.stats().live, 2);
        pool.release(a);
        assert_eq!(pool.stats().live, 1);
        assert_eq!(pool.stats().idle, 1);
        pool.release(b);
        assert_eq!(pool.stats().live, 0);
        assert_eq!(pool.stats().idle, 2);
    }

    #[test]
    fn an_idle_texture_is_evicted_once_it_ages_out() {
        let mut pool = TexturePool::new(CountingAllocator::default()).with_limits(u64::MAX, 2);
        let lease = pool.acquire(desc(64, 64));
        pool.release(lease);

        pool.end_frame();
        pool.end_frame();
        assert_eq!(pool.stats().idle, 1);
        pool.end_frame();
        assert_eq!(pool.stats().idle, 0);
        assert_eq!(pool.stats().evicted, 1);
        assert_eq!(pool.stats().idle_bytes, 0);
    }

    #[test]
    fn acquiring_resets_the_age_so_a_hot_texture_is_never_evicted() {
        let mut pool = TexturePool::new(CountingAllocator::default()).with_limits(u64::MAX, 2);
        for _ in 0..10 {
            let lease = pool.acquire(desc(64, 64));
            pool.release(lease);
            pool.end_frame();
        }
        assert_eq!(pool.stats().created, 1);
        assert_eq!(pool.stats().evicted, 0);
    }

    #[test]
    fn the_byte_budget_evicts_rather_than_growing_without_bound() {
        let one = desc(1024, 1024).byte_size();
        let mut pool =
            TexturePool::new(CountingAllocator::default()).with_limits(one * 2, u32::MAX);

        for _ in 0..5 {
            let a = pool.acquire(desc(1024, 1024));
            let b = pool.acquire(desc(1024, 1024));
            let c = pool.acquire(desc(1024, 1024));
            pool.release(a);
            pool.release(b);
            pool.release(c);
            pool.end_frame();
        }
        assert!(pool.stats().idle_bytes <= one * 2);
        assert!(pool.stats().evicted > 0);
    }

    #[test]
    fn byte_size_follows_the_format() {
        assert_eq!(
            TextureDesc::new(100, 100, wgpu::TextureFormat::Rgba8Unorm).byte_size(),
            100 * 100 * 4
        );
        assert_eq!(
            TextureDesc::new(100, 100, wgpu::TextureFormat::Rgba16Float).byte_size(),
            100 * 100 * 8
        );
        assert_eq!(
            TextureDesc::new(100, 100, wgpu::TextureFormat::R8Unorm).byte_size(),
            100 * 100
        );
    }

    #[test]
    fn clearing_drops_every_idle_texture_and_keeps_the_accounting_straight() {
        let mut pool = pool();
        let a = pool.acquire(desc(64, 64));
        let b = pool.acquire(desc(128, 128));
        pool.release(a);
        pool.release(b);
        pool.clear();
        assert_eq!(pool.stats().idle, 0);
        assert_eq!(pool.stats().idle_bytes, 0);
        assert_eq!(pool.stats().evicted, 2);
    }

    #[test]
    fn a_steady_frame_loop_allocates_once_and_then_never_again() {
        let mut pool = pool();
        for _ in 0..600 {
            let bg = pool.acquire(desc(1920, 1080));
            let scratch = pool.acquire(desc(1920, 1080));
            let half = pool.acquire(desc(960, 540));
            pool.release(bg);
            pool.release(scratch);
            pool.release(half);
            pool.end_frame();
        }
        assert_eq!(pool.stats().created, 3);
        assert_eq!(pool.stats().live, 0);
    }
}
