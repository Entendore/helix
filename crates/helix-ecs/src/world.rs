//! World (ECS-1..8): generational entities, sparse-set storage (ECS-4) with
//! change stamps (CHG-1), n-way queries over the smallest store (ECS-6).
//! Structural changes are immediate; systems obey snapshot-mutate (ECS-7).
use std::any::{Any, TypeId};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Entity {
    pub index: u32,
    pub generation: u32,
}

pub trait Component: Any + 'static {}
impl<T: Any + 'static> Component for T {}

trait Erased {
    fn remove(&mut self, index: u32);
    fn clear(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// ECS-4: dense values + sparse slot map; removal is swap-remove, so
/// iteration order is unspecified (ECS-8). `ticks` is parallel to `dense`.
struct Storage<T> {
    dense: Vec<(u32, T)>,
    sparse: Vec<u32>, // slot+1 = dense position; 0 = absent
    ticks: Vec<u64>,
}

impl<T: Component> Storage<T> {
    fn get(&self, index: u32) -> Option<&T> {
        let &p = self.sparse.get(index as usize)?;
        let (i, v) = self.dense.get(p as usize - 1)?;
        (i == index).then_some(v)
    }
    fn get_mut(&mut self, index: u32, tick: u64) -> Option<&mut T> {
        let &p = self.sparse.get(index as usize)?;
        let pos = p as usize - 1;
        let (i, v) = self.dense.get_mut(pos)?;
        if *i == index {
            self.ticks[pos] = tick; // CHG-1: any &mut access is a write
            Some(v)
        } else {
            None
        }
    }
    fn insert(&mut self, index: u32, value: T, tick: u64) {
        if let Some(&p) = self.sparse.get(index as usize) {
            if p != 0 {
                self.dense[p as usize - 1].1 = value;
                self.ticks[p as usize - 1] = tick;
                return;
            }
        }
        if self.sparse.len() <= index as usize {
            self.sparse.resize(index as usize + 1, 0);
        }
        self.sparse[index as usize] = self.dense.len() as u32 + 1;
        self.dense.push((index, value));
        self.ticks.push(tick);
    }
    fn remove(&mut self, index: u32) -> Option<T> {
        let &p = self.sparse.get(index as usize)?;
        let pos = p as usize - 1;
        if self.dense.get(pos)?.0 != index {
            return None;
        }
        let last = self.dense.len() - 1;
        self.dense.swap(pos, last);
        self.ticks.swap(pos, last);
        let (moved, value) = self.dense.pop().unwrap();
        self.ticks.pop();
        if pos != last {
            self.sparse[moved.0 as usize] = pos as u32 + 1;
        }
        self.sparse[index as usize] = 0;
        Some(value)
    }
    fn changed(&self, index: u32, since: u64) -> bool {
        let &p = self.sparse.get(index as usize).unwrap_or(&0);
        p != 0 && self.ticks[p as usize - 1] >= since
    }
    fn clear(&mut self) {
        self.dense.clear();
        self.ticks.clear();
        self.sparse.clear();
    }
}

impl<T: Component> Erased for Storage<T> {
    fn remove(&mut self, index: u32) {
        Storage::remove(self, index);
    }
    fn clear(&mut self) {
        Storage::clear(self);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Default)]
pub struct World {
    pub tick: u64,
    generations: Vec<u32>,
    live: Vec<bool>,
    free: Vec<u32>,
    storages: HashMap<TypeId, Box<dyn Erased>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self) -> Entity {
        let index = if let Some(i) = self.free.pop() {
            i
        } else {
            self.generations.push(0);
            self.live.push(false);
            (self.generations.len() - 1) as u32
        };
        self.live[index as usize] = true;
        Entity { index, generation: self.generations[index as usize] }
    }

    /// Generation bumps exactly here: stale handles die on despawn (ECS-2).
    pub fn despawn(&mut self, e: Entity) {
        if !self.is_alive(e) {
            return;
        }
        self.generations[e.index as usize] = self.generations[e.index as usize].wrapping_add(1);
        self.live[e.index as usize] = false;
        self.free.push(e.index);
        for s in self.storages.values_mut() {
            s.remove(e.index);
        }
    }

    /// ECL-4: bump EVERY slot's generation so no pre-clear handle can alias.
    pub fn clear(&mut self) {
        for g in self.generations.iter_mut() {
            *g = g.wrapping_add(1);
        }
        self.free = (0..self.generations.len() as u32).collect();
        self.live.iter_mut().for_each(|l| *l = false);
        self.storages.clear();
    }

    pub fn is_alive(&self, e: Entity) -> bool {
        self.generations.get(e.index as usize) == Some(&e.generation)
            && self.live.get(e.index as usize) == Some(&true)
    }

    /// Live identities only. Order unspecified (ECS-8).
    pub fn iter_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.generations.iter().enumerate().filter_map(|(i, &g)| {
            self.live
                .get(i)
                .filter(|&&l| l)
                .map(|_| Entity { index: i as u32, generation: g })
        })
    }

    fn typed<T: Component>(&self) -> Option<&Storage<T>> {
        self.storages.get(&TypeId::of::<T>())?.as_any().downcast_ref()
    }
    fn typed_mut<T: Component>(&mut self) -> &mut Storage<T> {
        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Storage { dense: vec![], sparse: vec![], ticks: vec![] }))
            .as_any_mut()
            .downcast_mut()
            .expect("storage type mismatch")
    }

    /// ECS-5: insert on a dead entity is a programmer error.
    pub fn insert<T: Component>(&mut self, e: Entity, c: T) {
        assert!(self.is_alive(e), "insert on dead entity {e:?}");
        let tick = self.tick;
        self.typed_mut::<T>().insert(e.index, c, tick);
    }

    pub fn get<T: Component>(&self, e: Entity) -> Option<&T> {
        self.typed::<T>()?.get(e.index)
    }

    pub fn get_mut<T: Component>(&mut self, e: Entity) -> Option<&mut T> {
        let tick = self.tick;
        self.typed_mut::<T>().get_mut(e.index, tick)
    }

    pub fn remove_component<T: Component>(&mut self, e: Entity) -> Option<T> {
        self.typed_mut::<T>().remove(e.index)
    }

    pub fn changed<T: Component>(&self, e: Entity, since: u64) -> bool {
        self.typed::<T>().map(|s| s.changed(e.index, since)).unwrap_or(false)
    }

    pub fn query1<'a, A: Component>(&'a self) -> Box<dyn Iterator<Item = (Entity, &'a A)> + 'a> {
        match self.typed::<A>() {
            Some(s) => {
                let gens = &self.generations;
                Box::new(s.dense.iter().map(move |(i, a)| {
                    (Entity { index: *i, generation: gens[*i as usize] }, a)
                }))
            }
            None => Box::new(std::iter::empty()),
        }
    }

    pub fn query2<'a, A: Component, B: Component>(
        &'a self,
    ) -> Box<dyn Iterator<Item = (Entity, &'a A, &'a B)> + 'a> {
        let (Some(sa), Some(sb)) = (self.typed::<A>(), self.typed::<B>())
            else { return Box::new(std::iter::empty()) };
        let gens = &self.generations;
        Box::new(sa.dense.iter().filter_map(move |(i, a)| {
            let b = sb.get(*i)?;
            Some((Entity { index: *i, generation: gens[*i as usize] }, a, b))
        }))
    }

    pub fn query3<'a, A: Component, B: Component, C: Component>(
        &'a self,
    ) -> Box<dyn Iterator<Item = (Entity, &'a A, &'a B, &'a C)> + 'a> {
        let (Some(sa), Some(sb), Some(sc)) =
            (self.typed::<A>(), self.typed::<B>(), self.typed::<C>())
            else { return Box::new(std::iter::empty()) };
        let gens = &self.generations;
        Box::new(sa.dense.iter().filter_map(move |(i, a)| {
            let b = sb.get(*i)?;
            let c = sc.get(*i)?;
            Some((Entity { index: *i, generation: gens[*i as usize] }, a, b, c))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [T] ECS-2/5 — lifecycle, generation reuse, dead-insert panic.
    #[test]
    fn lifecycle_and_generations() {
        let mut w = World::new();
        let a = w.spawn();
        w.insert(a, 10u32);
        w.insert(a, 1.5f32);
        let b = w.spawn();
        w.insert(b, 20u32);
        assert_eq!(w.query2::<u32, f32>().count(), 1);
        w.despawn(a);
        assert!(!w.is_alive(a));
        assert!(w.get::<u32>(a).is_none());
        let c = w.spawn();
        assert_eq!(c.index, a.index);
        assert_ne!(c.generation, a.generation);
    }

    /// [T] ECS-4/CHG-1 — swap-remove keeps others reachable; stamps work.
    #[test]
    fn swap_remove_and_stamps() {
        let mut w = World::new();
        let (a, b, c) = (w.spawn(), w.spawn(), w.spawn());
        for e in [a, b, c] {
            w.insert(e, e.index as i64);
        }
        assert_eq!(w.remove_component::<i64>(b), Some(b.index as i64));
        assert_eq!(w.get::<i64>(c), Some(&(c.index as i64)));
        assert!(!w.changed::<i64>(a, w.tick + 1));
        w.tick += 1;
        *w.get_mut::<i64>(a).unwrap() = 99;
        assert!(w.changed::<i64>(a, w.tick));
        assert!(!w.changed::<i64>(c, w.tick));
    }

    /// [T] ECL-4 — clear() kills every stale handle.
    #[test]
    fn clear_invalidates_handles() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, 1u32);
        w.clear();
        assert!(!w.is_alive(e));
        assert_eq!(w.iter_entities().count(), 0);
        let e2 = w.spawn();
        assert_ne!(e2.generation, e.generation);
    }
}
