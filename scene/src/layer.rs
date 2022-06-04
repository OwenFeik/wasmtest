use std::sync::atomic::{AtomicI64, Ordering};

use serde_derive::{Deserialize, Serialize};

use super::{Id, ScenePoint, Sprite};

#[derive(Clone, Serialize, Deserialize)]
pub struct Layer {
    pub local_id: Id,
    pub canonical_id: Option<Id>,
    pub title: String,
    pub z: i32,
    pub sprites: Vec<Sprite>,
    pub z_min: i32,
    pub z_max: i32,
}

impl Layer {
    fn next_id() -> Id {
        static LAYER_ID: AtomicI64 = AtomicI64::new(1);
        LAYER_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub fn new(title: &str, z: i32) -> Self {
        Layer {
            local_id: Self::next_id(),
            canonical_id: None,
            title: title.to_string(),
            z,
            sprites: Vec::new(),
            z_min: 0,
            z_max: 0,
        }
    }

    pub fn refresh_local_ids(&mut self) {
        self.local_id = Self::next_id();
        self.sprites = self
            .sprites
            .iter_mut()
            .map(|s| Sprite::from_remote(s))
            .collect();
    }

    pub fn sprite(&mut self, local_id: Id) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.local_id == local_id)
    }

    pub fn sprite_canonical(&mut self, canonical_id: Id) -> Option<&mut Sprite> {
        self.sprites
            .iter_mut()
            .find(|s| s.canonical_id == Some(canonical_id))
    }

    pub fn sprite_canonical_ref(&self, canonical_id: Id) -> Option<&Sprite> {
        self.sprites
            .iter()
            .find(|s| s.canonical_id == Some(canonical_id))
    }

    fn sort_sprites(&mut self) {
        self.sprites.sort_by(|a, b| a.z.cmp(&b.z));
    }

    fn update_z_bounds(&mut self, sprite: &Sprite) {
        if sprite.z > self.z_max {
            self.z_max = sprite.z;
        } else if sprite.z < self.z_min {
            self.z_min = sprite.z;
        }
    }

    pub fn add_sprite(&mut self, sprite: Sprite) {
        self.update_z_bounds(&sprite);
        self.sprites.push(sprite);
        self.sort_sprites();
    }

    pub fn add_sprites(&mut self, sprites: &mut Vec<Sprite>) {
        for s in sprites.iter() {
            self.update_z_bounds(s);
        }
        self.sprites.append(sprites);
        self.sort_sprites();
    }

    pub fn remove_sprite(&mut self, local_id: Id) {
        self.sprites.retain(|s| s.local_id != local_id);
    }

    pub fn sprite_at(&mut self, at: ScenePoint) -> Option<&mut Sprite> {
        // Reversing the iterator atm because the sprites are rendered from the
        // front of the Vec to the back, hence the last Sprite in the Vec is
        // rendered on top, and will be clicked first.
        for sprite in self.sprites.iter_mut().rev() {
            if sprite.rect.contains_point(at) {
                return Some(sprite);
            }
        }

        None
    }
}

impl Default for Layer {
    fn default() -> Self {
        Layer::new("Layer", 0)
    }
}