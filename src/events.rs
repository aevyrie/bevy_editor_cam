use bevy::ecs::event::Event;

#[derive(Debug, Clone, Event)]
pub enum EditorCamEvent {
    Projection(ProjectionChange),
}

impl EditorCamEvent {
    // fn receive(events: EventReader<Self>) {}
}

#[derive(Debug, Clone)]
pub enum ProjectionChange {
    Perspective,
    Orthographic,
    Toggle,
}
