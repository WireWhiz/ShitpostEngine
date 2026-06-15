use glam::{Quat, Vec3};

//use crate::resources::{MaterialId, MeshId};

/*
pub struct Chunk {
    pub actors: Vec<Actor>,
    pub unused_actor_ids: Vec<ActorId>,
}

impl Chunk {
    pub fn new_actor(&mut self, actor: Actor) -> ActorId {
        if let Some(id) = self.unused_actor_ids.pop() {
            id
        } else {
            let id = self.actors.len();
            self.actors.push(actor);
            id
        }
    }
}

pub type ActorId = usize;
pub struct Actor {
    pub transform: Transform,
    pub mesh: Option<ActorMesh>,
}

pub struct Transform {
    parent: Option<ActorId>,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

pub struct ActorMesh {
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
}
*/
