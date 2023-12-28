use bevy::{
    app::prelude::*,
    asset::Handle,
    core_pipeline::{prelude::*, Skybox},
    ecs::prelude::*,
    render::{prelude::*, view::RenderLayers},
    transform::prelude::*,
};

#[derive(Component)]
pub struct SkyboxCamConfig {
    pub skybox: Handle<Image>,
    /// The corresponding skybox camera entity.
    skybox_cam: Option<Entity>,
}

impl SkyboxCamConfig {
    pub fn new(skybox: Handle<Image>) -> Self {
        Self {
            skybox,
            skybox_cam: None,
        }
    }
}

pub struct SkyboxCamPlugin;

impl Plugin for SkyboxCamPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                SkyboxCam::spawn,
                SkyboxCam::despawn,
                apply_deferred,
                SkyboxCam::update,
            )
                .chain(),
        );
    }
}

#[derive(Component)]
pub struct SkyboxCam {
    /// The camera that this skybox camera is observing.
    driven_by: Entity,
}

impl SkyboxCam {
    /// Spawns [`SkyboxCam`]s when a [`SkyboxCamConfig`] exists without a skybox entity.
    pub fn spawn(
        mut commands: Commands,
        mut editor_cams: Query<(Entity, &mut SkyboxCamConfig)>,
        skybox_cams: Query<&SkyboxCam>,
    ) {
        for (editor_cam_entity, mut editor_without_skybox) in
            editor_cams.iter_mut().filter(|(_, config)| {
                config
                    .skybox_cam
                    .and_then(|e| skybox_cams.get(e).ok())
                    .is_none()
            })
        {
            let entity = commands
                .spawn((
                    Camera3dBundle {
                        camera: Camera {
                            order: 0,
                            hdr: true,
                            ..Default::default()
                        },
                        camera_3d: Camera3d {
                            clear_color: bevy::core_pipeline::clear_color::ClearColorConfig::None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    RenderLayers::none(),
                    Skybox(editor_without_skybox.skybox.clone()),
                    SkyboxCam {
                        driven_by: editor_cam_entity,
                    },
                ))
                .id();
            editor_without_skybox.skybox_cam = Some(entity);
        }
    }

    /// Despawns [`SkyboxCam`]s when their corresponding [`SkyboxCamConfig`] entity does not exist.
    pub fn despawn(
        mut commands: Commands,
        skybox_cams: Query<(Entity, &SkyboxCam)>,
        editor_cams: Query<&SkyboxCamConfig>,
    ) {
        for (skybox_entity, skybox) in &skybox_cams {
            if editor_cams.get(skybox.driven_by).is_err() {
                commands.entity(skybox_entity).despawn();
            }
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn update(
        editor_cams: Query<
            (&SkyboxCamConfig, &Transform, &Projection),
            (
                Or<(Changed<SkyboxCamConfig>, Changed<Transform>)>,
                Without<Self>,
            ),
        >,
        mut skybox_cams: Query<(&mut Transform, &mut Projection), With<Self>>,
    ) {
        for (editor_cam, editor_transform, editor_projection) in &editor_cams {
            let Some(skybox_entity) = editor_cam.skybox_cam else {
                continue;
            };
            let Ok((mut skybox_transform, mut skybox_projection)) =
                skybox_cams.get_mut(skybox_entity)
            else {
                continue;
            };

            if let Projection::Perspective(editor_perspective) = editor_projection {
                *skybox_projection = Projection::Perspective(editor_perspective.clone())
            }

            *skybox_transform = *editor_transform;
        }
    }
}
