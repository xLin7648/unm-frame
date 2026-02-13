use std::collections::HashMap;

pub trait IdMapKey: Sized {
    fn from(id: u64) -> Self;
    fn to(&self) -> u64;
}

#[derive(Clone)]
pub struct IdMap<V, H: IdMapKey> {
    data: HashMap<u64, V>,
    next_id: u64,
    _phantom: std::marker::PhantomData<H>,
}

impl<V, H: IdMapKey> IdMap<V, H> {
    pub fn new() -> Self {
        IdMap {
            data: HashMap::new(),
            next_id: 1, // 从 1 开始，0 往往可以作为无效句柄的保留值
            _phantom: std::marker::PhantomData,
        }
    }

    /// 插入新值，生成一个全局唯一的句柄
    pub fn insert(&mut self, value: V) -> H {
        let current_id = self.next_id;

        // 检查 ID 溢出情况
        if current_id == u64::MAX {
            panic!("IdMap ID 空间已耗尽！无法生成更多唯一的 ID。");
        }

        // 核心逻辑：直接自增，不检查 free_ids，不回收任何 ID
        self.next_id += 1;

        self.data.insert(current_id, value);
        H::from(current_id)
    }

    /// 移除值，其对应的句柄将永远变为失效状态
    pub fn remove(&mut self, handle: H) -> Option<V> {
        let id_value = handle.to();
        // 直接从 map 中移除，不再将 id 放入 free_ids
        self.data.remove(&id_value)
    }

    pub fn get(&self, handle: H) -> Option<&V> {
        self.data.get(&handle.to())
    }

    pub fn get_mut(&mut self, handle: H) -> Option<&mut V> {
        self.data.get_mut(&handle.to())
    }

    pub fn keys(&self) -> impl Iterator<Item = H> + '_ {
        self.data.keys().map(|&id| H::from(id))
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.data.values()
    }

    pub fn iter(&self) -> impl Iterator<Item = (H, &V)> {
        self.data.iter().map(|(&id, v)| (H::from(id), v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (H, &mut V)> {
        self.data.iter_mut().map(|(&id, v)| (H::from(id), v))
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 获取下一个即将分配的 ID（用于调试或统计）
    pub fn peek_next_id(&self) -> u64 {
        self.next_id
    }
}